/**
 * P12.1 D4 — QR login form behaviour.
 *
 * Scope locked down by this spec (matches WPF `qr_form.xaml(.cs)` +
 * `MainWindow.qrCheckLogin_Tick` L2340-2368):
 *
 * 1. Renders QR bitmap after mount-time `auth.loginQrStart`.
 * 2. HK pre-flight guard — redirects to `/login` + info toast without
 *    hitting the backend (backend would refuse with
 *    `auth.qr_unsupported_region`).
 * 3. `setTimeout`-recursive polling every 2s → `auth.loginQrCheck`.
 * 4. `pending` / `retry` status keeps polling silently (WPF
 *    `res == 0` path, `ResultMessage in { "Wait Login", "Failed" }`).
 * 5. `approved` status stops polling + pushes `/accounts`.
 * 6. `expired` status auto-refreshes via `loginQrStart` (WPF
 *    `qrCheckLogin_Tick` L2364-2367 `refreshQRCode()`).
 * 7. `loginQrCheck` error result stops polling + shows inline
 *    "connection lost" banner (Q11 = B — no toast, inline fallback).
 * 8. Refresh button re-mints the QR.
 * 9. Back button ("返回一般登入") navigates to `/login/id-pass` (NOT
 *    `/login?pick=1` — the button switches login mode within the same
 *    region, it does not re-pick region) and halts the polling loop.
 * 10. Copy deeplink success path uses `navigator.clipboard.writeText`
 *     + success toast.
 * 11. Copy deeplink button is disabled when the server returns no
 *     deeplink.
 * 12. Clipboard write rejection surfaces `CopyFailed` toast.
 * 13. `onBeforeUnmount` clears the polling timer.
 * 14. Locale switch re-renders the localized copy.
 */

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { flushPromises, mount } from '@vue/test-utils'
import { createMemoryHistory, createRouter } from 'vue-router'
import { defineComponent, h } from 'vue'

import type {
  CommandError,
  QrStart,
  QrStatus,
  Result,
  SessionInfo,
} from '../../../src/types/bindings'

/*
 * Hoisted spies for ElMessage variants. We assert against each level
 * (info / success / error) independently because the form uses all
 * three for different UX signals — bundling them would hide mis-level
 * toasts (e.g. surfacing an info-toned message as an error).
 */
const { elMessageInfo, elMessageSuccess, elMessageError } = vi.hoisted(() => ({
  elMessageInfo: vi.fn(),
  elMessageSuccess: vi.fn(),
  elMessageError: vi.fn(),
}))

vi.mock('element-plus', () => ({
  ElButton: defineComponent({
    name: 'ElButtonStub',
    props: {
      loading: { type: Boolean, default: false },
      disabled: { type: Boolean, default: false },
      type: { type: String, default: 'default' },
    },
    emits: ['click'],
    setup(props, { slots, emit, attrs }) {
      return () =>
        h(
          'button',
          {
            ...attrs,
            class: 'el-button-stub',
            type: 'button',
            disabled: props.disabled || props.loading,
            'data-loading': props.loading ? 'true' : 'false',
            onClick: (e: MouseEvent) => emit('click', e),
          },
          slots.default?.(),
        )
    },
  }),
  ElMessage: {
    info: elMessageInfo,
    success: elMessageSuccess,
    error: elMessageError,
    warning: vi.fn(),
  },
}))

vi.mock('../../../src/types/bindings', () => ({
  commands: {
    getAllConfig: vi.fn(),
    setConfig: vi.fn(),
    getConfigValue: vi.fn(),
    loginRegular: vi.fn(),
    loginTotp: vi.fn(),
    loginQrStart: vi.fn(),
    loginQrCheck: vi.fn(),
    getVerifyPageInfo: vi.fn(),
    getVerifyCaptcha: vi.fn(),
    submitVerify: vi.fn(),
    logout: vi.fn(),
    detectGamePath: vi.fn(),
    listGameProcesses: vi.fn(),
    killGameProcesses: vi.fn(),
    openUrl: vi.fn(),
    launchGame: vi.fn(),
  },
}))

/*
 * P12.4 followup-A D9 — stub the launcher composable so QrForm
 * `GameStart` tests don't have to seed game store + config + 5 IPC
 * mocks. We only assert that the button delegates correctly; the
 * composable's own spec covers the launch pipeline behaviour.
 */
const { runGameSpy } = vi.hoisted(() => ({ runGameSpy: vi.fn() }))

vi.mock('../../../src/composables/useGameLauncher', () => ({
  useGameLauncher: () => ({ runGame: runGameSpy }),
}))

import { commands } from '../../../src/types/bindings'
import QrForm from '../../../src/pages/QrForm.vue'
import { useConfigStore } from '../../../src/stores/config'
import { createAppI18n, i18nMessages, setLocale } from '../../../src/i18n'

const mockLoginQrStart = vi.mocked(commands.loginQrStart)
const mockLoginQrCheck = vi.mocked(commands.loginQrCheck)

const ok = <T>(data: T): Promise<Result<T, CommandError>> => Promise.resolve({ status: 'ok', data })

const err = (error: CommandError): Promise<Result<never, CommandError>> =>
  Promise.resolve({ status: 'error', error })

const CHALLENGE: QrStart = {
  bitmap_base64: 'data:image/png;base64,AAAA',
  deeplink: 'https://example.com/deeplink',
}

const CHALLENGE_NO_DEEPLINK: QrStart = {
  bitmap_base64: 'data:image/png;base64,BBBB',
  deeplink: null,
}

const SESSION: SessionInfo = {
  region: 'TW',
  account_id: 'alice',
  service_code: '610074',
  service_region: 'T9',
}

const STATUS_PENDING: QrStatus = { status: 'pending' }
const STATUS_RETRY: QrStatus = { status: 'retry' }
const STATUS_EXPIRED: QrStatus = { status: 'expired' }
const STATUS_APPROVED: QrStatus = { status: 'approved', session: SESSION }

/**
 * Drive the recursive `setTimeout(runPollTick, 2000)` loop forward
 * one tick. `advanceTimersByTimeAsync` both fires the pending timer
 * and awaits the microtasks the tick schedules, so a follow-up
 * `flushPromises` is redundant for sync assertions — but we still
 * call it when the tick chains into further async work (e.g.
 * `router.push` on approval) to ensure navigation settles.
 */
async function advancePoll(times = 1): Promise<void> {
  for (let i = 0; i < times; i++) {
    await vi.advanceTimersByTimeAsync(2000)
  }
}

/**
 * Set up a memory router + i18n + optional config seed, then mount
 * QrForm inside `/login/qr`. Mirrors `IdPassForm.spec.ts` so the two
 * harnesses stay visually comparable.
 */
function mountForm(opts: { region?: string } = {}) {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: [
      { path: '/login', name: 'login', component: LoginStub },
      { path: '/login/qr', name: 'login-qr', component: QrForm },
      {
        path: '/login/id-pass',
        name: 'login-id-pass',
        component: defineComponent({
          name: 'IdPassStub',
          render: () => h('div', { 'data-testid': 'id-pass-stub' }),
        }),
      },
      {
        path: '/accounts',
        name: 'accounts',
        component: defineComponent({ name: 'AccountsStub', render: () => h('div') }),
      },
    ],
  })

  const i18n = createAppI18n()
  if (opts.region !== undefined) {
    const config = useConfigStore()
    config.entries['loginRegion'] = opts.region
  }

  return {
    router,
    i18n,
    async mountIt() {
      await router.push('/login/qr')
      await router.isReady()
      return mount(QrForm, {
        global: { plugins: [router, i18n] },
      })
    },
  }
}

const LoginStub = defineComponent({
  name: 'LoginStub',
  render: () => h('div', { class: 'login-stub' }),
})

/**
 * Restore `navigator.clipboard` between tests so a test that deletes
 * it doesn't bleed into the next. We hold the descriptor instead of
 * the value because the clipboard is a non-writable own property on
 * the real platform `navigator`.
 */
const ORIGINAL_CLIPBOARD = Object.getOwnPropertyDescriptor(navigator, 'clipboard')

function mockClipboard(writeText: (text: string) => Promise<void>): void {
  Object.defineProperty(navigator, 'clipboard', {
    configurable: true,
    value: { writeText: vi.fn(writeText) },
  })
}

describe('QrForm', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    vi.useFakeTimers()
    mockLoginQrStart.mockReset()
    mockLoginQrCheck.mockReset()
    elMessageInfo.mockReset()
    elMessageSuccess.mockReset()
    elMessageError.mockReset()
    runGameSpy.mockReset()
    mockClipboard(() => Promise.resolve())
  })

  afterEach(() => {
    vi.useRealTimers()
    if (ORIGINAL_CLIPBOARD) {
      Object.defineProperty(navigator, 'clipboard', ORIGINAL_CLIPBOARD)
    } else {
      // Node test runtime started without a clipboard shim — leave it absent.
      delete (navigator as { clipboard?: unknown }).clipboard
    }
  })

  it('calls loginQrStart on mount and renders the QR bitmap', async () => {
    mockLoginQrStart.mockReturnValueOnce(ok(CHALLENGE))
    mockLoginQrCheck.mockResolvedValue({ status: 'ok', data: STATUS_PENDING })

    const ctx = mountForm()
    const wrapper = await ctx.mountIt()
    await flushPromises()

    expect(mockLoginQrStart).toHaveBeenCalledWith('TW')
    expect(wrapper.find('[data-testid="qr-bitmap"]').exists()).toBe(true)
    expect(wrapper.find('[data-testid="qr-bitmap"]').attributes('src')).toBe(
      CHALLENGE.bitmap_base64,
    )
  })

  it('redirects HK-configured users to /login with an info toast (pre-flight guard)', async () => {
    const ctx = mountForm({ region: 'HK' })
    await ctx.mountIt()
    await flushPromises()

    expect(elMessageInfo).toHaveBeenCalledWith(i18nMessages['zh-TW'].loginQr.unsupportedHK)
    expect(ctx.router.currentRoute.value.path).toBe('/login')
    expect(mockLoginQrStart).not.toHaveBeenCalled()
  })

  it('polls loginQrCheck every 2 seconds after start resolves', async () => {
    mockLoginQrStart.mockReturnValueOnce(ok(CHALLENGE))
    mockLoginQrCheck.mockResolvedValue({ status: 'ok', data: STATUS_PENDING })

    const ctx = mountForm()
    await ctx.mountIt()
    await flushPromises()

    expect(mockLoginQrCheck).not.toHaveBeenCalled()

    await advancePoll(1)
    expect(mockLoginQrCheck).toHaveBeenCalledTimes(1)

    await advancePoll(2)
    expect(mockLoginQrCheck).toHaveBeenCalledTimes(3)
  })

  it('keeps polling silently on retry status (WPF Failed branch)', async () => {
    mockLoginQrStart.mockReturnValueOnce(ok(CHALLENGE))
    mockLoginQrCheck
      .mockReturnValueOnce(ok(STATUS_PENDING))
      .mockReturnValueOnce(ok(STATUS_RETRY))
      .mockReturnValue(ok(STATUS_PENDING))

    const ctx = mountForm()
    const wrapper = await ctx.mountIt()
    await flushPromises()

    await advancePoll(3)

    expect(mockLoginQrCheck).toHaveBeenCalledTimes(3)
    expect(wrapper.find('[data-testid="qr-connection-lost"]').exists()).toBe(false)
    expect(elMessageError).not.toHaveBeenCalled()
    expect(ctx.router.currentRoute.value.path).toBe('/login/qr')
  })

  it('pushes /accounts and halts polling on approved status', async () => {
    mockLoginQrStart.mockReturnValueOnce(ok(CHALLENGE))
    mockLoginQrCheck
      .mockReturnValueOnce(ok(STATUS_PENDING))
      .mockReturnValueOnce(ok(STATUS_APPROVED))
      .mockReturnValue(ok(STATUS_PENDING))

    const ctx = mountForm()
    await ctx.mountIt()
    await flushPromises()

    await advancePoll(2)
    await flushPromises()

    expect(ctx.router.currentRoute.value.path).toBe('/accounts')

    const callsAfterNav = mockLoginQrCheck.mock.calls.length
    await advancePoll(3)
    expect(mockLoginQrCheck.mock.calls.length).toBe(callsAfterNav)
  })

  it('auto-refreshes the QR on expired status (WPF qrCheckLogin_Tick res == -2)', async () => {
    const REFRESHED: QrStart = {
      bitmap_base64: 'data:image/png;base64,CCCC',
      deeplink: null,
    }
    mockLoginQrStart.mockReturnValueOnce(ok(CHALLENGE)).mockReturnValueOnce(ok(REFRESHED))
    mockLoginQrCheck.mockReturnValueOnce(ok(STATUS_EXPIRED)).mockReturnValue(ok(STATUS_PENDING))

    const ctx = mountForm()
    const wrapper = await ctx.mountIt()
    await flushPromises()

    expect(mockLoginQrStart).toHaveBeenCalledTimes(1)

    await advancePoll(1)
    await flushPromises()

    expect(mockLoginQrStart).toHaveBeenCalledTimes(2)
    expect(wrapper.find('[data-testid="qr-bitmap"]').attributes('src')).toBe(
      REFRESHED.bitmap_base64,
    )
  })

  it('stops polling and shows inline error when loginQrCheck returns an error', async () => {
    mockLoginQrStart.mockReturnValueOnce(ok(CHALLENGE))
    mockLoginQrCheck.mockReturnValueOnce(
      err({
        code: 'beanfun.qr_json_parse_failed',
        message: 'parse failed',
        details: null,
      }),
    )

    const ctx = mountForm()
    const wrapper = await ctx.mountIt()
    await flushPromises()

    await advancePoll(1)
    await flushPromises()

    expect(wrapper.find('[data-testid="qr-connection-lost"]').text()).toBe(
      i18nMessages['zh-TW'].loginQr.connectionLost,
    )
    expect(elMessageError).not.toHaveBeenCalled()

    const callsAfterError = mockLoginQrCheck.mock.calls.length
    await advancePoll(3)
    expect(mockLoginQrCheck.mock.calls.length).toBe(callsAfterError)
  })

  it('Refresh button re-issues loginQrStart and clears the connection-lost banner', async () => {
    const REFRESHED: QrStart = {
      bitmap_base64: 'data:image/png;base64,DDDD',
      deeplink: 'https://example.com/again',
    }
    mockLoginQrStart.mockReturnValueOnce(ok(CHALLENGE)).mockReturnValueOnce(ok(REFRESHED))
    mockLoginQrCheck.mockReturnValueOnce(
      err({ code: 'beanfun.transport', message: 'net', details: null }),
    )

    const ctx = mountForm()
    const wrapper = await ctx.mountIt()
    await flushPromises()

    await advancePoll(1)
    await flushPromises()
    expect(wrapper.find('[data-testid="qr-connection-lost"]').exists()).toBe(true)

    await wrapper.find('[data-testid="qr-refresh"]').trigger('click')
    await flushPromises()

    expect(mockLoginQrStart).toHaveBeenCalledTimes(2)
    expect(wrapper.find('[data-testid="qr-connection-lost"]').exists()).toBe(false)
    expect(wrapper.find('[data-testid="qr-bitmap"]').attributes('src')).toBe(
      REFRESHED.bitmap_base64,
    )
  })

  it('Back button ("返回一般登入") navigates to /login/id-pass and halts polling', async () => {
    /*
     * Regression for the bug where goBack pushed `/login?pick=1`,
     * which dumped the user back at the region picker even though
     * the button label promised "back to regular (id-pass) login".
     * The correct behaviour is mode-switch within the saved region.
     */
    mockLoginQrStart.mockReturnValueOnce(ok(CHALLENGE))
    mockLoginQrCheck.mockResolvedValue({ status: 'ok', data: STATUS_PENDING })

    const ctx = mountForm()
    const wrapper = await ctx.mountIt()
    await flushPromises()

    await wrapper.find('[data-testid="qr-back"]').trigger('click')
    await flushPromises()

    expect(ctx.router.currentRoute.value.path).toBe('/login/id-pass')

    const callsAfterBack = mockLoginQrCheck.mock.calls.length
    await advancePoll(3)
    expect(mockLoginQrCheck.mock.calls.length).toBe(callsAfterBack)
  })

  it('Copy Deeplink writes to clipboard and surfaces a success toast', async () => {
    mockLoginQrStart.mockReturnValueOnce(ok(CHALLENGE))
    mockLoginQrCheck.mockResolvedValue({ status: 'ok', data: STATUS_PENDING })

    const ctx = mountForm()
    const wrapper = await ctx.mountIt()
    await flushPromises()

    await wrapper.find('[data-testid="qr-copy-deeplink"]').trigger('click')
    await flushPromises()

    expect(navigator.clipboard.writeText).toHaveBeenCalledWith(CHALLENGE.deeplink)
    expect(elMessageSuccess).toHaveBeenCalledWith(i18nMessages['zh-TW'].CopyDeeplinkSuccess)
  })

  it('disables Copy Deeplink when the server returns no deeplink', async () => {
    mockLoginQrStart.mockReturnValueOnce(ok(CHALLENGE_NO_DEEPLINK))
    mockLoginQrCheck.mockResolvedValue({ status: 'ok', data: STATUS_PENDING })

    const ctx = mountForm()
    const wrapper = await ctx.mountIt()
    await flushPromises()

    const button = wrapper.find('[data-testid="qr-copy-deeplink"]')
    expect((button.element as HTMLButtonElement).disabled).toBe(true)
  })

  it('surfaces CopyFailed toast when navigator.clipboard throws', async () => {
    mockLoginQrStart.mockReturnValueOnce(ok(CHALLENGE))
    mockLoginQrCheck.mockResolvedValue({ status: 'ok', data: STATUS_PENDING })
    mockClipboard(() => Promise.reject(new Error('denied')))

    const ctx = mountForm()
    const wrapper = await ctx.mountIt()
    await flushPromises()

    await wrapper.find('[data-testid="qr-copy-deeplink"]').trigger('click')
    await flushPromises()

    expect(elMessageError).toHaveBeenCalledWith(i18nMessages['zh-TW'].CopyFailed)
    expect(elMessageSuccess).not.toHaveBeenCalled()
  })

  it('clears the poll timer on unmount so background ticks stop', async () => {
    mockLoginQrStart.mockReturnValueOnce(ok(CHALLENGE))
    mockLoginQrCheck.mockResolvedValue({ status: 'ok', data: STATUS_PENDING })

    const ctx = mountForm()
    const wrapper = await ctx.mountIt()
    await flushPromises()

    wrapper.unmount()
    await flushPromises()

    const callsAfterUnmount = mockLoginQrCheck.mock.calls.length
    await advancePoll(3)
    expect(mockLoginQrCheck.mock.calls.length).toBe(callsAfterUnmount)
  })

  it('re-renders localized copy after a runtime locale switch', async () => {
    mockLoginQrStart.mockReturnValueOnce(ok(CHALLENGE))
    mockLoginQrCheck.mockResolvedValue({ status: 'ok', data: STATUS_PENDING })

    const ctx = mountForm()
    const wrapper = await ctx.mountIt()
    await flushPromises()

    expect(wrapper.text()).toContain(i18nMessages['zh-TW'].loginQr.title)

    setLocale(ctx.i18n, 'en-US')
    await flushPromises()

    expect(wrapper.text()).toContain(i18nMessages['en-US'].loginQr.title)
  })

  /**
   * P12.4 followup-A D9 — GameStart parity (WPF
   * `qr_form.xaml.cs::btn_StartGame_Click` L84-87, a 3-line
   * `App.MainWnd.runGame()` call).
   *
   * The composable owns snapshot restoration + the empty-state
   * `GameSelected` toast, so this spec asserts only the
   * delegation path: click → `useGameLauncher().runGame()` with
   * no credential args, and no incidental network side-effects
   * (no `loginQrStart` re-fire / no `loginQrCheck` extra hits).
   */
  it('GameStart delegates to useGameLauncher().runGame() with no credentials', async () => {
    mockLoginQrStart.mockReturnValueOnce(ok(CHALLENGE))
    mockLoginQrCheck.mockResolvedValue({ status: 'ok', data: STATUS_PENDING })

    const ctx = mountForm()
    const wrapper = await ctx.mountIt()
    await flushPromises()

    const startCallsBefore = mockLoginQrStart.mock.calls.length
    await wrapper.find('[data-testid="qr-game-start"]').trigger('click')
    await flushPromises()

    expect(runGameSpy).toHaveBeenCalledTimes(1)
    expect(runGameSpy).toHaveBeenCalledWith()
    expect(mockLoginQrStart.mock.calls.length).toBe(startCallsBefore)
    expect(ctx.router.currentRoute.value.path).toBe('/login/qr')
  })
})
