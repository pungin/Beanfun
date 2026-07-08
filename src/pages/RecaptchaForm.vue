<script setup lang="ts">
/**
 * reCAPTCHA widget-solve status page (issues #313 / #315 / #318 —
 * token-replay).
 *
 * # Why this page exists
 *
 * The TW account/password login (`Login/Index`) gates its two POSTs
 * (`CheckAccountType` / `AccountLogin`) behind a Google reCAPTCHA
 * Enterprise challenge whenever the server flags the client (IP /
 * accelerator / VPN reputation). The token cannot be produced by the
 * headless `reqwest` flow, and — the lesson of #308/#309 — it cannot be
 * produced by doing the whole login inside a WebView either (WebView2
 * Tracking Prevention breaks the widget; #318). So when `loginRegular`
 * surfaces `auth.recaptcha_required`, `IdPassForm` routes here.
 *
 * # Flow (token-replay)
 *
 * ```
 *   IdPassForm submit → loginRegular → auth.recaptcha_required
 *     → backend parks a resumable `pending_tw_login` continuation
 *     → router.push('/login/recaptcha')   (this page)
 *         → mount → register recaptcha-token / -cancelled listeners
 *                 → openRecaptchaWindow   (widget-solve popup on beanfun origin)
 *                 → user ticks "I'm not a robot" in the popup
 *                 → backend harvests the token from the URL fragment and
 *                   emits `recaptcha-token`
 *                 → resumeTwLoginWithRecaptcha(token) replays it over HTTP:
 *                     • SessionInfo   → persist creds + /accounts
 *                     • pendingVerify → /login/verify (advance check)
 *                     • pendingRecaptcha (next step also gated) → re-open widget
 *                 → `recaptcha-cancelled` (closed / timed out) → retry / back
 * ```
 *
 * Unlike #308/#309 this page never completes the login in-page and never
 * touches `applyGamepassSession` — the backend owns the HTTP flow; the
 * popup is a pure token harvester.
 */

import { onBeforeUnmount, onMounted, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useRouter } from 'vue-router'
import { ElButton } from 'element-plus'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'

import { useAuthStore } from '../stores/auth'
import { useAccountStore } from '../stores/account'
import { LOGIN_METHOD } from '../constants/login'
import { CommandInvocationError } from '../services/invoke'

/**
 * Tauri events emitted by the backend reCAPTCHA widget window
 * (`src-tauri/src/commands/auth.rs` — `RECAPTCHA_TOKEN_EVENT` /
 * `RECAPTCHA_CANCELLED_EVENT`). Kept as literals because `tauri-specta`
 * models commands, not `app.emit(...)` events.
 */
const TOKEN_EVENT = 'recaptcha-token'
const CANCELLED_EVENT = 'recaptcha-cancelled'

interface RecaptchaTokenPayload {
  step: string
  token: string
}

defineOptions({ name: 'RecaptchaForm' })

type Phase = 'opening' | 'waiting' | 'verifying' | 'cancelled' | 'error'

const { t } = useI18n()
const router = useRouter()
const auth = useAuthStore()
const account = useAccountStore()

const phase = ref<Phase>('opening')

/**
 * Disposal sentinel — set on unmount / terminal navigation so a late
 * event callback is a no-op. Same rationale as `GamepassForm`.
 */
let disposed = false

const unlistenFns: UnlistenFn[] = []

/**
 * Persist the just-used credentials on a full success, mirroring
 * `IdPassForm::persistAfterFullSuccess` (WPF `SaveLoginCredentials`).
 * Best-effort: the login already succeeded, so a persistence hiccup only
 * costs the next-boot prefill.
 */
async function persistOnSuccess(): Promise<void> {
  const intent = auth.loginIntent
  if (!intent) return
  try {
    await account.saveLoginCredentials({
      region: intent.region,
      accountId: intent.accountId,
      password: intent.password,
      rememberPassword: intent.rememberPassword,
      verify: auth.verifyIntent?.code ?? '',
      rememberVerify: auth.verifyIntent?.remember ?? false,
      method: LOGIN_METHOD.Regular,
      autoLogin: intent.autoLogin,
    })
  } catch (e) {
    console.error('[recaptcha-form] persistOnSuccess failed', e)
  } finally {
    auth.clearLoginIntent()
    auth.clearVerifyIntent()
  }
}

async function openWidget(): Promise<void> {
  phase.value = 'opening'
  try {
    await auth.openRecaptchaWindow()
    if (disposed) return
    phase.value = 'waiting'
  } catch (e) {
    if (disposed) return
    // `openRecaptchaWindow` already toasted via `wrapCommand`; the banner
    // adds the retry / go-back affordance. A non-CommandInvocationError
    // (e.g. withGuard "already in progress" on a double mount) is benign.
    if (e instanceof CommandInvocationError) phase.value = 'error'
  }
}

async function onToken(token: string): Promise<void> {
  if (disposed) return
  phase.value = 'verifying'
  try {
    const session = await auth.resumeTwLoginWithRecaptcha(token)
    if (disposed) return
    if (session) {
      await persistOnSuccess()
      account.clearSessionData()
      disposed = true
      await router.push('/accounts')
      return
    }
    // Continuation flags — inspect both without assuming exclusivity.
    if (auth.pendingVerify) {
      disposed = true
      await router.push('/login/verify')
      return
    }
    if (auth.pendingRecaptcha) {
      // The *next* step now also needs a token — re-open the widget.
      await openWidget()
      return
    }
  } catch {
    if (disposed) return
    // `resumeTwLoginWithRecaptcha` already toasted via `surfaceCommandError`.
    phase.value = 'error'
  }
}

async function registerEventListeners(): Promise<void> {
  const tokenUnlisten = await listen<RecaptchaTokenPayload>(TOKEN_EVENT, (event) => {
    void onToken(event.payload.token)
  })
  const cancelledUnlisten = await listen<null>(CANCELLED_EVENT, () => {
    if (disposed) return
    phase.value = 'cancelled'
  })
  unlistenFns.push(tokenUnlisten, cancelledUnlisten)
}

onMounted(async () => {
  // Listeners MUST register before `openRecaptchaWindow` so an eager token
  // (harvested before the command Promise resolves) is not missed.
  try {
    await registerEventListeners()
  } catch (e) {
    console.error('[recaptcha-form] failed to register tauri listeners', e)
    phase.value = 'error'
    return
  }
  if (disposed) return
  await openWidget()
})

onBeforeUnmount(() => {
  disposed = true
  for (const unlisten of unlistenFns) {
    try {
      unlisten()
    } catch (e) {
      console.error('[recaptcha-form] unlisten threw', e)
    }
  }
  unlistenFns.length = 0
})

async function retry(): Promise<void> {
  await openWidget()
}

async function goBack(): Promise<void> {
  disposed = true
  await router.push('/login/id-pass')
}
</script>

<template>
  <section class="recaptcha-form">
    <header class="recaptcha-form__header">
      <h3 class="recaptcha-form__title">{{ t('loginRecaptcha.title') }}</h3>
      <p class="recaptcha-form__subtitle">{{ t('loginRecaptcha.subtitle') }}</p>
    </header>

    <p
      v-if="phase === 'opening' || phase === 'waiting' || phase === 'verifying'"
      class="recaptcha-form__status"
      data-testid="recaptcha-status"
    >
      <template v-if="phase === 'opening'">{{ t('loginRecaptcha.opening') }}</template>
      <template v-else-if="phase === 'waiting'">{{ t('loginRecaptcha.waiting') }}</template>
      <template v-else>{{ t('loginRecaptcha.verifying') }}</template>
    </p>

    <p
      v-if="phase === 'cancelled'"
      class="recaptcha-form__notice"
      data-testid="recaptcha-cancelled"
    >
      {{ t('loginRecaptcha.cancelled') }}
    </p>

    <p v-if="phase === 'error'" class="recaptcha-form__error" data-testid="recaptcha-error">
      {{ t('loginRecaptcha.windowError') }}
    </p>

    <p class="recaptcha-form__alt">{{ t('loginRecaptcha.altHint') }}</p>

    <div class="recaptcha-form__actions">
      <el-button
        v-if="phase === 'cancelled' || phase === 'error'"
        class="recaptcha-form__retry"
        type="primary"
        size="large"
        data-testid="recaptcha-retry"
        @click="retry"
      >
        {{ t('loginRecaptcha.retry') }}
      </el-button>
      <el-button
        class="recaptcha-form__back"
        :type="phase === 'cancelled' || phase === 'error' ? 'default' : 'primary'"
        size="large"
        data-testid="recaptcha-back"
        @click="goBack"
      >
        {{ t('BackRegularLogin') }}
      </el-button>
    </div>
  </section>
</template>

<style scoped>
.recaptcha-form {
  display: flex;
  flex-direction: column;
  gap: 1rem;
  align-items: stretch;
}

.recaptcha-form__header {
  text-align: center;
}

.recaptcha-form__title {
  margin: 0;
  font-size: 1rem;
  font-weight: 700;
  color: #1f1a16;
}

.recaptcha-form__subtitle {
  margin: 0.375rem 0 0;
  font-size: 0.8125rem;
  color: #54443a;
}

.recaptcha-form__status {
  margin: 0;
  padding: 0.625rem 0.875rem;
  border-radius: 8px;
  background: color-mix(in srgb, var(--el-color-primary, #ff8201) 12%, transparent);
  color: var(--bf-primary, #954a00);
  font-size: 0.8125rem;
  text-align: center;
}

.recaptcha-form__notice {
  margin: 0;
  padding: 0.625rem 0.875rem;
  border-radius: 8px;
  background: color-mix(in srgb, var(--el-color-warning, #e6a23c) 14%, transparent);
  color: var(--el-color-warning, #b88230);
  font-size: 0.8125rem;
  text-align: center;
}

.recaptcha-form__error {
  margin: 0;
  padding: 0.625rem 0.875rem;
  border-radius: 8px;
  background: color-mix(in srgb, var(--el-color-danger, #f56c6c) 14%, transparent);
  color: var(--el-color-danger, #f56c6c);
  font-size: 0.8125rem;
  text-align: center;
}

.recaptcha-form__alt {
  margin: 0;
  font-size: 0.75rem;
  color: #54443a;
  text-align: center;
}

.recaptcha-form__actions {
  display: flex;
  gap: 0.75rem;
}

.recaptcha-form__retry,
.recaptcha-form__back {
  flex: 1;
  font-weight: 700;
}
</style>
