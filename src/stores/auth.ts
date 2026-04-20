/**
 * Auth store — session + login-flow state machine.
 *
 * # Scope (P11 Q4 = A: 4-store layout)
 *
 * Owns:
 *
 * - The current Beanfun session (`SessionInfo | null`) — the single
 *   piece of "am I logged in?" truth used by route guards and the
 *   header user chip.
 * - The transient login-flow sub-state — `pendingTotp` /
 *   `pendingVerify` flags that tell the UI to swap from the regular
 *   login form to a TOTP / verify page, plus the active QR challenge
 *   payload (`qrChallenge`) when the user picked QR-mode login.
 * - A single `pendingAction` slot that prevents the user from firing
 *   the same action twice (e.g. double-click on the "Login" button).
 *
 * # Why not split into `auth` + `loginFlow` stores
 *
 * The flow state and the session are tightly coupled — every
 * successful login transition mutates both at once
 * (`loginRegular` resolves → `session = …, pendingTotp = false,
 * pendingVerify = false`). Splitting would require either a Pinia
 * cross-store action or duplicate event wiring. Keeping them
 * together aligns with backend [`AppState::auth`] +
 * [`AppState::pending_*`] living in a single `Mutex` for the same
 * reason.
 *
 * # Why `safeInvoke` for the login actions
 *
 * `loginRegular` and `loginTotp` are special: certain backend
 * `CommandError::code` values (`auth.totp_required`,
 * `auth.advance_check_required`) are *not* user-facing failures —
 * they're "the server says we need a second factor". The store catches
 * those codes via {@link safeInvoke}, sets the corresponding
 * pending flag, and **returns `null`** instead of throwing /
 * toasting. Every other error is funneled through
 * {@link surfaceCommandError} so the user sees the same toast they
 * would for any other command failure.
 *
 * # Wire-string contract: `auth.advance_check_required`
 *
 * The flow-continuation code for the AdvanceCheck (captcha + extra
 * auth info) branch is the **backend's** `LoginError::AdvanceCheckRequired`
 * mapping (`commands/error.rs` L393-396 → `"auth.advance_check_required"`).
 * P10.2 originally exposed it via the `VerifyRequired` constant under
 * the wrong wire string (`auth.verify_required`), which made the
 * `pendingVerify` branch silently unreachable in production despite
 * mock-driven unit tests passing. P12.1 D8 pre-flight retired the bad
 * spelling and renamed the constant to {@link AdvanceCheckRequired}
 * to match the backend SSOT byte-for-byte. The error's `details.url`
 * field carries the TW `AccountLogin resultCode=2` AdvanceCheck URL
 * (HK paths leave it `null` → backend falls back to the static TW
 * URL inside `get_verify_page_info`); we capture it on
 * {@link useAuthStore.advanceCheckUrl} so {@link useAuthStore.getVerifyPageInfo}
 * can pass it through without `VerifyPage.vue` having to dig into
 * the raw `CommandError.details` shape.
 */

import { computed, ref } from 'vue'
import { defineStore } from 'pinia'

import {
  CommandInvocationError,
  safeInvoke,
  type SafeResult,
  surfaceCommandError,
  wrapCommand,
} from '../services/invoke'
import { commands } from '../types/bindings'
import type {
  LoginRegion,
  QrStart,
  QrStatus,
  SessionInfo,
  VerifyCaptcha,
  VerifyPage,
  VerifySubmit,
} from '../types/bindings'

/**
 * Transient form snapshot stashed by `IdPassForm` immediately
 * before calling {@link useAuthStore.loginRegular}, so subsequent
 * login-flow pages can read back the user's inputs without
 * shipping a plaintext password through `Config.xml` or query
 * params.
 *
 * # Lifetimes
 *
 * - **Set** by `IdPassForm.vue::submit` right before the
 *   `loginRegular` IPC call (so the slot is populated regardless
 *   of whether the call resolves to `SessionInfo`, `null` +
 *   pending flag, or throws — see WPF parity matrix in
 *   `IdPassForm.vue` docblock).
 * - **Read** by `LoginTotp.vue` (TOTP success path needs the
 *   original region / account / password to call
 *   `account.saveLoginCredentials`), `VerifyPage.vue` (mount
 *   prefill of stored verify code uses `region` + `accountId` to
 *   find the stored record), and `IdPassForm.vue` (second-pass
 *   success after the verify round-trip combines this with
 *   {@link VerifyIntent}).
 * - **Cleared** by {@link useAuthStore.clearLoginIntent} after a
 *   credential save fires (single-shot semantics — a stale
 *   intent must never silently feed into a *later* unrelated
 *   submit), by {@link useAuthStore.clearSession} (logout / 401
 *   bridge), and on every fresh `setLoginIntent` (always
 *   overwrite, never append).
 *
 * # Why a store slot rather than route params or component refs
 *
 * - **Route params**: would expose the password as a string in
 *   the URL and the browser back/forward stack — not just bad
 *   UX, an actual leak surface.
 * - **Component refs**: WPF kept `IdPassForm` mounted in a
 *   `ContentControl` so `t_AccountID.Text` / `t_Password.Password`
 *   stayed alive across the verify round-trip. Vue's router
 *   unmounts the form when navigating to `/login/verify`, so the
 *   refs would be `null` by the time `VerifyPage` mounts. A
 *   store-level slot is the SPA equivalent of WPF's "control
 *   stays alive" assumption.
 *
 * Mirrors WPF `MainWindow.xaml.cs::SaveLoginCredentials` reading
 * `idPassForm.t_AccountID.Text`, `idPassForm.t_Password.Password`,
 * `idPassForm.checkBox_RememberPWD.IsChecked`,
 * `idPassForm.checkBox_AutoLogin.IsChecked` (L1344-1360).
 */
export interface LoginIntent {
  region: LoginRegion
  accountId: string
  password: string
  rememberPassword: boolean
  autoLogin: boolean
}

/**
 * Transient verify-form snapshot stashed by `VerifyPage` on
 * successful `submitVerify`, consumed once by `IdPassForm`'s
 * second-pass `loginRegular` success path so the saved record
 * gets `verify` filled per the user's `RememberVerify` choice.
 *
 * Same lifetime pattern as {@link LoginIntent} — single-shot,
 * cleared by {@link useAuthStore.clearVerifyIntent} after
 * persistence and by {@link useAuthStore.clearSession}.
 *
 * Mirrors WPF `MainWindow.xaml.cs::SaveLoginCredentials` reading
 * `verifyPage.checkBoxRememberVerify.IsChecked` /
 * `verifyPage.t_Verify.Text` (L1357).
 */
export interface VerifyIntent {
  code: string
  remember: boolean
}

/** Codes the auth flow swallows instead of toasting. */
const FLOW_CONTINUATION_CODES = {
  TotpRequired: 'auth.totp_required',
  AdvanceCheckRequired: 'auth.advance_check_required',
} as const

/**
 * Pull the AdvanceCheck URL out of an `auth.advance_check_required`
 * error's `details` payload.
 *
 * Backend shape: `details: { url: string | null }` (see
 * `commands/error.rs::From<LoginError> for CommandError`,
 * `LoginError::AdvanceCheckRequired { url } => with_details(json!({ "url": url }))`).
 * TW `AccountLogin resultCode=2` populates the URL; HK paths leave it
 * `null` and the backend's `get_verify_page_info` falls back to the
 * static TW URL.
 *
 * Returned `null` on any shape mismatch (defence-in-depth — backend
 * contract is explicit, but we don't want to hard-fail the flow if
 * a future backend change drops the field).
 */
function readAdvanceCheckUrl(details: unknown): string | null {
  if (details === null || typeof details !== 'object') return null
  const url = (details as { url?: unknown }).url
  return typeof url === 'string' ? url : null
}

/**
 * Symbolic action ids used by {@link useAuthStore.pendingAction}.
 * Exported so view components can compare against them when
 * rendering per-button loading spinners.
 */
export const AUTH_ACTIONS = {
  LoginRegular: 'login.regular',
  LoginTotp: 'login.totp',
  LoginQrStart: 'login.qr_start',
  LoginQrCheck: 'login.qr_check',
  LoginGamepassStart: 'login.gamepass_start',
  GetVerifyPageInfo: 'verify.page_info',
  GetVerifyCaptcha: 'verify.captcha',
  SubmitVerify: 'verify.submit',
  Logout: 'auth.logout',
} as const

export type AuthAction = (typeof AUTH_ACTIONS)[keyof typeof AUTH_ACTIONS]

export const useAuthStore = defineStore('auth', () => {
  const session = ref<SessionInfo | null>(null)
  const pendingTotp = ref(false)
  const pendingVerify = ref(false)
  const qrChallenge = ref<QrStart | null>(null)
  const pendingAction = ref<AuthAction | null>(null)

  /**
   * AdvanceCheck URL extracted from the most recent
   * `auth.advance_check_required` error's `details.url`.
   *
   * - TW `AccountLogin resultCode=2` populates this with a session-
   *   scoped URL.
   * - HK paths leave it `null`; the backend `get_verify_page_info`
   *   command falls back to the static TW URL when given `null`,
   *   matching WPF `BeanfunClient.Verify.cs` L23-25.
   *
   * `VerifyPage.vue` passes this straight through to
   * {@link getVerifyPageInfo}. Cleared on:
   * - successful login (regular / TOTP / QR / GamePass — anything
   *   that mints a session implies the AdvanceCheck slot is no
   *   longer relevant), and
   * - {@link submitVerify} returning `success`, and
   * - {@link logout}.
   */
  const advanceCheckUrl = ref<string | null>(null)

  /**
   * Transient `IdPassForm` submission snapshot.
   *
   * See {@link LoginIntent} for the full lifecycle docblock.
   * Mutated only via {@link setLoginIntent} / {@link clearLoginIntent}
   * / {@link clearSession} so call sites grep cleanly.
   */
  const loginIntent = ref<LoginIntent | null>(null)

  /**
   * Transient `VerifyPage` submission snapshot.
   *
   * See {@link VerifyIntent} for the full lifecycle docblock.
   * Mutated only via {@link setVerifyIntent} / {@link clearVerifyIntent}
   * / {@link clearSession} so call sites grep cleanly.
   */
  const verifyIntent = ref<VerifyIntent | null>(null)

  const isLoggedIn = computed(() => session.value !== null)

  /**
   * Run `fn` while marking `action` as in-flight. Throws an Error
   * synchronously if the same action is already running so callers
   * (or the v-loading binding on a button) can short-circuit.
   *
   * Single-slot semantics: only one auth action can be in flight at
   * a time, mirroring backend `AppState::auth_lock` behavior. Any
   * concurrent attempt — same action *or* different action — is
   * rejected.
   */
  async function withGuard<T>(action: AuthAction, fn: () => Promise<T>): Promise<T> {
    if (pendingAction.value !== null) {
      throw new Error(
        `auth: another action is already in progress (${pendingAction.value}); rejected ${action}`,
      )
    }
    pendingAction.value = action
    try {
      return await fn()
    } finally {
      pendingAction.value = null
    }
  }

  /**
   * Attempt regular (account + password) login.
   *
   * @returns `SessionInfo` on full success, or `null` when the
   *   server signaled that a second factor is required (TOTP or
   *   verify). Inspect {@link pendingTotp} / {@link pendingVerify}
   *   to decide which UI to show next.
   * @throws {CommandInvocationError} for every other error code
   *   (already toasted via {@link surfaceCommandError}).
   */
  async function loginRegular(
    region: LoginRegion,
    account: string,
    password: string,
  ): Promise<SessionInfo | null> {
    return withGuard(AUTH_ACTIONS.LoginRegular, async () => {
      const result = await safeInvoke(commands.loginRegular(region, account, password))
      if (result.ok) {
        session.value = result.data
        pendingTotp.value = false
        pendingVerify.value = false
        qrChallenge.value = null
        advanceCheckUrl.value = null
        return result.data
      }
      if (result.error.code === FLOW_CONTINUATION_CODES.TotpRequired) {
        pendingTotp.value = true
        return null
      }
      if (result.error.code === FLOW_CONTINUATION_CODES.AdvanceCheckRequired) {
        pendingVerify.value = true
        advanceCheckUrl.value = readAdvanceCheckUrl(result.error.details)
        return null
      }
      surfaceCommandError(result.error)
      throw new CommandInvocationError(result.error)
    })
  }

  /**
   * Submit a TOTP code for an in-flight regular login.
   *
   * Backend may still require a verify step after a successful TOTP
   * exchange (rare but observed in WPF) — same null-on-flow-continuation
   * convention as {@link loginRegular}.
   */
  async function loginTotp(code: string): Promise<SessionInfo | null> {
    return withGuard(AUTH_ACTIONS.LoginTotp, async () => {
      const result = await safeInvoke(commands.loginTotp(code))
      if (result.ok) {
        session.value = result.data
        pendingTotp.value = false
        pendingVerify.value = false
        advanceCheckUrl.value = null
        return result.data
      }
      if (result.error.code === FLOW_CONTINUATION_CODES.AdvanceCheckRequired) {
        pendingTotp.value = false
        pendingVerify.value = true
        advanceCheckUrl.value = readAdvanceCheckUrl(result.error.details)
        return null
      }
      surfaceCommandError(result.error)
      throw new CommandInvocationError(result.error)
    })
  }

  /**
   * Start a QR-code login. Stores the bitmap + deeplink in
   * {@link qrChallenge} so multiple components (login page +
   * settings preview) can render the same challenge without
   * re-issuing the IPC call.
   */
  async function loginQrStart(region: LoginRegion): Promise<QrStart> {
    return withGuard(AUTH_ACTIONS.LoginQrStart, async () => {
      const challenge = await wrapCommand(commands.loginQrStart(region))
      qrChallenge.value = challenge
      return challenge
    })
  }

  /**
   * Poll the QR challenge. Returns a {@link SafeResult} so the caller's
   * poll loop can branch on both success (status dispatch) and failure
   * (stop polling, show inline error) without try/catch or duplicate
   * `ElMessage.error` toasts on every tick.
   *
   * On `ok: true` with `status === 'approved'` the store also updates
   * `session` and clears `qrChallenge` / pending flags; on `expired`
   * it clears `qrChallenge`. Errors are returned untouched — no
   * console / toast side effects — mirroring WPF
   * `MainWindow.qrCheckLogin_Tick` (L2358-2359) which silently disables
   * the timer on any non-zero result instead of surfacing a MessageBox.
   * The caller is expected to render an inline fallback and let the
   * user hit "Refresh" (WPF equivalent: the user clicks `btn_Refresh_QRCode`).
   */
  async function loginQrCheck(): Promise<SafeResult<QrStatus>> {
    return withGuard(AUTH_ACTIONS.LoginQrCheck, async () => {
      const result = await safeInvoke(commands.loginQrCheck())
      if (result.ok) {
        if (result.data.status === 'approved') {
          session.value = result.data.session
          qrChallenge.value = null
          pendingTotp.value = false
          pendingVerify.value = false
          advanceCheckUrl.value = null
        } else if (result.data.status === 'expired') {
          qrChallenge.value = null
        }
      }
      return result
    })
  }

  /**
   * Start a GamePass login. The command stashes a fresh
   * `BeanfunClient` + portal session key on the backend's
   * `pending_gamepass` slot; nothing useful comes back across the
   * IPC boundary (P10.2 Q4=C "no secrets over IPC"), so the action
   * resolves to `void`.
   *
   * The actual WebView window opening + cookie-driven completion
   * lives in P12.1 D5b CP3 (`open_gamepass_window` cmd + Tauri
   * `gamepass-login-success` / `gamepass-login-failed` events).
   * The caller (`GamepassForm`) is expected to advance its UI
   * progress tracker on resolve / inline-error on reject; errors
   * follow the same `wrapCommand` → toast path as `loginQrStart`.
   *
   * A backend refusal (`auth.gamepass_unsupported_region` — TW only)
   * is a defence-in-depth line; `GamepassForm` pre-flights on region
   * before invoking, so this branch is reachable only if a hostile
   * caller bypasses the UI.
   */
  async function loginGamepassStart(region: LoginRegion): Promise<void> {
    return withGuard(AUTH_ACTIONS.LoginGamepassStart, async () => {
      await wrapCommand(commands.loginGamepassStart(region))
    })
  }

  /**
   * Install a freshly-minted GamePass session on the store.
   *
   * # Why a store action rather than a direct `session.value = info`
   *
   * P12.1 D5b CP4 delivers the terminal session via the Tauri
   * `gamepass-login-success` event (not a command Promise), so the
   * normal `wrapCommand` → inline-mutation path used by
   * {@link loginRegular} / {@link loginQrCheck} doesn't apply. This
   * action replicates the exact 4-field post-success mutation
   * (`session` + clear `pendingTotp` / `pendingVerify` /
   * `qrChallenge`) so the GamePass path leaves the store in the
   * same shape the other login flows do — SRP keeps every session
   * write inside the store module, DRY avoids duplicating the
   * clear-pending-flags block in the view layer.
   *
   * # Guard semantics
   *
   * Intentionally **not** wrapped in {@link withGuard}: the
   * `LoginGamepassStart` guard released long before this fires
   * (between `loginGamepassStart` Promise resolve and the
   * user-driven OAuth round-trip in the WebView), and the event
   * can arrive at arbitrary later time. Re-entering `withGuard`
   * would fight the guard slot rather than help.
   */
  function applyGamepassSession(info: SessionInfo): void {
    session.value = info
    pendingTotp.value = false
    pendingVerify.value = false
    qrChallenge.value = null
    advanceCheckUrl.value = null
  }

  async function getVerifyPageInfo(advanceCheckUrl: string | null): Promise<VerifyPage> {
    return withGuard(AUTH_ACTIONS.GetVerifyPageInfo, () =>
      wrapCommand(commands.getVerifyPageInfo(advanceCheckUrl)),
    )
  }

  async function getVerifyCaptcha(): Promise<VerifyCaptcha> {
    return withGuard(AUTH_ACTIONS.GetVerifyCaptcha, () => wrapCommand(commands.getVerifyCaptcha()))
  }

  /**
   * Submit a verify code + captcha. The store does not auto-resume
   * the prior login flow — the caller decides whether to retry
   * `loginRegular` (for the captcha-then-regular path) or display
   * the success / retry UI (`wrong_captcha`, `wrong_auth_info`,
   * `server_message`).
   *
   * On `result === 'success'` the store clears the
   * {@link pendingVerify} flag so the UI can switch back to the
   * normal login form.
   */
  async function submitVerify(verifyCode: string, captchaCode: string): Promise<VerifySubmit> {
    return withGuard(AUTH_ACTIONS.SubmitVerify, async () => {
      const r = await wrapCommand(commands.submitVerify(verifyCode, captchaCode))
      if (r.result === 'success') {
        pendingVerify.value = false
        advanceCheckUrl.value = null
      }
      return r
    })
  }

  /**
   * Wipe every piece of session-derived state without invoking any
   * backend command.
   *
   * Two callers in scope:
   *
   * 1. {@link logout} — uses this **after** a successful
   *    `commands.logout()` round-trip so the local-state-clear
   *    logic lives in exactly one place (DRY).
   * 2. The D10 router-level session-expired bridge
   *    (`router/index.ts::installRouterGuards`) — calls this
   *    **directly**, skipping the backend round-trip, because
   *    `auth.session_required` means the server already considers
   *    the session gone; calling `commands.logout()` would just
   *    fail with the same code and surface an extra toast.
   *
   * Skips the {@link withGuard} slot deliberately: the guard
   * exists to prevent **concurrent** in-flight actions, but
   * `clearSession` is a synchronous local mutation that cannot
   * race with itself or with anything else (Vue reactivity is
   * single-threaded). Holding the guard slot here would also
   * deadlock against the session-expired path firing while a
   * regular action is mid-flight (e.g. `loginRegular` returning
   * `auth.session_required` from a stale cookie).
   */
  function clearSession(): void {
    session.value = null
    pendingTotp.value = false
    pendingVerify.value = false
    qrChallenge.value = null
    advanceCheckUrl.value = null
    /*
     * Transient login-flow snapshots ({@link loginIntent} /
     * {@link verifyIntent}) are scoped to a single login attempt by
     * design — a follow-up sign-in must start from a clean slate
     * regardless of whether the previous attempt finished, errored,
     * or got pre-empted by a session-expired bridge. Wiping them
     * here keeps the "every reset path lives in one function" SRP
     * (the same rationale that already applies to `qrChallenge` /
     * `pendingTotp` / `pendingVerify`).
     */
    loginIntent.value = null
    verifyIntent.value = null
  }

  /**
   * Stash the IdPassForm submit inputs so downstream login-flow
   * pages can consume them. Always overwrites — never appends —
   * so a stale half-filled intent from a previous attempt cannot
   * leak through.
   */
  function setLoginIntent(intent: LoginIntent): void {
    loginIntent.value = intent
  }

  /** Wipe the {@link loginIntent} slot. Single-shot consume after save. */
  function clearLoginIntent(): void {
    loginIntent.value = null
  }

  /**
   * Stash the VerifyPage submit inputs so the next IdPassForm
   * second-pass save can fold the verify code into the stored
   * record. Same overwrite-always semantics as
   * {@link setLoginIntent}.
   */
  function setVerifyIntent(intent: VerifyIntent): void {
    verifyIntent.value = intent
  }

  /** Wipe the {@link verifyIntent} slot. Single-shot consume after save. */
  function clearVerifyIntent(): void {
    verifyIntent.value = null
  }

  async function logout(): Promise<void> {
    return withGuard(AUTH_ACTIONS.Logout, async () => {
      await wrapCommand(commands.logout())
      clearSession()
    })
  }

  return {
    session,
    isLoggedIn,
    pendingTotp,
    pendingVerify,
    qrChallenge,
    advanceCheckUrl,
    pendingAction,
    loginIntent,
    verifyIntent,

    loginRegular,
    loginTotp,
    loginQrStart,
    loginQrCheck,
    loginGamepassStart,
    applyGamepassSession,
    getVerifyPageInfo,
    getVerifyCaptcha,
    submitVerify,
    clearSession,
    logout,
    setLoginIntent,
    clearLoginIntent,
    setVerifyIntent,
    clearVerifyIntent,
  }
})
