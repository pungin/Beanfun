<script setup lang="ts">
/**
 * TOTP (6-digit 2FA) login form.
 *
 * # WPF parity
 *
 * Mirrors `Beanfun/Pages/LoginTotp.xaml(.cs)`. The WPF page renders
 * six single-character `TextBox`es (`totp_1` … `totp_6`) with a
 * `PreviewKeyUp` handler that auto-advances focus to the next box
 * and, once the sixth is filled, auto-submits by pressing `btn_login`
 * (`IsDefault="True"` so Enter also triggers it). A `btn_cancel`
 * button returns to the login page.
 *
 * The Vue port preserves every observable behaviour:
 *
 * | WPF                                           | Vue                                                  |
 * |-----------------------------------------------|------------------------------------------------------|
 * | 6 `TextBox` with `MaxLength="1"`              | 6 `ElInput` with `maxlength="1"`                    |
 * | `PreviewKeyUp` auto-advance + auto-submit     | `useOtpInputs` `handleInput` + `onComplete(submit)` |
 * | `btn_login` `IsDefault="True"` Enter submits  | `<el-form @submit.prevent="submit">` + native-type=submit |
 * | `btn_cancel` → `NavigateLoginPage()`          | Back link → `router.push('/login/id-pass')`          |
 * | `totpWorker_RunWorkerCompleted` error nav    | Catch branch → `router.push('/login/id-pass')`       |
 * | `GotFocus="totp_1_GotFocus"` (SelectAll)      | `@focus="selectOnFocus"` on every cell               |
 *
 * # Why reuse `useOtpInputs` instead of hand-rolling focus logic
 *
 * The same focus / paste / digit-filter dance is needed for P12.1 D8
 * verify-captcha and any future 2FA flows. Extracting the logic keeps
 * the page a thin presentational shell (SRP) and prevents the inevitable
 * drift between two hand-rolled implementations (DRY). See the
 * composable's header for the WPF-parity behaviour matrix.
 *
 * # Post-submit navigation
 *
 * `auth.loginTotp` surfaces three outcomes, mapped per WPF
 * `totpWorker_RunWorkerCompleted`:
 *
 * | Auth store outcome                | Router push target     | WPF reference                  |
 * |-----------------------------------|------------------------|--------------------------------|
 * | `SessionInfo` returned (success)  | `/accounts`            | `MainWindow` L1480 go accounts |
 * | `null` + `pendingVerify = true`   | `/login/verify`        | `LoginAdvanceCheck` (L1494)    |
 * | throws (invalid code / network)   | `/login/id-pass`       | `errexit(err, 1)` (L1462)      |
 *
 * The `/login/verify` target is D8 territory; in the meantime the
 * catch-all in the router redirects it back to `/` which lands at
 * `/login`, i.e. the user is not stranded. Same trade-off as
 * `IdPassForm.vue`'s Q2 = A deferral.
 *
 * # Why navigate back to `/login/id-pass` on error (not stay on form)
 *
 * WPF hard-resets the session on a TOTP error and forces the user to
 * re-enter credentials. Keeping that contract means we don't have to
 * reason about half-committed server state (the TOTP exchange may or
 * may not have invalidated the skey). `IdPassForm` starts fresh on
 * each mount (`password = ref('')`), matching WPF's
 * `accountList.t_Password.Text = ""` reset.
 */

import { computed, onMounted } from 'vue'
import { useI18n } from 'vue-i18n'
import { useRouter } from 'vue-router'
import { ElButton, ElForm, ElInput } from 'element-plus'

import { useAccountStore } from '../stores/account'
import { AUTH_ACTIONS, useAuthStore } from '../stores/auth'
import { useConfigStore } from '../stores/config'
import { LOGIN_METHOD } from '../constants/login'
import { useOtpInputs, type FocusableInput } from '../composables/useOtpInputs'

defineOptions({ name: 'LoginTotp' })

const TOTP_LENGTH = 6

/**
 * `Config.xml` key tracking the most recently logged-in account id.
 * Mirrors WPF `MainWindow.xaml.cs` L1340 / L1347 — kept in sync
 * with the matching constant in `IdPassForm.vue`. Both files own
 * a `SaveLoginCredentials`-equivalent post-success path; pulling
 * the constant into a shared module would introduce a
 * three-line file with one consumer per file, which loses more
 * to navigation friction than it gains in DRY.
 */
const CONFIG_KEY_LAST_ACCOUNT_ID = 'AccountID'

const { t } = useI18n()
const router = useRouter()
const auth = useAuthStore()
const accountStore = useAccountStore()
const config = useConfigStore()

const otp = useOtpInputs({
  length: TOTP_LENGTH,
  onComplete: (code) => {
    void submit(code)
  },
})
/*
 * Destructure so `cells` lands in setup() as a top-level binding —
 * Vue's template auto-unwrap only walks one level, so a nested
 * `otp.cells.value` access from the template would leak `.value`
 * into the markup (a known Vue 3 footgun).
 */
const { cells, register, handleInput, handleKeydown, handlePaste, focusFirst, reset } = otp

const submitting = computed(() => auth.pendingAction === AUTH_ACTIONS.LoginTotp)

/**
 * Vue template-ref callback. Receives the `ElInput` component instance
 * on mount and `null` on unmount; the composable tolerates either.
 * The `focus()` method is exposed by `ElInput` on its public API, so
 * a structural cast to {@link FocusableInput} is sufficient — we do
 * not depend on any other ElInput internals.
 */
function setCellRef(index: number, el: unknown): void {
  register(index, (el as FocusableInput | null) ?? null)
}

/**
 * WPF `totp_1_GotFocus` parity: selecting the existing digit on focus
 * means a second attempt at a cell overwrites rather than appends, so
 * users who mis-typed and clicked back in are not stuck with a
 * maxlength-blocked input.
 */
function selectOnFocus(event: FocusEvent): void {
  const target = event.target as HTMLInputElement | null
  target?.select()
}

onMounted(() => {
  focusFirst()
})

async function submit(explicitCode?: string): Promise<void> {
  if (submitting.value) return
  const code = explicitCode ?? otp.code.value
  if (code.length !== TOTP_LENGTH) return

  try {
    const session = await auth.loginTotp(code)
    if (session) {
      // WPF parity: TOTP success funnels through the same
      // `OnLoginCompleted` → `SaveLoginCredentials` chain as
      // a no-TOTP regular login (`MainWindow.xaml.cs` L1308-1314,
      // L1334-1363). The form-level state lives in
      // `auth.loginIntent` (stashed by IdPassForm before it
      // navigated here) — see `auth.ts::LoginIntent`.
      await persistAfterFullSuccess()
      await router.push('/accounts')
      return
    }
    if (auth.pendingVerify) {
      // No persistence here — the verify round-trip will land back
      // on IdPassForm and the second-pass success there will run
      // `persistAfterFullSuccess` with the verify code folded in.
      await router.push('/login/verify')
      return
    }
  } catch {
    /*
     * Reset cells before navigating so a future KeepAlive / nav-back
     * lands on an empty form; the toast already fired via
     * `surfaceCommandError` inside the store.
     */
    reset()
    await router.push('/login/id-pass')
  }
}

/**
 * Replays WPF `SaveLoginCredentials` (L1334-1363) on the TOTP
 * success branch. Reads the form snapshot from
 * `auth.loginIntent` (set by IdPassForm before pushing here)
 * and any stashed verify code from `auth.verifyIntent` (rare
 * verify-then-totp ordering — verify slot stays populated until
 * an explicit `clearVerifyIntent`).
 *
 * The intent should always exist on this code path because the
 * only way to land on `/login/totp` is via IdPassForm's
 * `pendingTotp` branch, which stashes the intent before
 * navigating. The defensive guard logs and returns rather than
 * throws so a hypothetical race (deep-link to /login/totp via
 * nav restoration?) does not brick the success navigation.
 */
async function persistAfterFullSuccess(): Promise<void> {
  const intent = auth.loginIntent
  if (!intent) {
    console.warn('[LoginTotp] persistAfterFullSuccess: no loginIntent; skipping persist')
    return
  }
  try {
    await accountStore.saveLoginCredentials({
      region: intent.region,
      accountId: intent.accountId,
      password: intent.password,
      rememberPassword: intent.rememberPassword,
      verify: auth.verifyIntent?.code ?? '',
      rememberVerify: auth.verifyIntent?.remember ?? false,
      method: LOGIN_METHOD.Regular,
      autoLogin: intent.autoLogin,
    })
    await config.set(CONFIG_KEY_LAST_ACCOUNT_ID, intent.accountId)
  } catch (err) {
    console.error('[LoginTotp] persistAfterFullSuccess failed', err)
  } finally {
    auth.clearLoginIntent()
    auth.clearVerifyIntent()
  }
}

function goBack(): void {
  void router.push('/login/id-pass')
}
</script>

<template>
  <el-form class="login-totp" label-position="top" @submit.prevent="submit()">
    <div class="login-totp__icon-wrap">
      <span class="material-symbols-outlined login-totp__icon">security</span>
    </div>

    <header class="login-totp__header">
      <h3 class="login-totp__title">{{ t('loginTotp.title') }}</h3>
      <p class="login-totp__subtitle">{{ t('loginTotp.subtitle') }}</p>
    </header>

    <div class="login-totp__cells" role="group" :aria-label="t('loginTotp.title')">
      <el-input
        v-for="(cell, i) in cells"
        :key="i"
        :ref="(el) => setCellRef(i, el)"
        class="login-totp__cell"
        size="large"
        :model-value="cell"
        :maxlength="1"
        :data-test="`totp-cell-${i}`"
        inputmode="numeric"
        autocomplete="one-time-code"
        @input="(value: string) => handleInput(i, value)"
        @keydown="(event: Event) => handleKeydown(i, event as KeyboardEvent)"
        @paste="(event: Event) => handlePaste(i, event as ClipboardEvent)"
        @focus="(event: Event) => selectOnFocus(event as FocusEvent)"
      />
    </div>

    <div class="login-totp__actions">
      <button type="button" class="login-totp__back-btn" data-test="totp-back" @click="goBack">
        {{ t('Back') }}
      </button>
      <el-button
        type="primary"
        class="login-totp__submit"
        native-type="submit"
        data-test="totp-submit"
        :loading="submitting"
      >
        {{ t('Login') }}
      </el-button>
    </div>
  </el-form>
</template>

<style scoped>
.login-totp {
  display: flex;
  flex-direction: column;
  gap: 1rem;
  align-items: center;
}

.login-totp__icon-wrap {
  width: 64px;
  height: 64px;
  border-radius: 16px;
  background: linear-gradient(
    135deg,
    var(--bf-primary-container, #ff8201),
    var(--bf-primary, #954a00)
  );
  display: grid;
  place-items: center;
  box-shadow: 0 8px 20px color-mix(in srgb, var(--bf-primary, #954a00) 35%, transparent);
}

.login-totp__icon {
  font-size: 32px;
  color: var(--bf-on-primary, #fff);
}

.login-totp__header {
  text-align: center;
}

.login-totp__title {
  margin: 0;
  font-size: 1.5rem;
  font-weight: 800;
  color: var(--bf-on-surface, #1f1a16);
}

.login-totp__subtitle {
  margin: 0.375rem 0 0;
  font-size: 0.8125rem;
  color: var(--bf-on-surface-variant, #54443a);
}

.login-totp__cells {
  display: grid;
  grid-template-columns: repeat(6, 52px);
  gap: 0.5rem;
  justify-content: center;
  margin: 0.5rem 0;
}

.login-totp__cell :deep(.el-input__wrapper) {
  background: rgba(255, 255, 255, 0.6);
  border-bottom: 2px solid var(--bf-outline, #85736a);
  border-radius: 8px;
  box-shadow: none;
  height: 62px;
  padding: 0;
  transition: all 0.2s ease;
}

.login-totp__cell :deep(.el-input__wrapper:focus-within) {
  border-bottom-color: var(--bf-primary-container, #ff8201);
  background: rgba(255, 255, 255, 0.9);
  box-shadow: 0 0 0 3px color-mix(in srgb, var(--bf-primary-container, #ff8201) 25%, transparent);
}

.login-totp__cell :deep(.el-input__inner) {
  text-align: center;
  font-size: 1.75rem;
  font-weight: 700;
  letter-spacing: 0.05em;
  font-variant-numeric: tabular-nums;
  padding: 0;
  height: 100%;
}

.login-totp__actions {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 0.75rem;
  width: 100%;
  margin-top: 0.5rem;
}

.login-totp__back-btn {
  appearance: none;
  background: rgba(255, 255, 255, 0.6);
  border: 1px solid rgba(0, 0, 0, 0.08);
  border-radius: 8px;
  padding: 10px 18px;
  font: inherit;
  font-weight: 500;
  color: var(--bf-on-surface, #221a11);
  cursor: pointer;
  transition: background 150ms ease;
}

.login-totp__back-btn:hover {
  background: rgba(255, 255, 255, 0.85);
}

.login-totp__submit {
  width: 100%;
  font-weight: 700;
}
</style>
