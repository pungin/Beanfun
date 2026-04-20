/**
 * P12.1 D3 — id-pass form behaviour.
 *
 * What this spec locks down (matches WPF
 * `id-pass_form.xaml.cs` button + checkbox handlers verbatim):
 *
 * 1. Renders all WPF labels (account / password / remember /
 *    auto-login / login button) — proves i18n flow.
 * 2. Empty account → `AccountNeed` toast, no IPC call (matches WPF
 *    `btn_login_Click` early returns).
 * 3. Empty password → `PasswordNeed` toast, no IPC call.
 * 4. Toggling AutoLogin auto-checks Remember (WPF
 *    `checkBox_AutoLogin_Checked`).
 * 5. Un-checking Remember un-checks AutoLogin (WPF
 *    `checkBox_RememberPWD_Unchecked`).
 * 6. Submit with credentials sends region from Config.xml (TW
 *    default + HK override) → `auth.loginRegular(region, …)` → on
 *    success pushes to `/accounts`.
 * 7. `pendingTotp = true` after `loginRegular` → router.push
 *    `/login/totp` (D6 destination).
 * 8. `pendingVerify = true` after `loginRegular` → router.push
 *    `/login/verify` (D8 destination).
 *
 * D3 → D4 hotfix additions (per "應該要可以回上一頁" feedback):
 *
 * 9. Back button → `/login` (region picker) — SPA affordance gap.
 * 10. QR-switch link → `/login/qr` — WPF parity for `btn_QRCode`
 *     (`id-pass_form.xaml` L736 `btn_QRCode_Click`).
 */

import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { flushPromises, mount } from '@vue/test-utils'
import { createMemoryHistory, createRouter } from 'vue-router'
import { defineComponent, h } from 'vue'

import type { CommandError, Result, SessionInfo } from '../../../src/types/bindings'

/*
 * `vi.mock` factories are hoisted above every top-level `const`, so
 * variables they reference must live inside `vi.hoisted` to actually
 * exist at mock-evaluation time. The shared `elMessageError` spy lets
 * tests assert that validation paths surface the right toast.
 */
const { elMessageError } = vi.hoisted(() => ({ elMessageError: vi.fn() }))

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
  ElFormItem: defineComponent({
    name: 'ElFormItemStub',
    props: ['label'],
    setup(props, { slots }) {
      return () =>
        h('div', { class: 'el-form-item-stub' }, [
          h('label', { class: 'el-form-item-stub__label' }, props.label),
          h('div', { class: 'el-form-item-stub__content' }, slots.default?.()),
        ])
    },
  }),
  ElInput: defineComponent({
    name: 'ElInputStub',
    props: {
      modelValue: { type: String, default: '' },
      placeholder: { type: String, default: '' },
      type: { type: String, default: 'text' },
    },
    emits: ['update:modelValue'],
    setup(props, { emit, attrs }) {
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
  ElCheckbox: defineComponent({
    name: 'ElCheckboxStub',
    props: {
      modelValue: { type: Boolean, default: false },
      label: { type: String, default: '' },
    },
    emits: ['update:modelValue'],
    setup(props, { emit, attrs }) {
      return () =>
        h('label', { class: 'el-checkbox-stub' }, [
          h('input', {
            ...attrs,
            type: 'checkbox',
            class: 'el-checkbox-stub__input',
            checked: props.modelValue,
            onChange: (e: Event) =>
              emit('update:modelValue', (e.target as HTMLInputElement).checked),
          }),
          h('span', { class: 'el-checkbox-stub__label' }, props.label),
        ])
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
  Lock: defineComponent({ name: 'LockStub', render: () => h('svg') }),
  User: defineComponent({ name: 'UserStub', render: () => h('svg') }),
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
    loadAccounts: vi.fn(),
    saveAccount: vi.fn(),
    detectGamePath: vi.fn(),
    listGameProcesses: vi.fn(),
    killGameProcesses: vi.fn(),
    openUrl: vi.fn(),
    launchGame: vi.fn(),
  },
}))

/*
 * P12.4 followup-B B9 — stub the in-app browser composable so
 * RegisterAccount / ForgotPassword tests can assert the URL the
 * button dispatches without booting the backend IPC. The
 * `openInAppBrowserSpy` records each call in order so the per-
 * test assertions can compare against the region-aware URL
 * tables verbatim.
 */
const { openInAppBrowserSpy } = vi.hoisted(() => ({ openInAppBrowserSpy: vi.fn() }))

vi.mock('../../../src/composables/useInAppBrowser', () => ({
  useInAppBrowser: () => ({ open: openInAppBrowserSpy }),
}))

/*
 * P12.4 followup-A D9 — stub the launcher composable so GameStart
 * tests don't have to seed game store + config + 5 IPC mocks. We
 * just want to assert the button wires through.
 */
const { runGameSpy } = vi.hoisted(() => ({ runGameSpy: vi.fn() }))

vi.mock('../../../src/composables/useGameLauncher', () => ({
  useGameLauncher: () => ({ runGame: runGameSpy }),
}))

import { commands } from '../../../src/types/bindings'
import IdPassForm from '../../../src/pages/IdPassForm.vue'
import { useAccountStore } from '../../../src/stores/account'
import { useAuthStore } from '../../../src/stores/auth'
import { useConfigStore } from '../../../src/stores/config'
import { createAppI18n, i18nMessages, setLocale } from '../../../src/i18n'
import type { Account } from '../../../src/types/bindings'

const mockLoginRegular = vi.mocked(commands.loginRegular)
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
 * Standalone harness: mounts IdPassForm at `/login/id-pass` with a
 * memory router that also exposes stub routes for the post-login
 * navigation targets (`/accounts`, `/login/totp`, `/login/verify`).
 * Mirrors the LoginRegionSelection harness so each form gets a small
 * sandbox of just the routes it actually navigates to.
 */
function mountForm(initialRegion?: string) {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: [
      { path: '/login/id-pass', name: 'login-id-pass', component: IdPassForm },
      {
        path: '/login',
        name: 'login-region',
        component: defineComponent({ name: 'RegionStub', render: () => h('div') }),
      },
      {
        path: '/login/qr',
        name: 'login-qr',
        component: defineComponent({ name: 'QrStub', render: () => h('div') }),
      },
      {
        path: '/login/gamepass',
        name: 'login-gamepass',
        component: defineComponent({ name: 'GamepassStub', render: () => h('div') }),
      },
      {
        path: '/accounts',
        name: 'accounts',
        component: defineComponent({ name: 'AccountsStub', render: () => h('div') }),
      },
      {
        path: '/login/totp',
        name: 'login-totp',
        component: defineComponent({ name: 'TotpStub', render: () => h('div') }),
      },
      {
        path: '/login/verify',
        name: 'login-verify',
        component: defineComponent({ name: 'VerifyStub', render: () => h('div') }),
      },
    ],
  })

  const i18n = createAppI18n()
  if (initialRegion) {
    const config = useConfigStore()
    config.entries['loginRegion'] = initialRegion
  }

  return {
    router,
    i18n,
    async mountIt() {
      await router.push('/login/id-pass')
      await router.isReady()
      return mount(IdPassForm, {
        global: { plugins: [router, i18n] },
      })
    },
  }
}

describe('IdPassForm', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    mockLoginRegular.mockReset()
    mockSaveAccount.mockReset()
    mockSetConfig.mockReset()
    elMessageError.mockReset()
  })

  it('renders all WPF labels + login button', async () => {
    const ctx = mountForm()
    const wrapper = await ctx.mountIt()

    expect(wrapper.text()).toContain(i18nMessages['zh-TW'].AcountOrEmail)
    expect(wrapper.text()).toContain(i18nMessages['zh-TW'].Password_)
    expect(wrapper.text()).toContain(i18nMessages['zh-TW'].RememberPassword)
    expect(wrapper.text()).toContain(i18nMessages['zh-TW'].AutoLogin)
    expect(wrapper.find('.el-button-stub').text()).toBe(i18nMessages['zh-TW'].Login)
  })

  it('blocks submit + toasts AccountNeed when account is empty', async () => {
    const ctx = mountForm()
    const wrapper = await ctx.mountIt()

    await wrapper.find('.el-form-stub').trigger('submit')
    await flushPromises()

    expect(elMessageError).toHaveBeenCalledWith(i18nMessages['zh-TW'].AccountNeed)
    expect(mockLoginRegular).not.toHaveBeenCalled()
  })

  it('blocks submit + toasts PasswordNeed when password is empty', async () => {
    const ctx = mountForm()
    const wrapper = await ctx.mountIt()

    const inputs = wrapper.findAll('.el-input-stub')
    await inputs[0].setValue('user@example.com')
    await wrapper.find('.el-form-stub').trigger('submit')
    await flushPromises()

    expect(elMessageError).toHaveBeenCalledWith(i18nMessages['zh-TW'].PasswordNeed)
    expect(mockLoginRegular).not.toHaveBeenCalled()
  })

  it('checking AutoLogin auto-checks Remember (WPF coupling)', async () => {
    const ctx = mountForm()
    const wrapper = await ctx.mountIt()

    const checkboxes = wrapper.findAll('.el-checkbox-stub__input')
    const remember = checkboxes[0].element as HTMLInputElement
    const autoLogin = checkboxes[1].element as HTMLInputElement

    expect(remember.checked).toBe(false)
    expect(autoLogin.checked).toBe(false)

    await checkboxes[1].setValue(true)

    expect(remember.checked).toBe(true)
    expect(autoLogin.checked).toBe(true)
  })

  it('un-checking Remember un-checks AutoLogin (WPF coupling)', async () => {
    const ctx = mountForm()
    const wrapper = await ctx.mountIt()

    const checkboxes = wrapper.findAll('.el-checkbox-stub__input')
    const remember = checkboxes[0].element as HTMLInputElement
    const autoLogin = checkboxes[1].element as HTMLInputElement

    await checkboxes[1].setValue(true)
    expect(remember.checked).toBe(true)
    expect(autoLogin.checked).toBe(true)

    await checkboxes[0].setValue(false)

    expect(remember.checked).toBe(false)
    expect(autoLogin.checked).toBe(false)
  })

  it('submits with the region from Config.xml (TW default → loginRegular(TW, ...))', async () => {
    const ctx = mountForm()
    const wrapper = await ctx.mountIt()

    mockLoginRegular.mockReturnValueOnce(ok(FAKE_SESSION))

    const inputs = wrapper.findAll('.el-input-stub')
    await inputs[0].setValue('user@example.com')
    await inputs[1].setValue('hunter2')
    await wrapper.find('.el-form-stub').trigger('submit')
    await flushPromises()

    expect(mockLoginRegular).toHaveBeenCalledWith('TW', 'user@example.com', 'hunter2')
    expect(ctx.router.currentRoute.value.path).toBe('/accounts')
  })

  it('respects HK from Config.xml when submitting', async () => {
    const ctx = mountForm('HK')
    const wrapper = await ctx.mountIt()

    mockLoginRegular.mockReturnValueOnce(ok({ ...FAKE_SESSION, region: 'HK' }))

    const inputs = wrapper.findAll('.el-input-stub')
    await inputs[0].setValue('hk-user')
    await inputs[1].setValue('hk-pass')
    await wrapper.find('.el-form-stub').trigger('submit')
    await flushPromises()

    expect(mockLoginRegular).toHaveBeenCalledWith('HK', 'hk-user', 'hk-pass')
  })

  it('navigates to /login/totp when the server requires TOTP', async () => {
    const ctx = mountForm()
    const wrapper = await ctx.mountIt()

    mockLoginRegular.mockReturnValueOnce(
      err({
        code: 'auth.totp_required',
        message: 'totp required',
        details: null,
      }),
    )

    const inputs = wrapper.findAll('.el-input-stub')
    await inputs[0].setValue('user')
    await inputs[1].setValue('pass')
    await wrapper.find('.el-form-stub').trigger('submit')
    await flushPromises()

    expect(ctx.router.currentRoute.value.path).toBe('/login/totp')
    expect(elMessageError).not.toHaveBeenCalled()
  })

  it('navigates to /login/verify when the server requires advanced verify', async () => {
    const ctx = mountForm()
    const wrapper = await ctx.mountIt()

    mockLoginRegular.mockReturnValueOnce(
      err({
        code: 'auth.advance_check_required',
        message: 'verify required',
        details: { url: 'https://tw.beanfun.com/AdvanceCheck.aspx?xyz' },
      }),
    )

    const inputs = wrapper.findAll('.el-input-stub')
    await inputs[0].setValue('user')
    await inputs[1].setValue('pass')
    await wrapper.find('.el-form-stub').trigger('submit')
    await flushPromises()

    expect(ctx.router.currentRoute.value.path).toBe('/login/verify')
    expect(elMessageError).not.toHaveBeenCalled()
  })

  it('back button navigates to /login (region picker) without calling loginRegular', async () => {
    const ctx = mountForm()
    const wrapper = await ctx.mountIt()

    await wrapper.get('[data-test="id-pass-back"]').trigger('click')
    await flushPromises()

    expect(ctx.router.currentRoute.value.path).toBe('/login')
    expect(mockLoginRegular).not.toHaveBeenCalled()
  })

  it('QR-switch link navigates to /login/qr (WPF btn_QRCode parity)', async () => {
    const ctx = mountForm()
    const wrapper = await ctx.mountIt()

    await wrapper.get('[data-test="id-pass-switch-qr"]').trigger('click')
    await flushPromises()

    expect(ctx.router.currentRoute.value.path).toBe('/login/qr')
    expect(mockLoginRegular).not.toHaveBeenCalled()
  })

  it('GamePass-switch link navigates to /login/gamepass (WPF btn_GamePass parity)', async () => {
    const ctx = mountForm()
    const wrapper = await ctx.mountIt()

    await wrapper.get('[data-test="id-pass-switch-gamepass"]').trigger('click')
    await flushPromises()

    expect(ctx.router.currentRoute.value.path).toBe('/login/gamepass')
    expect(mockLoginRegular).not.toHaveBeenCalled()
  })

  it('re-renders labels after a runtime locale switch', async () => {
    const ctx = mountForm()
    const wrapper = await ctx.mountIt()

    expect(wrapper.find('.el-button-stub').text()).toBe(i18nMessages['zh-TW'].Login)

    setLocale(ctx.i18n, 'en-US')
    await flushPromises()

    expect(wrapper.find('.el-button-stub').text()).toBe(i18nMessages['en-US'].Login)
  })
})

/**
 * P12.2 D2 — credential persistence integration tests.
 *
 * What this block locks down (matches WPF
 * `MainWindow.xaml.cs::SaveLoginCredentials` L1334-1363 +
 * `loginMethodChanged` L1054-1092 prefill):
 *
 * 1. Mount-time prefill: `Config.AccountID` + matching stored
 *    record → account / password / remember / autoLogin all
 *    populated.
 * 2. Mount-time prefill: missing config key → no prefill.
 * 3. Mount-time prefill: stored record has empty password →
 *    account prefilled but checkboxes left untouched.
 * 4. Submit success: `commands.saveAccount` called with the
 *    SaveLoginCredentials-shaped payload and `commands.setConfig`
 *    persists `AccountID`.
 * 5. Submit success after a verify round-trip:
 *    `auth.verifyIntent` is folded into the saved record.
 * 6. Submit success: both intent slots are cleared after
 *    persistence (single-shot consume).
 */
describe('IdPassForm — P12.2 D2 credential persistence', () => {
  const STORED_ALICE: Account = {
    region: 'TW',
    account_id: 'alice',
    account_name: '',
    password: 'stored-pw',
    verify: '',
    method: 0,
    auto_login: true,
  }

  beforeEach(() => {
    setActivePinia(createPinia())
    mockLoginRegular.mockReset()
    mockSaveAccount.mockReset()
    mockSetConfig.mockReset()
    elMessageError.mockReset()
  })

  it('mount prefills account / password / remember / autoLogin from stored record', async () => {
    const ctx = mountForm()
    const account = useAccountStore()
    account.accounts = [STORED_ALICE]
    const config = useConfigStore()
    config.entries['AccountID'] = 'alice'

    const wrapper = await ctx.mountIt()
    const inputs = wrapper.findAll('.el-input-stub')
    expect((inputs[0].element as HTMLInputElement).value).toBe('alice')
    expect((inputs[1].element as HTMLInputElement).value).toBe('stored-pw')

    const checkboxes = wrapper.findAll('.el-checkbox-stub__input')
    expect((checkboxes[0].element as HTMLInputElement).checked).toBe(true)
    expect((checkboxes[1].element as HTMLInputElement).checked).toBe(true)
  })

  it('mount with no Config.AccountID leaves form blank', async () => {
    const ctx = mountForm()
    const account = useAccountStore()
    account.accounts = [STORED_ALICE]
    const wrapper = await ctx.mountIt()
    const inputs = wrapper.findAll('.el-input-stub')
    expect((inputs[0].element as HTMLInputElement).value).toBe('')
    expect((inputs[1].element as HTMLInputElement).value).toBe('')
  })

  it('mount with stored record but empty password prefills account only (WPF L1067 short-circuit parity)', async () => {
    const ctx = mountForm()
    const account = useAccountStore()
    account.accounts = [{ ...STORED_ALICE, password: '', auto_login: true }]
    const config = useConfigStore()
    config.entries['AccountID'] = 'alice'

    const wrapper = await ctx.mountIt()
    const inputs = wrapper.findAll('.el-input-stub')
    expect((inputs[0].element as HTMLInputElement).value).toBe('alice')
    expect((inputs[1].element as HTMLInputElement).value).toBe('')
    const checkboxes = wrapper.findAll('.el-checkbox-stub__input')
    expect((checkboxes[0].element as HTMLInputElement).checked).toBe(false)
    expect((checkboxes[1].element as HTMLInputElement).checked).toBe(false)
  })

  it('submit success calls saveAccount with the WPF-shape payload + persists AccountID', async () => {
    const ctx = mountForm()
    const wrapper = await ctx.mountIt()

    mockLoginRegular.mockReturnValueOnce(ok(FAKE_SESSION))
    mockSaveAccount.mockReturnValueOnce(ok([]))
    mockSetConfig.mockReturnValueOnce(ok(null))

    const inputs = wrapper.findAll('.el-input-stub')
    await inputs[0].setValue('alice')
    await inputs[1].setValue('hunter2')
    const checkboxes = wrapper.findAll('.el-checkbox-stub__input')
    await checkboxes[0].setValue(true)

    await wrapper.find('.el-form-stub').trigger('submit')
    await flushPromises()

    expect(mockSaveAccount).toHaveBeenCalledTimes(1)
    expect(mockSaveAccount).toHaveBeenCalledWith({
      region: 'TW',
      account_id: 'alice',
      account_name: '',
      password: 'hunter2',
      verify: '',
      method: 0,
      auto_login: false,
    })
    expect(mockSetConfig).toHaveBeenCalledWith('AccountID', 'alice')
    expect(ctx.router.currentRoute.value.path).toBe('/accounts')
  })

  it('second-pass submit folds auth.verifyIntent into the saved record', async () => {
    const ctx = mountForm()
    const wrapper = await ctx.mountIt()
    const auth = useAuthStore()
    auth.setVerifyIntent({ code: 'V789', remember: true })

    mockLoginRegular.mockReturnValueOnce(ok(FAKE_SESSION))
    mockSaveAccount.mockReturnValueOnce(ok([]))
    mockSetConfig.mockReturnValueOnce(ok(null))

    const inputs = wrapper.findAll('.el-input-stub')
    await inputs[0].setValue('alice')
    await inputs[1].setValue('hunter2')
    const checkboxes = wrapper.findAll('.el-checkbox-stub__input')
    await checkboxes[0].setValue(true)

    await wrapper.find('.el-form-stub').trigger('submit')
    await flushPromises()

    expect(mockSaveAccount).toHaveBeenCalledWith(
      expect.objectContaining({ verify: 'V789', region: 'TW', account_id: 'alice' }),
    )
  })

  it('clears both intent slots after a successful persist', async () => {
    const ctx = mountForm()
    const wrapper = await ctx.mountIt()
    const auth = useAuthStore()
    auth.setVerifyIntent({ code: 'V', remember: true })

    mockLoginRegular.mockReturnValueOnce(ok(FAKE_SESSION))
    mockSaveAccount.mockReturnValueOnce(ok([]))
    mockSetConfig.mockReturnValueOnce(ok(null))

    const inputs = wrapper.findAll('.el-input-stub')
    await inputs[0].setValue('alice')
    await inputs[1].setValue('hunter2')

    await wrapper.find('.el-form-stub').trigger('submit')
    await flushPromises()

    expect(auth.loginIntent).toBeNull()
    expect(auth.verifyIntent).toBeNull()
  })

  it('pendingTotp branch leaves the loginIntent populated for LoginTotp to consume', async () => {
    const ctx = mountForm()
    const wrapper = await ctx.mountIt()
    const auth = useAuthStore()

    mockLoginRegular.mockReturnValueOnce(
      err({ code: 'auth.totp_required', message: 'totp required', details: null }),
    )

    const inputs = wrapper.findAll('.el-input-stub')
    await inputs[0].setValue('alice')
    await inputs[1].setValue('hunter2')

    await wrapper.find('.el-form-stub').trigger('submit')
    await flushPromises()

    expect(ctx.router.currentRoute.value.path).toBe('/login/totp')
    expect(mockSaveAccount).not.toHaveBeenCalled()
    expect(auth.loginIntent).toEqual({
      region: 'TW',
      accountId: 'alice',
      password: 'hunter2',
      rememberPassword: false,
      autoLogin: false,
    })
  })

  it('pendingVerify branch leaves the loginIntent populated for VerifyPage to consume', async () => {
    const ctx = mountForm()
    const wrapper = await ctx.mountIt()
    const auth = useAuthStore()

    mockLoginRegular.mockReturnValueOnce(
      err({
        code: 'auth.advance_check_required',
        message: 'verify',
        details: { url: 'https://x' },
      }),
    )

    const inputs = wrapper.findAll('.el-input-stub')
    await inputs[0].setValue('alice')
    await inputs[1].setValue('pw')

    await wrapper.find('.el-form-stub').trigger('submit')
    await flushPromises()

    expect(ctx.router.currentRoute.value.path).toBe('/login/verify')
    expect(auth.loginIntent?.accountId).toBe('alice')
  })
})

/**
 * P12.4 followup-A D9 + followup-B B9 — RegisterAccount /
 * ForgotPassword / GameStart.
 *
 * What this block locks down (matches WPF
 * `id-pass_form.xaml(.cs)` `RegAcc_Click` / `FindPwd_Click` /
 * `btn_StartGame_Click` parity decisions):
 *
 * 1. ForgotPassword button → calls `useInAppBrowser().open(url)`
 *    with the region-aware `LOGIN_EXTERNAL_URLS.forgotPwd[region]`
 *    URL. Default region (no Config.xml override) → TW URL.
 * 2. ForgotPassword button respects HK from Config.xml → HK URL.
 * 3. RegisterAccount button → same dispatch, but with the
 *    `register` URL set, region-aware (TW default + HK override).
 * 4. GameStart button → delegates to `useGameLauncher().runGame()`
 *    with no credentials (matches WPF `App.MainWnd.runGame()` no-arg
 *    call). The composable owns the `restoreLastSelected` hand-off
 *    and the empty-store toast, so this spec only asserts the
 *    delegation.
 *
 * followup-B replaced the self-mounted `WebBrowser.vue` dialog
 * with the `useInAppBrowser` composable; the spec accordingly
 * mocks the composable and asserts the spy was invoked with the
 * exact URL instead of probing dialog `data-visible` / `data-url`
 * attributes.
 */
describe('IdPassForm — P12.4 followup-A/B (Register / Forgot / GameStart)', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    mockLoginRegular.mockReset()
    mockSaveAccount.mockReset()
    mockSetConfig.mockReset()
    elMessageError.mockReset()
    runGameSpy.mockReset()
    openInAppBrowserSpy.mockReset()
  })

  it('ForgotPassword dispatches in-app browser with TW forgot_pwd URL by default', async () => {
    const ctx = mountForm()
    const wrapper = await ctx.mountIt()

    expect(openInAppBrowserSpy).not.toHaveBeenCalled()
    await wrapper.get('[data-test="id-pass-forgot-password"]').trigger('click')
    await flushPromises()

    expect(openInAppBrowserSpy).toHaveBeenCalledTimes(1)
    expect(openInAppBrowserSpy).toHaveBeenCalledWith(
      'https://tw.beanfun.com/member/forgot_pwd.aspx',
    )
    expect(runGameSpy).not.toHaveBeenCalled()
  })

  it('ForgotPassword respects HK from Config.xml → HK forgot_pwd URL', async () => {
    const ctx = mountForm('HK')
    const wrapper = await ctx.mountIt()

    await wrapper.get('[data-test="id-pass-forgot-password"]').trigger('click')
    await flushPromises()

    expect(openInAppBrowserSpy).toHaveBeenCalledWith(
      'https://bfweb.hk.beanfun.com/member/forgot_pwd.aspx',
    )
  })

  it('RegisterAccount dispatches in-app browser with TW signup URL by default', async () => {
    const ctx = mountForm()
    const wrapper = await ctx.mountIt()

    await wrapper.get('[data-test="id-pass-register"]').trigger('click')
    await flushPromises()

    expect(openInAppBrowserSpy).toHaveBeenCalledTimes(1)
    expect(openInAppBrowserSpy).toHaveBeenCalledWith(
      'https://tw.beanfun.com/TW/signup/Join_beanfun_signup.aspx?service=999999_T0',
    )
  })

  it('RegisterAccount respects HK from Config.xml → HK signup URL', async () => {
    const ctx = mountForm('HK')
    const wrapper = await ctx.mountIt()

    await wrapper.get('[data-test="id-pass-register"]').trigger('click')
    await flushPromises()

    expect(openInAppBrowserSpy).toHaveBeenCalledWith(
      'https://bfweb.hk.beanfun.com/beanfun_web_ap/signup/preregistration.aspx?service=999999_T0',
    )
  })

  it('GameStart delegates to useGameLauncher().runGame() with no credentials', async () => {
    const ctx = mountForm()
    const wrapper = await ctx.mountIt()

    await wrapper.get('[data-test="id-pass-game-start"]').trigger('click')
    await flushPromises()

    expect(runGameSpy).toHaveBeenCalledTimes(1)
    expect(runGameSpy).toHaveBeenCalledWith()
    expect(mockLoginRegular).not.toHaveBeenCalled()
  })
})
