/**
 * Tauri IPC thin wrapper — single funnel for every command call.
 *
 * The auto-generated `bindings.ts` already provides type-safe
 * `commands.{name}(...)` functions, but every fallible command returns
 * `Promise<Result<T, CommandError>>` (tagged union) which forces every
 * caller to write the same `if (result.status === 'ok')` boilerplate.
 * `wrapCommand` collapses that boilerplate into a single line:
 *
 * ```ts
 * const session = await wrapCommand(commands.loginRegular(region, account, password))
 * ```
 *
 * On `status === 'ok'` the unwrapped data is returned; on
 * `status === 'error'` the wrapper performs three side effects in
 * order (P11-Q2 = B):
 *
 * 1. `console.error` the structured error (always — debug aid).
 * 2. If `code === 'auth.session_required'` and a global handler is
 *    registered, call it (boot-time hook lets the router redirect to
 *    the login page without every store action knowing about routing).
 * 3. Surface a localized error toast via `ElMessage.error` unless
 *    `silent: true` is passed; the message text comes from the
 *    registered translator (resolves `errors.{code}`) with a fallback
 *    to the backend's `error.message` if no translator is registered
 *    yet (boot ordering: invoke.ts is imported before i18n init).
 *
 * Then it throws {@link CommandInvocationError} so callers can choose
 * to catch (e.g. retry / continue) or let it bubble.
 *
 * For the rare caller that genuinely needs to inspect both branches
 * without `try`/`catch`, use {@link safeInvoke} which mirrors Rust's
 * `Result` shape verbatim.
 *
 * # Why the wrapper takes an in-flight `Promise`, not a function
 *
 * Callers write `wrapCommand(commands.loginRegular(...))` rather than
 * `wrapCommand(() => commands.loginRegular(...))`. The eager form is
 * shorter, lines up with `await` syntax naturally, and matches how
 * fetch wrappers are typically written in the Vue/Pinia ecosystem.
 * The wrapper does not need to retry or defer execution; if it ever
 * does, that is a different helper.
 */

import type { CommandError, Result } from '../types/bindings'
import { ElMessage } from 'element-plus'

/**
 * Per-call overrides for {@link wrapCommand}.
 *
 * Defaults to all-false — every error becomes a toast unless the
 * caller opts out for a specific reason (e.g. a polling loop that
 * intentionally swallows transient failures).
 */
export interface WrapOptions {
  /**
   * Skip the `ElMessage.error` toast. Useful when the caller renders
   * its own error UI (e.g. inline form validation) or when the
   * command is expected to fail as part of normal flow (e.g.
   * `login_qr_check` returning `qr.expired`).
   *
   * The console log and the `auth.session_required` hook still fire
   * — they are operational concerns, not user-facing UI.
   */
  silent?: boolean
}

/**
 * Thrown by {@link wrapCommand} when the underlying command returns
 * `Result::Err`. Wraps the structured {@link CommandError} so callers
 * can branch on `error.cause.code` without re-parsing the message.
 */
export class CommandInvocationError extends Error {
  constructor(public readonly cause: CommandError) {
    super(cause.message)
    this.name = 'CommandInvocationError'
  }
}

/**
 * The discriminated-union shape returned by {@link safeInvoke},
 * mirroring Rust's `Result<T, CommandError>` with a property-name
 * convention idiomatic for TypeScript consumers (`ok` instead of
 * `status: 'ok'`).
 */
export type SafeResult<T> = { ok: true; data: T } | { ok: false; error: CommandError }

type Translator = (code: string, fallback: string) => string
type SessionExpiredHandler = (error: CommandError) => void

let translator: Translator = (_code, fallback) => fallback
let sessionExpiredHandler: SessionExpiredHandler | null = null

/**
 * Register the function that converts a `<domain>.<variant>` code
 * into a localized string. Called once during boot after
 * `vue-i18n` is initialized; until then the wrapper falls back to
 * the backend-supplied English `error.message`.
 *
 * The translator receives both the i18n key (e.g.
 * `errors.auth.session_required`) and the fallback string, and
 * should return the fallback if the key is missing so we never
 * surface raw key strings to the user.
 */
export function registerErrorTranslator(t: Translator): void {
  translator = t
}

/**
 * Register the boot-time handler that fires when any command surfaces
 * `auth.session_required`. Pass `null` to clear (mainly for tests).
 *
 * The handler runs *before* the toast and *before* the throw, so it
 * can short-circuit the user-visible toast by mutating router state
 * (the throw still happens — the handler is a side-effect hook, not
 * a recovery path).
 *
 * Only one handler is supported (single-trust-boundary: there is
 * exactly one router in the app).
 */
export function registerSessionExpiredHandler(handler: SessionExpiredHandler | null): void {
  sessionExpiredHandler = handler
}

/**
 * Run the standard error side effects (console log, optional toast,
 * `auth.session_required` hook) without throwing. Exported so callers
 * that intentionally use {@link safeInvoke} for branch-on-code
 * dispatch (e.g. the auth store's `loginRegular` flow that swallows
 * `auth.totp_required` / `auth.verify_required` and re-routes the UI)
 * can still surface *unhandled* errors via the same pipeline as
 * {@link wrapCommand} — without duplicating the ElMessage import or
 * the translator lookup.
 */
export function surfaceCommandError(error: CommandError, opts: WrapOptions = {}): void {
  console.error(`[invoke] ${error.code}: ${error.message}`, error.details)

  if (error.code === 'auth.session_required' && sessionExpiredHandler) {
    try {
      sessionExpiredHandler(error)
    } catch (handlerError) {
      console.error('[invoke] session expired handler threw', handlerError)
    }
  }

  if (!opts.silent) {
    const localized = translator(`errors.${error.code}`, error.message)
    ElMessage.error(localized)
  }
}

/**
 * Unwrap a `Promise<Result<T, CommandError>>` into a `Promise<T>`,
 * routing failures through console + toast + the session-expired
 * hook. See module docs for the full design rationale.
 *
 * @throws {CommandInvocationError} when the command returns
 *   `Result::Err`.
 */
export async function wrapCommand<T>(
  invocation: Promise<Result<T, CommandError>>,
  opts: WrapOptions = {},
): Promise<T> {
  const result = await invocation
  if (result.status === 'ok') return result.data
  surfaceCommandError(result.error, opts)
  throw new CommandInvocationError(result.error)
}

/**
 * Non-throwing variant of {@link wrapCommand}. Returns the discriminated
 * union directly so the caller can branch without `try`/`catch`.
 *
 * Performs **none** of the side effects (no console log, no toast, no
 * session-expired hook) — `safeInvoke` is the explicit "I will handle
 * everything myself" escape hatch. Most call sites should use
 * {@link wrapCommand}; reach for `safeInvoke` only when the failure
 * is part of normal control flow (e.g. polling for a deferred result).
 */
export async function safeInvoke<T>(
  invocation: Promise<Result<T, CommandError>>,
): Promise<SafeResult<T>> {
  const result = await invocation
  if (result.status === 'ok') return { ok: true, data: result.data }
  return { ok: false, error: result.error }
}

/**
 * Test-only: reset the module-level registries to their defaults.
 *
 * Vitest's module isolation does not unload `invoke.ts` between
 * `it` blocks within the same test file, so handlers registered in
 * one test would leak into the next. Tests call this in
 * `beforeEach`/`afterEach` to keep them independent.
 *
 * Not exported through the package surface (consumers should never
 * touch this in production code), but kept inside the module so the
 * test does not need to reach into private state.
 */
export function __resetInvokeRegistriesForTesting(): void {
  translator = (_code, fallback) => fallback
  sessionExpiredHandler = null
}
