/**
 * P12.1 D5b CP4 — GamePass login form behaviour (full-flow spec).
 *
 * Scope extends CP2's step-1 scaffold to the complete event-driven
 * lifecycle delivered by CP3's backend + CP4's frontend listener.
 *
 * CP2-era assertions (kept, with updated step expectations now that
 * `openGamepassWindow` fires immediately after `loginGamepassStart`):
 *
 * 1. `auth.loginGamepassStart` fires on mount (auto-start — mirrors
 *    the route-as-intent design; no "Open GamePass" click gate).
 * 2. HK pre-flight guard — redirects to `/login` + info toast
 *    without hitting either backend command.
 * 3. On `loginGamepassStart` + `openGamepassWindow` success,
 *    `<el-steps>` advances to `active=2` (`STEP_WINDOW_OPENED`).
 * 4. On a `loginGamepassStart` `CommandInvocationError`, the inline
 *    `connection-lost` banner shows and the steps stay at `0`.
 * 5. Refresh re-issues both commands and clears the banner on
 *    success.
 * 6. Back button ("返回一般登入") navigates to `/login/id-pass` (NOT
 *    `/login?pick=1` — the button switches login mode within the same
 *    region, it does not re-pick region) without further backend calls.
 * 7. Locale switch re-renders the localized copy.
 *
 * CP4 event-wiring assertions (new):
 *
 * 8. `gamepass-login-success` event → `applyGamepassSession(payload)`
 *    + navigates to `/accounts`.
 * 9. `gamepass-login-failed` event → `window-error` banner + step
 *    resets to `1` (STEP_PREPARED).
 * 10. `gamepass-login-cancelled` event → silent step reset to `0`
 *     (WPF parity, no banner / no toast).
 * 11. `openGamepassWindow` error → `window-error` banner (step stays
 *     at `1` so the user can Refresh).
 * 12. Unmount detaches every registered listener (success / failed /
 *     cancelled) — late events must not mutate a destroyed tree.
 */

import { afterEach, beforeEach, describe, expect, it, vi, type MockInstance } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { flushPromises, mount } from '@vue/test-utils'
import { createMemoryHistory, createRouter } from 'vue-router'
import { defineComponent, h } from 'vue'

import type { CommandError, Result, SessionInfo } from '../../../src/types/bindings'

/*
 * Hoisted spies for ElMessage variants — same pattern as
 * QrForm.spec.ts so the two harnesses stay visually comparable.
 * Bundling the three levels would hide mis-level toasts.
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
  ElInput: defineComponent({
    name: 'ElInputStub',
    props: {
      modelValue: { type: String, default: '' },
      type: { type: String, default: 'text' },
      placeholder: { type: String, default: '' },
      showPassword: { type: Boolean, default: false },
    },
    emits: ['update:modelValue'],
    setup(props, { attrs, emit }) {
      return () =>
        h('input', {
          ...attrs,
          class: 'el-input-stub',
          type: props.type,
          value: props.modelValue,
          placeholder: props.placeholder,
          onInput: (e: Event) => emit('update:modelValue', (e.target as HTMLInputElement).value),
        })
    },
  }),
  /*
   * ElSteps / ElStep stubs intentionally render the `:active` prop
   * onto a data attribute so the test can assert progress advances
   * without cracking into Element Plus internals. Mirrors the
   * transparent stub style the rest of the suite uses for
   * element-plus components.
   */
  ElSteps: defineComponent({
    name: 'ElStepsStub',
    props: {
      active: { type: Number, default: 0 },
      alignCenter: { type: Boolean, default: false },
      finishStatus: { type: String, default: 'finish' },
    },
    setup(props, { slots, attrs }) {
      return () =>
        h(
          'ol',
          {
            ...attrs,
            class: 'el-steps-stub',
            'data-active': String(props.active),
            'data-finish-status': props.finishStatus,
          },
          slots.default?.(),
        )
    },
  }),
  ElStep: defineComponent({
    name: 'ElStepStub',
    props: { title: { type: String, default: '' } },
    setup(props, { attrs }) {
      return () =>
        h('li', { ...attrs, class: 'el-step-stub', 'data-step-title': props.title }, props.title)
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
    loginGamepassStart: vi.fn(),
    openGamepassWindow: vi.fn(),
    getVerifyPageInfo: vi.fn(),
    getVerifyCaptcha: vi.fn(),
    submitVerify: vi.fn(),
    logout: vi.fn(),
  },
}))

/*
 * Record the callback + unlisten spy for each Tauri event the form
 * subscribes to. Tests trigger events by invoking
 * `eventListeners['gamepass-login-success']?.({ payload: ... })`
 * directly, and assert cleanup by checking
 * `eventUnlistenSpies['gamepass-login-success']` was called on
 * unmount.
 *
 * Hoisted via `vi.hoisted` so the `vi.mock` factory below can
 * reference them — `vi.mock` factories run before the surrounding
 * module's top-level code.
 */
interface TauriEvent<T> {
  event: string
  id: number
  payload: T
}
type EventCallback<T> = (event: TauriEvent<T>) => void | Promise<void>

const { eventListeners, eventUnlistenSpies, listenSpy } = vi.hoisted(() => ({
  eventListeners: {} as Record<string, EventCallback<unknown> | undefined>,
  eventUnlistenSpies: {} as Record<string, ReturnType<typeof vi.fn>>,
  listenSpy: vi.fn(),
}))

vi.mock('@tauri-apps/api/event', () => ({
  listen: listenSpy.mockImplementation((event: string, cb: EventCallback<unknown>) => {
    eventListeners[event] = cb
    const unlisten = vi.fn()
    eventUnlistenSpies[event] = unlisten
    return Promise.resolve(unlisten)
  }),
}))

import { commands } from '../../../src/types/bindings'
import GamepassForm from '../../../src/pages/GamepassForm.vue'
import { useAuthStore } from '../../../src/stores/auth'
import { useAccountStore } from '../../../src/stores/account'
import { useUiStore } from '../../../src/stores/ui'
import { useConfigStore } from '../../../src/stores/config'
import { createAppI18n, i18nMessages, setLocale } from '../../../src/i18n'

const mockLoginGamepassStart = vi.mocked(commands.loginGamepassStart)
const mockOpenGamepassWindow = vi.mocked(commands.openGamepassWindow)

const ok = (): Promise<Result<null, CommandError>> => Promise.resolve({ status: 'ok', data: null })

const err = (error: CommandError): Promise<Result<never, CommandError>> =>
  Promise.resolve({ status: 'error', error })

const SAMPLE_SESSION: SessionInfo = {
  region: 'TW',
  account_id: 'tester123',
  service_code: '610074',
  service_region: 'T9',
}

const LoginStub = defineComponent({
  name: 'LoginStub',
  render: () => h('div', { class: 'login-stub' }),
})

const AccountsStub = defineComponent({
  name: 'AccountsStub',
  render: () => h('div', { class: 'accounts-stub' }),
})

/**
 * Invoke a previously-registered Tauri event listener with the
 * canonical shape `@tauri-apps/api/event` delivers. Kept as a
 * helper so tests read at the "what happened" layer rather than
 * re-documenting the event envelope at every call site.
 */
async function fireEvent<T>(eventName: string, payload: T): Promise<void> {
  const cb = eventListeners[eventName] as EventCallback<T> | undefined
  if (!cb) {
    throw new Error(`No listener registered for ${eventName}`)
  }
  await cb({ event: eventName, id: Date.now(), payload })
  await flushPromises()
}

/**
 * Set up a memory router + i18n + optional config seed, then mount
 * GamepassForm inside `/login/gamepass`. Factored to keep each
 * `it()` body at the "arrange asserts" layer.
 */
function mountForm(opts: { region?: string } = {}) {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: [
      { path: '/login', name: 'login', component: LoginStub },
      { path: '/login/gamepass', name: 'login-gamepass', component: GamepassForm },
      {
        path: '/login/id-pass',
        name: 'login-id-pass',
        component: defineComponent({
          name: 'IdPassStub',
          render: () => h('div', { 'data-testid': 'id-pass-stub' }),
        }),
      },
      { path: '/accounts', name: 'accounts', component: AccountsStub },
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
      await router.push('/login/gamepass')
      await router.isReady()
      return mount(GamepassForm, {
        global: { plugins: [router, i18n] },
      })
    },
  }
}

describe('GamepassForm', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    mockLoginGamepassStart.mockReset()
    mockOpenGamepassWindow.mockReset()
    // Default: happy path for both commands. Individual tests
    // override with `.mockReturnValueOnce(err(...))` where needed.
    mockLoginGamepassStart.mockReturnValue(ok())
    mockOpenGamepassWindow.mockReturnValue(ok())
    elMessageInfo.mockReset()
    elMessageSuccess.mockReset()
    elMessageError.mockReset()
    // Wipe the listener / unlisten registries between tests so a
    // leaked callback from a prior test can't fire into the next.
    for (const key of Object.keys(eventListeners)) {
      delete eventListeners[key]
    }
    for (const key of Object.keys(eventUnlistenSpies)) {
      delete eventUnlistenSpies[key]
    }
    listenSpy.mockClear()
  })

  afterEach(() => {
    vi.useRealTimers()
  })

  it('arms the session and shows the credential form on mount (no window yet)', async () => {
    const ctx = mountForm()
    const wrapper = await ctx.mountIt()
    await flushPromises()

    expect(mockLoginGamepassStart).toHaveBeenCalledWith('TW')
    // The window opens only after the user submits the credential form.
    expect(mockOpenGamepassWindow).not.toHaveBeenCalled()
    expect(wrapper.find('[data-testid="gamepass-creds"]').exists()).toBe(true)
    expect(wrapper.find('[data-testid="gamepass-steps"]').exists()).toBe(false)
  })

  it('opens the window with the entered credentials on submit', async () => {
    const ctx = mountForm()
    const wrapper = await ctx.mountIt()
    await flushPromises()

    await wrapper.find('[data-testid="gamepass-account"]').setValue('0981504933')
    await wrapper.find('[data-testid="gamepass-password"]').setValue('pw123')
    await wrapper.find('[data-testid="gamepass-creds"]').trigger('submit')
    await flushPromises()

    expect(mockOpenGamepassWindow).toHaveBeenCalledWith('0981504933', 'pw123')
    expect(wrapper.find('[data-testid="gamepass-steps"]').attributes('data-active')).toBe('2')
    // Form swaps out for the step tracker once the window is open.
    expect(wrapper.find('[data-testid="gamepass-creds"]').exists()).toBe(false)
  })

  it('redirects HK-configured users to /login with an info toast (pre-flight guard)', async () => {
    const ctx = mountForm({ region: 'HK' })
    await ctx.mountIt()
    await flushPromises()

    expect(elMessageInfo).toHaveBeenCalledWith(i18nMessages['zh-TW'].loginGamepass.unsupportedHK)
    expect(ctx.router.currentRoute.value.path).toBe('/login')
    expect(mockLoginGamepassStart).not.toHaveBeenCalled()
    expect(mockOpenGamepassWindow).not.toHaveBeenCalled()
  })

  it('shows the connection-lost banner on loginGamepassStart failure (no form)', async () => {
    mockLoginGamepassStart.mockReset()
    mockLoginGamepassStart.mockReturnValueOnce(
      err({ code: 'beanfun.transport', message: 'net', details: null }),
    )

    const ctx = mountForm()
    const wrapper = await ctx.mountIt()
    await flushPromises()

    expect(wrapper.find('[data-testid="gamepass-connection-lost"]').text()).toBe(
      i18nMessages['zh-TW'].loginGamepass.connectionLost,
    )
    // Arm failed → session not prepared → credential form is not shown.
    expect(wrapper.find('[data-testid="gamepass-creds"]').exists()).toBe(false)
    expect(mockOpenGamepassWindow).not.toHaveBeenCalled()
  })

  it('shows the window-error banner on openGamepassWindow failure (form stays)', async () => {
    mockOpenGamepassWindow.mockReset()
    mockOpenGamepassWindow.mockReturnValueOnce(
      err({ code: 'ui.window_create_failed', message: 'fail', details: null }),
    )

    const ctx = mountForm()
    const wrapper = await ctx.mountIt()
    await flushPromises()
    await wrapper.find('[data-testid="gamepass-creds"]').trigger('submit')
    await flushPromises()

    expect(mockLoginGamepassStart).toHaveBeenCalledTimes(1)
    expect(mockOpenGamepassWindow).toHaveBeenCalledTimes(1)
    expect(wrapper.find('[data-testid="gamepass-window-error"]').text()).toBe(
      i18nMessages['zh-TW'].loginGamepass.windowError,
    )
    // The form stays up so the user can retry.
    expect(wrapper.find('[data-testid="gamepass-creds"]').exists()).toBe(true)
  })

  it('Refresh re-arms the session and clears the banner', async () => {
    mockLoginGamepassStart.mockReset()
    mockLoginGamepassStart
      .mockReturnValueOnce(err({ code: 'beanfun.transport', message: 'net', details: null }))
      .mockReturnValueOnce(ok())

    const ctx = mountForm()
    const wrapper = await ctx.mountIt()
    await flushPromises()
    expect(wrapper.find('[data-testid="gamepass-connection-lost"]').exists()).toBe(true)

    await wrapper.find('[data-testid="gamepass-refresh"]').trigger('click')
    await flushPromises()

    expect(mockLoginGamepassStart).toHaveBeenCalledTimes(2)
    // Refresh only re-arms; the window opens via the credential form.
    expect(mockOpenGamepassWindow).not.toHaveBeenCalled()
    expect(wrapper.find('[data-testid="gamepass-connection-lost"]').exists()).toBe(false)
    expect(wrapper.find('[data-testid="gamepass-creds"]').exists()).toBe(true)
  })

  it('Back button ("返回一般登入") navigates to /login/id-pass without further backend calls', async () => {
    /*
     * Regression for the bug where goBack pushed `/login?pick=1`,
     * which dumped the user back at the region picker even though
     * the button label promised "back to regular (id-pass) login".
     * The correct behaviour is mode-switch within the saved region.
     */
    const ctx = mountForm()
    const wrapper = await ctx.mountIt()
    await flushPromises()

    const startCallsAfterMount = mockLoginGamepassStart.mock.calls.length
    const openCallsAfterMount = mockOpenGamepassWindow.mock.calls.length

    await wrapper.find('[data-testid="gamepass-back"]').trigger('click')
    await flushPromises()

    expect(ctx.router.currentRoute.value.path).toBe('/login/id-pass')
    expect(mockLoginGamepassStart.mock.calls.length).toBe(startCallsAfterMount)
    expect(mockOpenGamepassWindow.mock.calls.length).toBe(openCallsAfterMount)
  })

  it('re-renders localized copy after a runtime locale switch', async () => {
    const ctx = mountForm()
    const wrapper = await ctx.mountIt()
    await flushPromises()

    expect(wrapper.text()).toContain(i18nMessages['zh-TW'].loginGamepass.title)

    setLocale(ctx.i18n, 'en-US')
    await flushPromises()

    expect(wrapper.text()).toContain(i18nMessages['en-US'].loginGamepass.title)
  })

  /*
   * ──────────────────────────────────────────────────────────────
   * CP4 event wiring
   * ──────────────────────────────────────────────────────────────
   */

  it('success event installs the session and navigates to /accounts', async () => {
    const ctx = mountForm()
    const wrapper = await ctx.mountIt()
    await flushPromises()
    // Open the window first (the success event only arrives after that).
    await wrapper.find('[data-testid="gamepass-creds"]').trigger('submit')
    await flushPromises()

    const auth = useAuthStore()
    const account = useAccountStore()
    const applySpy: MockInstance = vi.spyOn(auth, 'applyGamepassSession')
    const clearSpy: MockInstance = vi.spyOn(account, 'clearSessionData')
    account.serviceAccounts = [
      {
        is_enable: true,
        visible: true,
        is_inherited: false,
        sid: 'stale-sid',
        ssn: '1',
        sname: 'Stale Account',
        screatetime: null,
        slastusedtime: null,
        sauthtype: null,
      },
    ]

    await fireEvent('gamepass-login-success', SAMPLE_SESSION)

    expect(clearSpy).toHaveBeenCalledTimes(1)
    expect(applySpy).toHaveBeenCalledWith(SAMPLE_SESSION)
    expect(account.serviceAccounts).toEqual([])
    expect(wrapper.find('[data-testid="gamepass-steps"]').attributes('data-active')).toBe('4')
    expect(ctx.router.currentRoute.value.path).toBe('/accounts')
  })

  it('classic mode: success event arms pendingClassicLaunch (TW GamaPass can drive Classic)', async () => {
    const ctx = mountForm()
    useConfigStore().entries['classicLoginMode'] = 'true'
    const wrapper = await ctx.mountIt()
    await flushPromises()
    await wrapper.find('[data-testid="gamepass-creds"]').trigger('submit')
    await flushPromises()

    await fireEvent('gamepass-login-success', SAMPLE_SESSION)

    expect(useUiStore().pendingClassicLaunch).toBe(true)
    expect(ctx.router.currentRoute.value.path).toBe('/accounts')
  })

  it('classic mode off: success event leaves pendingClassicLaunch unarmed', async () => {
    const ctx = mountForm()
    const wrapper = await ctx.mountIt()
    await flushPromises()
    await wrapper.find('[data-testid="gamepass-creds"]').trigger('submit')
    await flushPromises()

    await fireEvent('gamepass-login-success', SAMPLE_SESSION)

    expect(useUiStore().pendingClassicLaunch).toBe(false)
  })

  it('failed event surfaces the window-error banner and re-shows the form', async () => {
    const ctx = mountForm()
    const wrapper = await ctx.mountIt()
    await flushPromises()
    await wrapper.find('[data-testid="gamepass-creds"]').trigger('submit')
    await flushPromises()
    // Sanity: window opened → step tracker at 2.
    expect(wrapper.find('[data-testid="gamepass-steps"]').attributes('data-active')).toBe('2')

    await fireEvent('gamepass-login-failed', {
      code: 'auth.gamepass_cookie_harvest_failed',
      message: 'Unable to read cookies from the GamePass webview. Please retry.',
      details: null,
    } satisfies CommandError)

    expect(wrapper.find('[data-testid="gamepass-window-error"]').text()).toBe(
      i18nMessages['zh-TW'].loginGamepass.windowError,
    )
    // Window closed → credential form returns so the user can retry.
    expect(wrapper.find('[data-testid="gamepass-creds"]').exists()).toBe(true)
  })

  it('cancelled event silently re-arms and re-shows the credential form', async () => {
    const ctx = mountForm()
    const wrapper = await ctx.mountIt()
    await flushPromises()
    await wrapper.find('[data-testid="gamepass-creds"]').trigger('submit')
    await flushPromises()
    expect(wrapper.find('[data-testid="gamepass-steps"]').exists()).toBe(true)

    await fireEvent<null>('gamepass-login-cancelled', null)
    await flushPromises()

    // WPF parity: silent — no banner, no toast; the form comes back.
    expect(wrapper.find('[data-testid="gamepass-creds"]').exists()).toBe(true)
    expect(wrapper.find('[data-testid="gamepass-window-error"]').exists()).toBe(false)
    expect(wrapper.find('[data-testid="gamepass-connection-lost"]').exists()).toBe(false)
    expect(elMessageInfo).not.toHaveBeenCalled()
    expect(elMessageSuccess).not.toHaveBeenCalled()
    expect(elMessageError).not.toHaveBeenCalled()
  })

  it('unmount detaches every registered tauri listener', async () => {
    const ctx = mountForm()
    const wrapper = await ctx.mountIt()
    await flushPromises()

    // All three listeners must have been registered on mount.
    expect(listenSpy).toHaveBeenCalledWith('gamepass-login-success', expect.any(Function))
    expect(listenSpy).toHaveBeenCalledWith('gamepass-login-failed', expect.any(Function))
    expect(listenSpy).toHaveBeenCalledWith('gamepass-login-cancelled', expect.any(Function))

    wrapper.unmount()

    expect(eventUnlistenSpies['gamepass-login-success']).toHaveBeenCalledTimes(1)
    expect(eventUnlistenSpies['gamepass-login-failed']).toHaveBeenCalledTimes(1)
    expect(eventUnlistenSpies['gamepass-login-cancelled']).toHaveBeenCalledTimes(1)
  })
})
