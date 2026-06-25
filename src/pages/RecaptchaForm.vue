<script setup lang="ts">
/**
 * reCAPTCHA account-login form (issue #308).
 *
 * # Why this page exists
 *
 * As of 2026-06-25 the TW account/password login (`Login/Index`) gates
 * the credential POSTs behind a Google reCAPTCHA v2 "I'm not a robot"
 * challenge whenever the server flags the client (IP / fingerprint
 * reputation). A reCAPTCHA v2 token cannot be produced by our headless
 * `reqwest` flow — it needs a real browser, a human checkbox click, and
 * a token bound to beanfun's own domain. So when `loginRegular` surfaces
 * `auth.recaptcha_required`, `IdPassForm` routes here instead of
 * treating it as an error.
 *
 * # Flow
 *
 * ```
 *   IdPassForm submit → loginRegular → auth.recaptcha_required
 *     → backend parks (client, skey) on `pending_gamepass`
 *     → router.push('/login/recaptcha')   (this page)
 *         → mount → register gamepass-login-* listeners
 *                 → openAccountLoginWindow  (opens real Login/Index page)
 *                 → user types account + password + solves reCAPTCHA there
 *                 → on gamepass-login-success → applyGamepassSession + /accounts
 *                 → on gamepass-login-failed  → error banner
 *                 → on gamepass-login-cancelled → "go back" prompt
 * ```
 *
 * # Why it reuses the GamePass machinery
 *
 * The account-login WebView is mechanically identical to the GamePass
 * WebView — both seed the session cookies, open the same
 * `Login/Index?pSKey=…` page, and harvest `bfWebToken` once beanfun's
 * redirect chain lands on `return.aspx`. The backend therefore reuses
 * the same `pending_gamepass` slot, completion handlers, and
 * `gamepass-login-*` events; this page reuses {@link useAuthStore.applyGamepassSession}
 * the same way `GamepassForm` does. The only backend difference is that
 * the account-login window does **not** auto-click the Gama Pass button,
 * so the user sees the normal account/password + reCAPTCHA form.
 *
 * # No in-place retry
 *
 * Unlike `GamepassForm` (which can re-run `loginGamepassStart` to re-arm
 * the slot), this page cannot re-arm `pending_gamepass` — that requires
 * re-running `loginRegular` with the user's credentials, which only
 * `IdPassForm` holds. So a cancel / failure routes the user **back to
 * the id-pass form** to re-submit, rather than offering a same-page
 * refresh.
 */

import { onBeforeUnmount, onMounted, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useRouter } from 'vue-router'
import { ElButton } from 'element-plus'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'

import { useAuthStore } from '../stores/auth'
import { useAccountStore } from '../stores/account'
import { CommandInvocationError } from '../services/invoke'
import type { CommandError, SessionInfo } from '../types/bindings'

/**
 * Tauri event names emitted by the backend WebView-login flow. Shared
 * verbatim with the GamePass flow (the backend reuses the same events);
 * the authoritative definitions live in `src-tauri/src/commands/auth.rs`
 * (`GAMEPASS_SUCCESS_EVENT` etc.). Kept as literals because
 * `tauri-specta` models commands, not `app.emit(...)` events.
 */
const SUCCESS_EVENT = 'gamepass-login-success'
const FAILED_EVENT = 'gamepass-login-failed'
const CANCELLED_EVENT = 'gamepass-login-cancelled'

defineOptions({ name: 'RecaptchaForm' })

type Phase = 'opening' | 'waiting' | 'cancelled' | 'error'

const { t } = useI18n()
const router = useRouter()
const auth = useAuthStore()
const account = useAccountStore()

const phase = ref<Phase>('opening')

/**
 * Disposal sentinel — set on unmount so a late event callback (after the
 * user navigated away) is a no-op. Same rationale as `GamepassForm`.
 */
let disposed = false

const unlistenFns: UnlistenFn[] = []

async function registerEventListeners(): Promise<void> {
  const successUnlisten = await listen<SessionInfo>(SUCCESS_EVENT, async (event) => {
    if (disposed) return
    account.clearSessionData()
    auth.applyGamepassSession(event.payload)
    disposed = true
    await router.push('/accounts')
  })
  const failedUnlisten = await listen<CommandError>(FAILED_EVENT, (event) => {
    if (disposed) return
    // The cookie-harvest failure path doesn't go through `wrapCommand`,
    // so the console log is the one authoritative trail for an operator.
    console.error(`[recaptcha-form] ${FAILED_EVENT}`, event.payload)
    phase.value = 'error'
  })
  const cancelledUnlisten = await listen<null>(CANCELLED_EVENT, () => {
    if (disposed) return
    phase.value = 'cancelled'
  })
  unlistenFns.push(successUnlisten, failedUnlisten, cancelledUnlisten)
}

async function openWindow(): Promise<void> {
  phase.value = 'opening'
  try {
    // Pass the credentials the user already typed in `IdPassForm` (still
    // held in `loginIntent`) so the backend can best-effort autofill
    // beanfun's own login form — the user then only solves the reCAPTCHA
    // and submits. Empty strings (no intent) make the autofill a no-op.
    const intent = auth.loginIntent
    await auth.openAccountLoginWindow(intent?.accountId ?? '', intent?.password ?? '')
    if (disposed) return
    phase.value = 'waiting'
  } catch (e) {
    if (disposed) return
    // `openAccountLoginWindow` already toasted via `wrapCommand`; the
    // banner adds the "go back and sign in again" affordance. A non-
    // CommandInvocationError (e.g. withGuard "already in progress" on a
    // double mount) is benign — leave the phase untouched.
    if (e instanceof CommandInvocationError) {
      phase.value = 'error'
    }
  }
}

onMounted(async () => {
  // Listeners MUST register before `openAccountLoginWindow` so an eager
  // success (harvest completes before the command Promise resolves) is
  // not missed.
  try {
    await registerEventListeners()
  } catch (e) {
    console.error('[recaptcha-form] failed to register tauri listeners', e)
    phase.value = 'error'
    return
  }
  if (disposed) return
  await openWindow()
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
      v-if="phase === 'opening' || phase === 'waiting'"
      class="recaptcha-form__status"
      data-testid="recaptcha-status"
    >
      {{ phase === 'opening' ? t('loginRecaptcha.opening') : t('loginRecaptcha.waiting') }}
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
        class="recaptcha-form__back"
        type="primary"
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

.recaptcha-form__back {
  width: 100%;
  font-weight: 700;
}
</style>
