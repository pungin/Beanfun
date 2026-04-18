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
 * `auth.verify_required`) are *not* user-facing failures — they're
 * "the server says we need a second factor". The store catches
 * those codes via {@link safeInvoke}, sets the corresponding
 * pending flag, and **returns `null`** instead of throwing /
 * toasting. Every other error is funneled through
 * {@link surfaceCommandError} so the user sees the same toast they
 * would for any other command failure.
 */

import { computed, ref } from 'vue'
import { defineStore } from 'pinia'

import {
  CommandInvocationError,
  safeInvoke,
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

/** Codes the auth flow swallows instead of toasting. */
const FLOW_CONTINUATION_CODES = {
  TotpRequired: 'auth.totp_required',
  VerifyRequired: 'auth.verify_required',
} as const

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
        return result.data
      }
      if (result.error.code === FLOW_CONTINUATION_CODES.TotpRequired) {
        pendingTotp.value = true
        return null
      }
      if (result.error.code === FLOW_CONTINUATION_CODES.VerifyRequired) {
        pendingVerify.value = true
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
        return result.data
      }
      if (result.error.code === FLOW_CONTINUATION_CODES.VerifyRequired) {
        pendingTotp.value = false
        pendingVerify.value = true
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
   * Poll the QR challenge. The store updates `session` to the
   * approved session payload when `status === 'approved'`, but
   * always returns the raw {@link QrStatus} so the caller's poll
   * loop can dispatch on `pending` / `retry` / `expired` /
   * `approved` directly.
   *
   * @returns `null` only when the call is rejected by `withGuard`
   *   (another action in flight) — otherwise the live status.
   */
  async function loginQrCheck(): Promise<QrStatus> {
    return withGuard(AUTH_ACTIONS.LoginQrCheck, async () => {
      const status = await wrapCommand(commands.loginQrCheck())
      if (status.status === 'approved') {
        session.value = status.session
        qrChallenge.value = null
        pendingTotp.value = false
        pendingVerify.value = false
      } else if (status.status === 'expired') {
        qrChallenge.value = null
      }
      return status
    })
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
      if (r.result === 'success') pendingVerify.value = false
      return r
    })
  }

  async function logout(): Promise<void> {
    return withGuard(AUTH_ACTIONS.Logout, async () => {
      await wrapCommand(commands.logout())
      session.value = null
      pendingTotp.value = false
      pendingVerify.value = false
      qrChallenge.value = null
    })
  }

  return {
    session,
    isLoggedIn,
    pendingTotp,
    pendingVerify,
    qrChallenge,
    pendingAction,

    loginRegular,
    loginTotp,
    loginQrStart,
    loginQrCheck,
    getVerifyPageInfo,
    getVerifyCaptcha,
    submitVerify,
    logout,
  }
})
