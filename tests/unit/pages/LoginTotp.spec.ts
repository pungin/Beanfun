/**
 * P12.1 D6 — TOTP login form behaviour.
 *
 * Locks down the WPF `LoginTotp.xaml(.cs)` contract after the Vue
 * port:
 *
 * 1. Renders title + subtitle + 6 single-character cells + the
 *    shared `Login` button (i18n wiring end-to-end).
 * 2. Submitting with no cells filled is a no-op — WPF `btn_login`
 *    validation (`t_totp*` concatenation must be 6 digits) parity;
 *    no `auth.loginTotp` IPC call.
 * 3. Filling all 6 cells auto-submits the joined code — WPF
 *    `totp_6.TextChanged` → `btn_login_Click` auto-fire parity.
 * 4. Success → `router.push('/accounts')` (MainWindow L1480).
 * 5. `auth.advance_check_required` → `router.push('/login/verify')`
 *    (D8 destination; WPF `LoginAdvanceCheck` branch).
 * 6. Any other `loginTotp` error → `router.push('/login/id-pass')`
 *    (WPF `errexit(err, 1)` → `NavigateLoginPage()` parity).
 * 7. Back link → `/login/id-pass` without calling `loginTotp` —
 *    WPF `btn_cancel_Click` parity.
 * 8. Paste handler wires through the composable: pasting 6 digits
 *    into the first cell spreads + auto-submits (the "one-shot
 *    from password manager" UX path). Cell-level filter semantics
 *    are exhaustively covered in `useOtpInputs.spec.ts`; this case
 *    only proves the page-level `@paste` hook is bound.
 * 9. Labels re-render on runtime locale switch — proves the
 *    `loginTotp.*` namespace is wired through every locale.
 */

import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { flushPromises, mount } from '@vue/test-utils'
import { createMemoryHistory, createRouter } from 'vue-router'
import { defineComponent, h, ref } from 'vue'

import type { CommandError, Result, SessionInfo } from '../../../src/types/bindings'

const { elMessageError } = vi.hoisted(() => ({ elMessageError: vi.fn() }))

/*
 * Stubs closely mirror `IdPassForm.spec.ts`/`GamepassForm.spec.ts` so
 * page specs share an event-propagation contract. The one addition
 * here is `ElInput` exposing a `focus()` method via `expose()` — the
 * `useOtpInputs` composable's `register(i, el)` walks the exposed
 * instance and calls `.focus()` on auto-advance; without the stub
 * matching that shape, the composable would silently no-op and the
 * component spec would miss auto-advance regressions.
 */
vi.mock('element-plus', () => ({
  ElForm: defineComponent({
    name: 'ElFormStub',
    emits: ['submit'],
    setup(_, { slots, emit }) {
      return () =>
        h(
          'form',
          {
            class: 'el-form-stub',
            onSubmit: (e: Event) => {
              e.preventDefault()
              emit('submit', e)
            },
          },
          slots.default?.(),
        )
    },
  }),
  ElInput: defineComponent({
    name: 'ElInputStub',
    props: {
      modelValue: { type: String, default: '' },
      maxlength: { type: [Number, String], default: undefined },
      size: { type: String, default: '' },
      inputmode: { type: String, default: '' },
      autocomplete: { type: String, default: '' },
    },
    emits: ['update:modelValue', 'input', 'keydown', 'paste', 'focus'],
    setup(props, { emit, attrs, expose }) {
      const inputRef = ref<HTMLInputElement | null>(null)
      expose({
        focus: () => inputRef.value?.focus(),
      })
      return () =>
        h('input', {
          ...attrs,
          ref: inputRef,
          class: 'el-input-stub',
          value: props.modelValue,
          maxlength: props.maxlength,
          inputmode: props.inputmode,
          autocomplete: props.autocomplete,
          onInput: (e: Event) => {
            const v = (e.target as HTMLInputElement).value
            emit('input', v)
            emit('update:modelValue', v)
          },
          onKeydown: (e: KeyboardEvent) => emit('keydown', e),
          onPaste: (e: ClipboardEvent) => emit('paste', e),
          onFocus: (e: FocusEvent) => emit('focus', e),
        })
    },
  }),
  ElButton: defineComponent({
    name: 'ElButtonStub',
    props: {
      loading: { type: Boolean, default: false },
      nativeType: { type: String, default: 'button' },
    },
    emits: ['click'],
    setup(props, { slots, emit, attrs }) {
      return () =>
        h(
          'button',
          {
            ...attrs,
            class: 'el-button-stub',
            type: props.nativeType,
            disabled: props.loading,
            onClick: (e: MouseEvent) => emit('click', e),
          },
          slots.default?.(),
        )
    },
  }),
  ElIcon: defineComponent({
    name: 'ElIconStub',
    setup(_, { slots }) {
      return () => h('span', { class: 'el-icon-stub' }, slots.default?.())
    },
  }),
  ElMessage: { error: elMessageError, success: vi.fn(), warning: vi.fn() },
}))

vi.mock('@element-plus/icons-vue', () => ({
  ArrowLeft: defineComponent({ name: 'ArrowLeftStub', render: () => h('svg') }),
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
    loadAccounts: vi.fn(),
    saveAccount: vi.fn(),
  },
}))

import { commands } from '../../../src/types/bindings'
import LoginTotp from '../../../src/pages/LoginTotp.vue'
import { useAuthStore } from '../../../src/stores/auth'
import { createAppI18n, i18nMessages, setLocale } from '../../../src/i18n'
import type { Account } from '../../../src/types/bindings'

const mockLoginTotp = vi.mocked(commands.loginTotp)
const mockSaveAccount = vi.mocked(commands.saveAccount)
const mockSetConfig = vi.mocked(commands.setConfig)

const ok = <T>(data: T): Promise<Result<T, CommandError>> => Promise.resolve({ status: 'ok', data })

const err = (error: CommandError): Promise<Result<never, CommandError>> =>
  Promise.resolve({ status: 'error', error })

const FAKE_SESSION: SessionInfo = {
  region: 'TW',
  account_id: 'acc1',
  service_code: '610074',
  service_region: 'T9',
}

/**
 * Memory-router harness mirroring `IdPassForm.spec.ts` — stubs every
 * post-submit destination so navigation assertions don't require
 * shipping their components.
 */
function mountForm() {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: [
      { path: '/login/totp', name: 'login-totp', component: LoginTotp },
      {
        path: '/login/id-pass',
        name: 'login-id-pass',
        component: defineComponent({ name: 'IdPassStub', render: () => h('div') }),
      },
      {
        path: '/login/verify',
        name: 'login-verify',
        component: defineComponent({ name: 'VerifyStub', render: () => h('div') }),
      },
      {
        path: '/accounts',
        name: 'accounts',
        component: defineComponent({ name: 'AccountsStub', render: () => h('div') }),
      },
    ],
  })

  const i18n = createAppI18n()
  return {
    router,
    i18n,
    async mountIt() {
      await router.push('/login/totp')
      await router.isReady()
      return mount(LoginTotp, { global: { plugins: [router, i18n] } })
    },
  }
}

/**
 * Fill every cell sequentially so the composable's `onComplete` hook
 * fires exactly once after the sixth digit — mirrors how a user pastes
 * a code from their authenticator app (one keystroke per cell).
 */
async function fillAllCells(wrapper: ReturnType<typeof mount>, code: string) {
  const inputs = wrapper.findAll('.el-input-stub')
  for (let i = 0; i < code.length; i++) {
    await inputs[i].setValue(code[i])
  }
  await flushPromises()
}

describe('LoginTotp', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    mockLoginTotp.mockReset()
    mockSaveAccount.mockReset()
    mockSetConfig.mockReset()
    elMessageError.mockReset()
  })

  it('renders title, subtitle, 6 cells, and the Login button', async () => {
    const ctx = mountForm()
    const wrapper = await ctx.mountIt()

    expect(wrapper.text()).toContain(i18nMessages['zh-TW'].loginTotp.title)
    expect(wrapper.text()).toContain(i18nMessages['zh-TW'].loginTotp.subtitle)
    expect(wrapper.findAll('.el-input-stub')).toHaveLength(6)
    expect(wrapper.find('[data-test="totp-submit"]').text()).toBe(i18nMessages['zh-TW'].Login)
  })

  it('ignores Enter when no cells are filled (no IPC call)', async () => {
    const ctx = mountForm()
    const wrapper = await ctx.mountIt()

    await wrapper.find('.el-form-stub').trigger('submit')
    await flushPromises()

    expect(mockLoginTotp).not.toHaveBeenCalled()
  })

  it('auto-submits with the joined code once all six cells are filled', async () => {
    const ctx = mountForm()
    const wrapper = await ctx.mountIt()

    mockLoginTotp.mockReturnValueOnce(ok(FAKE_SESSION))

    await fillAllCells(wrapper, '123456')

    expect(mockLoginTotp).toHaveBeenCalledTimes(1)
    expect(mockLoginTotp).toHaveBeenCalledWith('123456')
  })

  it('navigates to /accounts on success', async () => {
    const ctx = mountForm()
    const wrapper = await ctx.mountIt()

    mockLoginTotp.mockReturnValueOnce(ok(FAKE_SESSION))

    await fillAllCells(wrapper, '123456')

    expect(ctx.router.currentRoute.value.path).toBe('/accounts')
  })

  it('navigates to /login/verify when the server requires advanced verify', async () => {
    const ctx = mountForm()
    const wrapper = await ctx.mountIt()

    mockLoginTotp.mockReturnValueOnce(
      err({
        code: 'auth.advance_check_required',
        message: 'verify required',
        details: { url: null },
      }),
    )

    await fillAllCells(wrapper, '123456')

    expect(ctx.router.currentRoute.value.path).toBe('/login/verify')
  })

  it('navigates back to /login/id-pass on any auth error (WPF errexit parity)', async () => {
    const ctx = mountForm()
    const wrapper = await ctx.mountIt()

    mockLoginTotp.mockReturnValueOnce(
      err({ code: 'auth.invalid_totp', message: 'bad totp', details: null }),
    )

    await fillAllCells(wrapper, '000000')

    expect(ctx.router.currentRoute.value.path).toBe('/login/id-pass')
  })

  it('back link navigates to /login/id-pass without calling loginTotp', async () => {
    const ctx = mountForm()
    const wrapper = await ctx.mountIt()

    await wrapper.get('[data-test="totp-back"]').trigger('click')
    await flushPromises()

    expect(ctx.router.currentRoute.value.path).toBe('/login/id-pass')
    expect(mockLoginTotp).not.toHaveBeenCalled()
  })

  it('spreads pasted 6-digit code across cells and auto-submits', async () => {
    const ctx = mountForm()
    const wrapper = await ctx.mountIt()

    mockLoginTotp.mockReturnValueOnce(ok(FAKE_SESSION))

    /*
     * JSDOM does not construct a usable `ClipboardEvent`, so feed the
     * handler a plain object shaped like the DOM contract. The
     * composable reads `clipboardData.getData('text')`; everything
     * else on the event is ignored.
     */
    const pasteEvent = {
      clipboardData: { getData: (type: string) => (type === 'text' ? '987654' : '') },
      preventDefault: vi.fn(),
    } as unknown as ClipboardEvent

    const inputs = wrapper.findAll('.el-input-stub')
    await inputs[0].trigger('paste', pasteEvent)
    await flushPromises()

    expect(mockLoginTotp).toHaveBeenCalledTimes(1)
    expect(mockLoginTotp).toHaveBeenCalledWith('987654')
  })

  it('re-renders labels after a runtime locale switch', async () => {
    const ctx = mountForm()
    const wrapper = await ctx.mountIt()

    expect(wrapper.find('[data-test="totp-submit"]').text()).toBe(i18nMessages['zh-TW'].Login)

    setLocale(ctx.i18n, 'en-US')
    await flushPromises()

    expect(wrapper.find('[data-test="totp-submit"]').text()).toBe(i18nMessages['en-US'].Login)
    expect(wrapper.text()).toContain(i18nMessages['en-US'].loginTotp.title)
  })
})

/**
 * P12.2 D2 — credential persistence tests for the TOTP success
 * branch.
 *
 * What this block locks down:
 *
 * 1. Successful TOTP submit reads `auth.loginIntent`
 *    (stashed by IdPassForm before navigation) and calls
 *    `commands.saveAccount` with the same WPF-shape payload as
 *    the no-TOTP regular login path.
 * 2. `commands.setConfig('AccountID', accountId)` is invoked
 *    after a successful save.
 * 3. `auth.verifyIntent` (set by VerifyPage on a prior round-trip)
 *    is folded into the saved record.
 * 4. Both intent slots are wiped after a successful persist
 *    (single-shot consume).
 * 5. Defensive guard: missing `auth.loginIntent` is logged but
 *    does not block navigation to `/accounts` (deep-link / nav
 *    restoration safety net).
 */
describe('LoginTotp — P12.2 D2 credential persistence', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    mockLoginTotp.mockReset()
    mockSaveAccount.mockReset()
    mockSetConfig.mockReset()
    elMessageError.mockReset()
    vi.spyOn(console, 'warn').mockImplementation(() => {})
  })

  function seedLoginIntent(): void {
    const auth = useAuthStore()
    auth.setLoginIntent({
      region: 'TW',
      accountId: 'alice',
      password: 'hunter2',
      rememberPassword: true,
      autoLogin: false,
    })
  }

  it('persists credentials with the WPF-shape payload after a successful TOTP', async () => {
    const ctx = mountForm()
    const wrapper = await ctx.mountIt()
    seedLoginIntent()

    mockLoginTotp.mockReturnValueOnce(ok(FAKE_SESSION))
    mockSaveAccount.mockReturnValueOnce(ok([] as Account[]))
    mockSetConfig.mockReturnValueOnce(ok(null))

    await fillAllCells(wrapper, '123456')

    expect(mockSaveAccount).toHaveBeenCalledTimes(1)
    expect(mockSaveAccount).toHaveBeenCalledWith({
      region: 'TW',
      account_id: 'alice',
      account_name: '',
      password: 'hunter2',
      verify: '',
      method: 0,
      auto_login: false,
      last_login_at: expect.any(String),
    })
    expect(mockSetConfig).toHaveBeenCalledWith('AccountID', 'alice')
    expect(ctx.router.currentRoute.value.path).toBe('/accounts')
  })

  it('folds auth.verifyIntent into the saved record when present', async () => {
    const ctx = mountForm()
    const wrapper = await ctx.mountIt()
    seedLoginIntent()
    const auth = useAuthStore()
    auth.setVerifyIntent({ code: 'V789', remember: true })

    mockLoginTotp.mockReturnValueOnce(ok(FAKE_SESSION))
    mockSaveAccount.mockReturnValueOnce(ok([] as Account[]))
    mockSetConfig.mockReturnValueOnce(ok(null))

    await fillAllCells(wrapper, '123456')

    expect(mockSaveAccount).toHaveBeenCalledWith(
      expect.objectContaining({ verify: 'V789', account_id: 'alice' }),
    )
  })

  it('clears both intent slots after a successful persist', async () => {
    const ctx = mountForm()
    const wrapper = await ctx.mountIt()
    seedLoginIntent()
    const auth = useAuthStore()
    auth.setVerifyIntent({ code: 'V', remember: true })

    mockLoginTotp.mockReturnValueOnce(ok(FAKE_SESSION))
    mockSaveAccount.mockReturnValueOnce(ok([] as Account[]))
    mockSetConfig.mockReturnValueOnce(ok(null))

    await fillAllCells(wrapper, '123456')

    expect(auth.loginIntent).toBeNull()
    expect(auth.verifyIntent).toBeNull()
    void ctx
  })

  it('skips saveAccount + still navigates when loginIntent is missing (defensive guard)', async () => {
    const ctx = mountForm()
    const wrapper = await ctx.mountIt()

    mockLoginTotp.mockReturnValueOnce(ok(FAKE_SESSION))

    await fillAllCells(wrapper, '123456')

    expect(mockSaveAccount).not.toHaveBeenCalled()
    expect(mockSetConfig).not.toHaveBeenCalled()
    expect(ctx.router.currentRoute.value.path).toBe('/accounts')
  })

  it('does not persist when the server still requires advance verify after TOTP', async () => {
    const ctx = mountForm()
    const wrapper = await ctx.mountIt()
    seedLoginIntent()

    mockLoginTotp.mockReturnValueOnce(
      err({
        code: 'auth.advance_check_required',
        message: 'verify required',
        details: { url: null },
      }),
    )

    await fillAllCells(wrapper, '123456')

    expect(mockSaveAccount).not.toHaveBeenCalled()
    expect(mockSetConfig).not.toHaveBeenCalled()
    expect(ctx.router.currentRoute.value.path).toBe('/login/verify')
    const auth = useAuthStore()
    expect(auth.loginIntent?.accountId).toBe('alice')
  })
})
