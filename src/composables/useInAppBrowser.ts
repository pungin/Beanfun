/**
 * In-app browser composable — single dispatch point for "open
 * `url` inside the embedded WebView2-equivalent window, falling
 * back to the system browser when the URL is outside the
 * backend allowlist".
 *
 * # WPF parity
 *
 * Mirrors the WPF `new WebBrowser(uri).Show()` call sites
 * (`Beanfun/Windows/WebBrowser.xaml(.cs)`, `Pages/IdPassForm`
 * RegisterAccount / ForgotPassword, `Pages/AccountList` Member
 * Center / Customer Service). The Tauri equivalent is the
 * `web_browser::open_in_app_browser` IPC command which builds a
 * fresh [`tauri::WebviewWindow`] per call (per-click new label,
 * matches WPF "every click instantiates a fresh window") and
 * pre-seeds the logged-in [`BeanfunClient`] cookies before
 * navigating to the target URL.
 *
 * # Why a composable (not a service / not inline)
 *
 * - **Reactive context** — `ElMessage` is a Vue context-only API
 *   (reads from the Element Plus context provider), which rules
 *   out a plain `services/*.ts` module. Wrapping in a composable
 *   keeps the `useI18n()` call inside Vue's reactive scope where
 *   it's guaranteed to resolve.
 * - **Stateless** — every call is a pure function of `(url, i18n)`.
 *   Multiple instantiations in the same component tree share zero
 *   local state by design (each consumer holds its own closure).
 *   Matches the same shape used by [`useGameLauncher`].
 * - **Single source of truth for fallback** — the host allowlist
 *   lives on the backend (`web_browser::ALLOWED_HOSTS`); this
 *   composable trusts the backend's `system.invalid_url` reject
 *   as the signal to fall back to `commands.openUrl` (system
 *   browser). Frontend never duplicates the host list (the
 *   placeholder `WebBrowser.vue` did, and immediately drifted
 *   from the WPF set — see followup-B B5 history).
 *
 * # Error handling decision matrix
 *
 * | Outcome of `commands.openInAppBrowser(url)`        | UX                                                                |
 * |----------------------------------------------------|-------------------------------------------------------------------|
 * | `ok`                                               | Window opens, no toast — same silence WPF `Show()` provides.      |
 * | `err.code === 'system.invalid_url'`                | Info toast `inAppBrowser.fallbackToSystem`, then `commands.openUrl`. |
 * | `err.code === 'ui.window_create_failed'`           | Error toast (Tauri builder failure — rare, surface to the user).  |
 * | other `err.code`                                   | Error toast via the shared [`surfaceCommandError`] pipeline.      |
 * | URL empty / not http(s)                            | Error toast `inAppBrowser.invalidUrl`, no IPC round-trip.         |
 *
 * Only `system.invalid_url` triggers the fallback — every other
 * failure is a real error worth surfacing. WPF's WebView2 chrome
 * has no equivalent silent-fallback for builder failures either.
 */

import { ElMessage } from 'element-plus'
import { useI18n } from 'vue-i18n'

import { commands } from '../types/bindings'
import { safeInvoke } from '../services/invoke'

/**
 * Backend `CommandError.code` returned when the requested URL is
 * malformed, uses a non-`https` scheme, or hits a host that is
 * not in `web_browser::ALLOWED_HOSTS`. Kept as a typed constant
 * so the test file can reference the exact same string the
 * backend emits.
 */
export const IN_APP_BROWSER_INVALID_URL_CODE = 'system.invalid_url'

/**
 * Public surface of the in-app browser composable. Single method
 * by design — every consumer needs the open-with-fallback chain
 * (no partial use cases discovered: callers either want the
 * embedded window or a system-browser fallback, never one without
 * the other).
 */
export interface UseInAppBrowserReturn {
  /**
   * Open `url` in the in-app browser window. Falls back to the
   * system browser when the backend rejects the URL with
   * `system.invalid_url` (host outside the allowlist).
   *
   * Returns `Promise<void>`; failures are toasted internally and
   * never thrown — call sites can fire-and-forget like the
   * existing `WebBrowser` self-mount pattern.
   */
  open: (url: string) => Promise<void>
}

export function useInAppBrowser(): UseInAppBrowserReturn {
  const { t } = useI18n()

  /**
   * Defensive client-side URL validation — saves a useless IPC
   * round-trip when the caller passes an empty / malformed URL
   * (e.g. a constants-table miss). The backend re-validates with
   * the full host allowlist; this shim only catches the obvious
   * "string is not a URL" cases that don't need the network.
   */
  function isPlausibleHttpsUrl(url: string): boolean {
    if (url === '') return false
    try {
      const parsed = new URL(url)
      return parsed.protocol === 'https:' || parsed.protocol === 'http:'
    } catch {
      return false
    }
  }

  async function open(url: string): Promise<void> {
    if (!isPlausibleHttpsUrl(url)) {
      ElMessage.error(t('inAppBrowser.invalidUrl'))
      return
    }

    const result = await safeInvoke(commands.openInAppBrowser(url))
    if (result.ok) return

    if (result.error.code === IN_APP_BROWSER_INVALID_URL_CODE) {
      /*
       * Host outside the backend allowlist — fall back to the
       * system browser so the user can still reach the page.
       * Mirrors WPF's "links to non-beanfun hosts go through
       * Process.Start(url) with UseShellExecute = true", which
       * is the same OS-level shell-open that `commands.openUrl`
       * funnels into.
       */
      ElMessage.info(t('inAppBrowser.fallbackToSystem'))
      const fallback = await safeInvoke(commands.openUrl(url))
      if (!fallback.ok) {
        ElMessage.error(fallback.error.message)
      }
      return
    }

    /*
     * Any other backend error is a real failure (most likely
     * `ui.window_create_failed` from the Tauri builder). Surface
     * the localized message; we deliberately bypass the global
     * `wrapCommand` toast pipeline because the caller is fire-
     * and-forget and a thrown `CommandInvocationError` here would
     * be swallowed by the void-returning button handlers.
     */
    ElMessage.error(result.error.message || t('inAppBrowser.openFailed'))
  }

  return { open }
}
