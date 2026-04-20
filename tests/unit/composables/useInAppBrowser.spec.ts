/**
 * P12.4 followup-B B9 — `useInAppBrowser` composable behaviour.
 *
 * The composable is a thin IPC wrapper around
 * `commands.openInAppBrowser` (followup-B B2 backend, B3
 * binding) with a single fallback rule: `system.invalid_url`
 * from the backend means "host outside the allowlist" → fall
 * back to `commands.openUrl` (system browser). Any other error
 * is a real failure surfaced via `ElMessage.error`.
 *
 * What this spec locks down:
 *
 * 1. Allowed host → `commands.openInAppBrowser` called once,
 *    no fallback IPC, no toast.
 * 2. Disallowed host (backend `system.invalid_url`) → info toast
 *    with the `inAppBrowser.fallbackToSystem` localized string,
 *    then `commands.openUrl` called with the same URL.
 * 3. Other backend error code (e.g. `ui.window_create_failed`)
 *    → error toast with the backend's `message`, no fallback.
 * 4. Empty / non-http URL → error toast with the
 *    `inAppBrowser.invalidUrl` string, no IPC fired.
 *
 * # Test harness
 *
 * Composables that touch `useI18n` need a Vue scope. We mount a
 * tiny host component that calls `useInAppBrowser` in setup()
 * and exposes the returned `open` via a ref so each test can
 * call it imperatively without a real consumer page.
 */

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { mount } from '@vue/test-utils'
import { defineComponent, h, ref } from 'vue'

import type { CommandError, Result } from '../../../src/types/bindings'

const { elMessageError, elMessageInfo } = vi.hoisted(() => ({
  elMessageError: vi.fn(),
  elMessageInfo: vi.fn(),
}))

vi.mock('element-plus', () => ({
  ElMessage: {
    error: elMessageError,
    info: elMessageInfo,
    warning: vi.fn(),
    success: vi.fn(),
  },
}))

vi.mock('../../../src/types/bindings', () => ({
  commands: {
    openInAppBrowser: vi.fn(),
    openUrl: vi.fn(),
  },
}))

import { commands } from '../../../src/types/bindings'
import { useInAppBrowser } from '../../../src/composables/useInAppBrowser'
import { createAppI18n, i18nMessages } from '../../../src/i18n'

const mockOpenInAppBrowser = vi.mocked(commands.openInAppBrowser)
const mockOpenUrl = vi.mocked(commands.openUrl)

const ok = <T>(data: T): Promise<Result<T, CommandError>> => Promise.resolve({ status: 'ok', data })
const err = (error: CommandError): Promise<Result<never, CommandError>> =>
  Promise.resolve({ status: 'error', error })

const ALLOWED_URL = 'https://tw.beanfun.com/member/forgot_pwd.aspx'
const DISALLOWED_URL = 'https://event.beanfun.com/customerservice/PluginReporting/PlayerReport.aspx'

/**
 * Mount a host that instantiates the composable. i18n is
 * required so `useI18n` resolves; pinia is set up too because
 * other composables imported transitively might initialise
 * stores during boot (defensive parity with the
 * `useGameLauncher` harness).
 */
function mountHarness(): { open: (url: string) => Promise<void> } {
  const i18n = createAppI18n()
  const openRef = ref<((url: string) => Promise<void>) | null>(null)

  const Host = defineComponent({
    name: 'InAppBrowserHost',
    setup() {
      const browser = useInAppBrowser()
      openRef.value = browser.open
      return () => h('div')
    },
  })

  mount(Host, { global: { plugins: [i18n] } })

  return { open: (url) => openRef.value!(url) }
}

describe('useInAppBrowser', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    mockOpenInAppBrowser.mockReset()
    mockOpenUrl.mockReset()
    elMessageError.mockReset()
    elMessageInfo.mockReset()
  })

  afterEach(() => {
    vi.restoreAllMocks()
  })

  it('routes allowed-host URL through openInAppBrowser without fallback or toast', async () => {
    /*
     * The `tw.beanfun.com` host is inside the backend
     * `web_browser::ALLOWED_HOSTS` allowlist, so the IPC must
     * resolve `ok` and the composable must NOT call `openUrl`
     * (no system-browser fallback) or surface any toast.
     */
    mockOpenInAppBrowser.mockReturnValueOnce(ok(null))

    const { open } = mountHarness()
    await open(ALLOWED_URL)

    expect(mockOpenInAppBrowser).toHaveBeenCalledTimes(1)
    expect(mockOpenInAppBrowser).toHaveBeenCalledWith(ALLOWED_URL)
    expect(mockOpenUrl).not.toHaveBeenCalled()
    expect(elMessageError).not.toHaveBeenCalled()
    expect(elMessageInfo).not.toHaveBeenCalled()
  })

  it('falls back to openUrl + info toast when backend rejects with system.invalid_url', async () => {
    /*
     * `event.beanfun.com` is outside the backend allowlist, so
     * the IPC will reject with `system.invalid_url`. The
     * composable surfaces the localized
     * `inAppBrowser.fallbackToSystem` info toast and re-routes
     * through `commands.openUrl` (system browser).
     */
    mockOpenInAppBrowser.mockReturnValueOnce(
      err({ code: 'system.invalid_url', message: 'host not allowed', details: null }),
    )
    mockOpenUrl.mockReturnValueOnce(ok(null))

    const { open } = mountHarness()
    await open(DISALLOWED_URL)

    expect(mockOpenInAppBrowser).toHaveBeenCalledTimes(1)
    expect(elMessageInfo).toHaveBeenCalledTimes(1)
    expect(elMessageInfo).toHaveBeenCalledWith(i18nMessages['zh-TW'].inAppBrowser.fallbackToSystem)
    expect(mockOpenUrl).toHaveBeenCalledTimes(1)
    expect(mockOpenUrl).toHaveBeenCalledWith(DISALLOWED_URL)
    expect(elMessageError).not.toHaveBeenCalled()
  })

  it('surfaces an error toast and does not fall back when backend returns ui.window_create_failed', async () => {
    /*
     * Any non-`system.invalid_url` failure is a real Tauri
     * builder error (e.g. WebView2 init failed). The composable
     * surfaces the backend's `message` directly — no fallback,
     * because the user already explicitly asked for the in-app
     * window and we'd be hiding the failure if we silently
     * popped the system browser instead.
     */
    mockOpenInAppBrowser.mockReturnValueOnce(
      err({
        code: 'ui.window_create_failed',
        message: 'Failed to create in-app browser window: x',
        details: null,
      }),
    )

    const { open } = mountHarness()
    await open(ALLOWED_URL)

    expect(mockOpenInAppBrowser).toHaveBeenCalledTimes(1)
    expect(mockOpenUrl).not.toHaveBeenCalled()
    expect(elMessageError).toHaveBeenCalledTimes(1)
    expect(elMessageError).toHaveBeenCalledWith('Failed to create in-app browser window: x')
    expect(elMessageInfo).not.toHaveBeenCalled()
  })

  it('rejects empty / non-http URLs locally without firing IPC', async () => {
    /*
     * Defensive shim: empty string and `javascript:` / `file:`
     * URLs are obvious "string is not a real URL" cases. Skip
     * the round-trip and toast the localized
     * `inAppBrowser.invalidUrl` so the user sees a clear
     * frontend-side validation message instead of a
     * confusingly-translated backend error.
     */
    const { open } = mountHarness()

    await open('')
    await open('javascript:alert(1)')
    await open('not-a-url')

    expect(mockOpenInAppBrowser).not.toHaveBeenCalled()
    expect(mockOpenUrl).not.toHaveBeenCalled()
    expect(elMessageError).toHaveBeenCalledTimes(3)
    expect(elMessageError).toHaveBeenLastCalledWith(i18nMessages['zh-TW'].inAppBrowser.invalidUrl)
  })
})
