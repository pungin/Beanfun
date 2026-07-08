/**
 * P12.1 D8 — AdvanceCheck verify page behaviour.
 *
 * Locks down the WPF `VerifyPage.xaml(.cs)` contract after the Vue
 * port:
 *
 * 1. Renders title + subtitle + auth-type tip + verify input + captcha
 *    bitmap + captcha input + Remember checkbox + AuthConfirm button.
 *    Proves the i18n wiring end-to-end (frontend-only `loginVerify.*`
 *    namespace + reused WPF locale keys).
 * 2. On mount, calls `getVerifyPageInfo(auth.advanceCheckUrl)` then
 *    `getVerifyCaptcha`, populating the auth-type label and the
 *    captcha bitmap (`<img>` `src` reflects the base64 data URL).
 * 3. Empty `verifyCode` on submit → toasts `MsgAuthInfoEmpty`, no
 *    `submitVerify` IPC call (WPF `Button_Click` early return parity).
 * 4. Empty `captchaCode` on submit → toasts `MsgCaptchaCodeEmpty`,
 *    no IPC call.
 * 5. Successful submit → `loginVerify.success` toast + navigate to
 *    `/login/id-pass` (no-secrets-over-IPC: backend doesn't auto-resume
 *    the login, see `VerifyPage.vue` docblock).
 * 6. `wrong_captcha` → `WrongCaptcha` toast + refresh captcha
 *    (re-fetches `getVerifyCaptcha`, clears the captcha input field).
 * 7. `wrong_auth_info` → `WrongAuthInfo` toast + refresh captcha.
 * 8. `server_message` → renders `outcome.message` verbatim
 *    (server-supplied alert text, already localised by region) and
 *    refreshes captcha.
 * 9. Click on the captcha image → refreshes the bitmap (`getVerifyCaptcha`
 *    re-call), clears the captcha input field. Mirrors WPF
 *    `Button_Click_1`.
 * 10. Back link → `/login/id-pass` without calling any verify IPC —
 *     WPF `Image_MouseLeftButtonDown` parity (return_page = loginPage).
 * 11. `getVerifyPageInfo` failure → inline load-failed banner + Retry
 *     button (no toast — `wrapCommand` already toasted; the banner is
 *     the recovery affordance).
 * 12. `auth.advanceCheckUrl` is forwarded to the IPC call so HK
 *     (null) and TW (URL) paths both reach the backend correctly.
 */

import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { flushPromises, mount } from '@vue/test-utils'
import { createMemoryHistory, createRouter } from 'vue-router'
import { defineComponent, h } from 'vue'

import type {
  CommandError,
  Result,
  VerifyCaptcha,
  VerifyPage,
  VerifySubmit,
} from '../../../src/types/bindings'

const { elMessageError, elMessageSuccess } = vi.hoisted(() => ({
  elMessageError: vi.fn(),
  elMessageSuccess: vi.fn(),
}))

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
    setup(_, { slots }) {
      return () => h('div', { class: 'el-form-item-stub' }, slots.default?.())
    },
  }),
  ElInput: defineComponent({
    name: 'ElInputStub',
    props: {
      modelValue: { type: String, default: '' },
      placeholder: { type: String, default: '' },
      disabled: { type: Boolean, default: false },
      size: { type: String, default: '' },
      autocomplete: { type: String, default: '' },
    },
    emits: ['update:modelValue'],
    setup(props, { emit, attrs }) {
      return () =>
        h('input', {
          ...attrs,
          class: 'el-input-stub',
          value: props.modelValue,
          placeholder: props.placeholder,
          disabled: props.disabled,
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
      size: { type: String, default: '' },
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
  ElMessage: { error: elMessageError, success: elMessageSuccess, warning: vi.fn() },
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
    resumeTwLoginAfterVerify: vi.fn(),
    logout: vi.fn(),
    loadAccounts: vi.fn(),
    saveAccount: vi.fn(),
  },
}))

import { commands } from '../../../src/types/bindings'
import VerifyPage from '../../../src/pages/VerifyPage.vue'
import { useAccountStore } from '../../../src/stores/account'
import { useAuthStore } from '../../../src/stores/auth'
import { createAppI18n, i18nMessages, setLocale } from '../../../src/i18n'
import type { Account } from '../../../src/types/bindings'

const mockGetVerifyPageInfo = vi.mocked(commands.getVerifyPageInfo)
const mockGetVerifyCaptcha = vi.mocked(commands.getVerifyCaptcha)
const mockSubmitVerify = vi.mocked(commands.submitVerify)
const mockResumeTwLoginAfterVerify = vi.mocked(commands.resumeTwLoginAfterVerify)
const mockSaveAccount = vi.mocked(commands.saveAccount)

const ok = <T>(data: T): Promise<Result<T, CommandError>> => Promise.resolve({ status: 'ok', data })

const err = (error: CommandError): Promise<Result<never, CommandError>> =>
  Promise.resolve({ status: 'error', error })

const FAKE_PAGE: VerifyPage = { lbl_auth_type: '您的註冊信箱 (a***@example.com)' }

const FAKE_CAPTCHA: VerifyCaptcha = {
  image_base64: 'data:image/png;base64,AAAA',
}

const FAKE_CAPTCHA_2: VerifyCaptcha = {
  image_base64: 'data:image/png;base64,BBBB',
}

const SUCCESS: VerifySubmit = { result: 'success' }
const WRONG_CAPTCHA: VerifySubmit = { result: 'wrong_captcha' }
const WRONG_AUTH_INFO: VerifySubmit = { result: 'wrong_auth_info' }
const SERVER_MESSAGE = (message: string): VerifySubmit => ({ result: 'server_message', message })

/**
 * Memory-router harness mirroring `LoginTotp.spec.ts`. The verify
 * page only navigates to `/login/id-pass` (back link + post-success),
 * so a single stub destination is enough.
 *
 * `initialAdvanceCheckUrl` is seeded into the auth store before mount
 * so we can assert it propagates through to the backend command (the
 * URL isn't a route param — it's stashed by `loginRegular` /
 * `loginTotp` ahead of time).
 */
function mountForm(initialAdvanceCheckUrl: string | null = null) {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: [
      { path: '/login/verify', name: 'login-verify', component: VerifyPage },
      {
        path: '/login/id-pass',
        name: 'login-id-pass',
        component: defineComponent({ name: 'IdPassStub', render: () => h('div') }),
      },
      {
        path: '/login/recaptcha',
        name: 'login-recaptcha',
        component: defineComponent({ name: 'RecaptchaStub', render: () => h('div') }),
      },
      {
        path: '/accounts',
        name: 'accounts',
        component: defineComponent({ name: 'AccountsStub', render: () => h('div') }),
      },
    ],
  })

  const i18n = createAppI18n()
  const auth = useAuthStore()
  auth.advanceCheckUrl = initialAdvanceCheckUrl

  return {
    router,
    i18n,
    auth,
    async mountIt() {
      await router.push('/login/verify')
      await router.isReady()
      const wrapper = mount(VerifyPage, { global: { plugins: [router, i18n] } })
      await flushPromises()
      return wrapper
    },
  }
}

async function fillVerifyAndCaptcha(
  wrapper: ReturnType<typeof mount>,
  verify: string,
  captcha: string,
): Promise<void> {
  await wrapper.get('[data-test="verify-input"]').setValue(verify)
  await wrapper.get('[data-test="verify-captcha-input"]').setValue(captcha)
}

describe('VerifyPage', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    mockGetVerifyPageInfo.mockReset()
    mockGetVerifyCaptcha.mockReset()
    mockSubmitVerify.mockReset()
    mockResumeTwLoginAfterVerify.mockReset()
    mockSaveAccount.mockReset()
    elMessageError.mockReset()
    elMessageSuccess.mockReset()
  })

  it('renders title, subtitle, inputs, captcha, remember and AuthConfirm on mount', async () => {
    mockGetVerifyPageInfo.mockReturnValueOnce(ok(FAKE_PAGE))
    mockGetVerifyCaptcha.mockReturnValueOnce(ok(FAKE_CAPTCHA))

    const ctx = mountForm('https://tw.beanfun.com/AdvanceCheck.aspx?xyz')
    const wrapper = await ctx.mountIt()

    expect(wrapper.text()).toContain(i18nMessages['zh-TW'].loginVerify.title)
    expect(wrapper.text()).toContain(i18nMessages['zh-TW'].loginVerify.subtitle)
    expect(wrapper.text()).toContain(i18nMessages['zh-TW'].YourAuthInfoTip)
    expect(wrapper.text()).toContain(FAKE_PAGE.lbl_auth_type)
    expect(wrapper.find('[data-test="verify-input"]').exists()).toBe(true)
    expect(wrapper.find('[data-test="verify-captcha-input"]').exists()).toBe(true)
    expect(wrapper.find('[data-test="verify-remember"]').exists()).toBe(true)
    expect(wrapper.find('[data-testid="verify-captcha-bitmap"]').attributes('src')).toBe(
      FAKE_CAPTCHA.image_base64,
    )
    expect(wrapper.find('[data-test="verify-submit"]').text()).toBe(
      i18nMessages['zh-TW'].AuthConfirm,
    )
  })

  it('forwards auth.advanceCheckUrl through to getVerifyPageInfo (TW)', async () => {
    mockGetVerifyPageInfo.mockReturnValueOnce(ok(FAKE_PAGE))
    mockGetVerifyCaptcha.mockReturnValueOnce(ok(FAKE_CAPTCHA))

    const url = 'https://tw.beanfun.com/AdvanceCheck.aspx?token=abc'
    const ctx = mountForm(url)
    await ctx.mountIt()

    expect(mockGetVerifyPageInfo).toHaveBeenCalledTimes(1)
    expect(mockGetVerifyPageInfo).toHaveBeenCalledWith(url)
  })

  it('forwards null advanceCheckUrl when the backend has no URL (HK fallback)', async () => {
    mockGetVerifyPageInfo.mockReturnValueOnce(ok(FAKE_PAGE))
    mockGetVerifyCaptcha.mockReturnValueOnce(ok(FAKE_CAPTCHA))

    const ctx = mountForm(null)
    await ctx.mountIt()

    expect(mockGetVerifyPageInfo).toHaveBeenCalledWith(null)
  })

  it('toasts MsgAuthInfoEmpty and skips IPC when verify code is empty', async () => {
    mockGetVerifyPageInfo.mockReturnValueOnce(ok(FAKE_PAGE))
    mockGetVerifyCaptcha.mockReturnValueOnce(ok(FAKE_CAPTCHA))

    const ctx = mountForm()
    const wrapper = await ctx.mountIt()

    await wrapper.get('[data-test="verify-captcha-input"]').setValue('abcd')
    await wrapper.get('.el-form-stub').trigger('submit')
    await flushPromises()

    expect(elMessageError).toHaveBeenCalledWith(i18nMessages['zh-TW'].MsgAuthInfoEmpty)
    expect(mockSubmitVerify).not.toHaveBeenCalled()
  })

  it('toasts MsgCaptchaCodeEmpty and skips IPC when captcha code is empty', async () => {
    mockGetVerifyPageInfo.mockReturnValueOnce(ok(FAKE_PAGE))
    mockGetVerifyCaptcha.mockReturnValueOnce(ok(FAKE_CAPTCHA))

    const ctx = mountForm()
    const wrapper = await ctx.mountIt()

    await wrapper.get('[data-test="verify-input"]').setValue('1234')
    await wrapper.get('.el-form-stub').trigger('submit')
    await flushPromises()

    expect(elMessageError).toHaveBeenCalledWith(i18nMessages['zh-TW'].MsgCaptchaCodeEmpty)
    expect(mockSubmitVerify).not.toHaveBeenCalled()
  })

  it('on success toasts loginVerify.success and navigates to /login/id-pass', async () => {
    mockGetVerifyPageInfo.mockReturnValueOnce(ok(FAKE_PAGE))
    mockGetVerifyCaptcha.mockReturnValueOnce(ok(FAKE_CAPTCHA))
    mockSubmitVerify.mockReturnValueOnce(ok(SUCCESS))

    const ctx = mountForm()
    const wrapper = await ctx.mountIt()

    await fillVerifyAndCaptcha(wrapper, '1234', 'ABCD')
    await wrapper.get('.el-form-stub').trigger('submit')
    await flushPromises()

    expect(mockSubmitVerify).toHaveBeenCalledWith('1234', 'ABCD')
    expect(elMessageSuccess).toHaveBeenCalledWith(i18nMessages['zh-TW'].loginVerify.success)
    expect(ctx.router.currentRoute.value.path).toBe('/login/id-pass')
  })

  it('on wrong_captcha toasts WrongCaptcha, refetches captcha, and clears the captcha field', async () => {
    mockGetVerifyPageInfo.mockReturnValueOnce(ok(FAKE_PAGE))
    mockGetVerifyCaptcha
      .mockReturnValueOnce(ok(FAKE_CAPTCHA))
      .mockReturnValueOnce(ok(FAKE_CAPTCHA_2))
    mockSubmitVerify.mockReturnValueOnce(ok(WRONG_CAPTCHA))

    const ctx = mountForm()
    const wrapper = await ctx.mountIt()

    await fillVerifyAndCaptcha(wrapper, '1234', 'WRONG')
    await wrapper.get('.el-form-stub').trigger('submit')
    await flushPromises()

    expect(elMessageError).toHaveBeenCalledWith(i18nMessages['zh-TW'].WrongCaptcha)
    expect(mockGetVerifyCaptcha).toHaveBeenCalledTimes(2)
    expect(wrapper.find('[data-testid="verify-captcha-bitmap"]').attributes('src')).toBe(
      FAKE_CAPTCHA_2.image_base64,
    )
    expect(
      (wrapper.get('[data-test="verify-captcha-input"]').element as HTMLInputElement).value,
    ).toBe('')
    expect(ctx.router.currentRoute.value.path).toBe('/login/verify')
  })

  it('on wrong_auth_info toasts WrongAuthInfo and refreshes the captcha', async () => {
    mockGetVerifyPageInfo.mockReturnValueOnce(ok(FAKE_PAGE))
    mockGetVerifyCaptcha
      .mockReturnValueOnce(ok(FAKE_CAPTCHA))
      .mockReturnValueOnce(ok(FAKE_CAPTCHA_2))
    mockSubmitVerify.mockReturnValueOnce(ok(WRONG_AUTH_INFO))

    const ctx = mountForm()
    const wrapper = await ctx.mountIt()

    await fillVerifyAndCaptcha(wrapper, 'wrong-answer', 'ABCD')
    await wrapper.get('.el-form-stub').trigger('submit')
    await flushPromises()

    expect(elMessageError).toHaveBeenCalledWith(i18nMessages['zh-TW'].WrongAuthInfo)
    expect(mockGetVerifyCaptcha).toHaveBeenCalledTimes(2)
  })

  it('on server_message renders the verbatim message and refreshes the captcha', async () => {
    mockGetVerifyPageInfo.mockReturnValueOnce(ok(FAKE_PAGE))
    mockGetVerifyCaptcha
      .mockReturnValueOnce(ok(FAKE_CAPTCHA))
      .mockReturnValueOnce(ok(FAKE_CAPTCHA_2))
    const verbatim = '驗證次數過多，請稍後再試。'
    mockSubmitVerify.mockReturnValueOnce(ok(SERVER_MESSAGE(verbatim)))

    const ctx = mountForm()
    const wrapper = await ctx.mountIt()

    await fillVerifyAndCaptcha(wrapper, '1234', 'ABCD')
    await wrapper.get('.el-form-stub').trigger('submit')
    await flushPromises()

    expect(elMessageError).toHaveBeenCalledWith(verbatim)
    expect(mockGetVerifyCaptcha).toHaveBeenCalledTimes(2)
  })

  it('clicking the captcha image refreshes the bitmap and clears the captcha field', async () => {
    mockGetVerifyPageInfo.mockReturnValueOnce(ok(FAKE_PAGE))
    mockGetVerifyCaptcha
      .mockReturnValueOnce(ok(FAKE_CAPTCHA))
      .mockReturnValueOnce(ok(FAKE_CAPTCHA_2))

    const ctx = mountForm()
    const wrapper = await ctx.mountIt()

    await wrapper.get('[data-test="verify-captcha-input"]').setValue('STALE')
    await wrapper.get('[data-test="verify-captcha-image"]').trigger('click')
    await flushPromises()

    expect(mockGetVerifyCaptcha).toHaveBeenCalledTimes(2)
    expect(wrapper.find('[data-testid="verify-captcha-bitmap"]').attributes('src')).toBe(
      FAKE_CAPTCHA_2.image_base64,
    )
    expect(
      (wrapper.get('[data-test="verify-captcha-input"]').element as HTMLInputElement).value,
    ).toBe('')
    expect(mockSubmitVerify).not.toHaveBeenCalled()
  })

  it('back link navigates to /login/id-pass without calling submitVerify', async () => {
    mockGetVerifyPageInfo.mockReturnValueOnce(ok(FAKE_PAGE))
    mockGetVerifyCaptcha.mockReturnValueOnce(ok(FAKE_CAPTCHA))

    const ctx = mountForm()
    const wrapper = await ctx.mountIt()

    await wrapper.get('[data-test="verify-back"]').trigger('click')
    await flushPromises()

    expect(ctx.router.currentRoute.value.path).toBe('/login/id-pass')
    expect(mockSubmitVerify).not.toHaveBeenCalled()
  })

  it('on getVerifyPageInfo failure shows the load-failed banner and Retry button (no submit)', async () => {
    mockGetVerifyPageInfo.mockReturnValueOnce(
      err({
        code: 'beanfun.transport',
        message: 'transport',
        details: null,
      }),
    )

    const ctx = mountForm()
    const wrapper = await ctx.mountIt()

    expect(wrapper.find('[data-test="verify-load-failed"]').exists()).toBe(true)
    expect(wrapper.find('[data-test="verify-retry"]').exists()).toBe(true)
    expect(wrapper.find('[data-test="verify-submit"]').exists()).toBe(false)
    expect(mockGetVerifyCaptcha).not.toHaveBeenCalled()
  })

  it('Retry on the load-failed banner re-fetches page info and captcha', async () => {
    mockGetVerifyPageInfo
      .mockReturnValueOnce(
        err({
          code: 'beanfun.transport',
          message: 'transport',
          details: null,
        }),
      )
      .mockReturnValueOnce(ok(FAKE_PAGE))
    mockGetVerifyCaptcha.mockReturnValueOnce(ok(FAKE_CAPTCHA))

    const ctx = mountForm()
    const wrapper = await ctx.mountIt()

    await wrapper.get('[data-test="verify-retry"]').trigger('click')
    await flushPromises()

    expect(mockGetVerifyPageInfo).toHaveBeenCalledTimes(2)
    expect(mockGetVerifyCaptcha).toHaveBeenCalledTimes(1)
    expect(wrapper.find('[data-test="verify-submit"]').exists()).toBe(true)
    expect(wrapper.find('[data-test="verify-load-failed"]').exists()).toBe(false)
  })

  it('re-renders labels after a runtime locale switch', async () => {
    mockGetVerifyPageInfo.mockReturnValueOnce(ok(FAKE_PAGE))
    mockGetVerifyCaptcha.mockReturnValueOnce(ok(FAKE_CAPTCHA))

    const ctx = mountForm()
    const wrapper = await ctx.mountIt()

    expect(wrapper.find('[data-test="verify-submit"]').text()).toBe(
      i18nMessages['zh-TW'].AuthConfirm,
    )

    setLocale(ctx.i18n, 'en-US')
    await flushPromises()

    expect(wrapper.find('[data-test="verify-submit"]').text()).toBe(
      i18nMessages['en-US'].AuthConfirm,
    )
    expect(wrapper.text()).toContain(i18nMessages['en-US'].loginVerify.title)
  })
})

/**
 * P12.2 D2 — verify-page prefill + verify-intent-on-success tests.
 *
 * What this block locks down:
 *
 * 1. Mount-time prefill: when `auth.loginIntent` points at a stored
 *    account that has a saved `verify` code, both the verify input
 *    and the Remember checkbox are pre-populated.
 * 2. Mount-time prefill: missing intent → no prefill.
 * 3. Mount-time prefill: stored record without `verify` → no
 *    prefill (and Remember stays off).
 * 4. Mount-time prefill: intent present but no matching stored
 *    record → no prefill.
 * 5. Submit success: stashes the submitted verify code + Remember
 *    flag into `auth.verifyIntent` so the next IdPassForm second-
 *    pass success can fold them into the saved record.
 * 6. Submit success leaves `loginIntent` intact so IdPassForm has
 *    its credentials to retry the login from.
 */
describe('VerifyPage — P12.2 D2 prefill + verifyIntent stash', () => {
  const STORED_ALICE: Account = {
    region: 'TW',
    account_id: 'alice',
    account_name: '',
    password: 'pw',
    verify: 'V0001',
    method: 0,
    auto_login: false,
  }

  beforeEach(() => {
    setActivePinia(createPinia())
    mockGetVerifyPageInfo.mockReset()
    mockGetVerifyCaptcha.mockReset()
    mockSubmitVerify.mockReset()
    mockResumeTwLoginAfterVerify.mockReset()
    mockSaveAccount.mockReset()
    elMessageError.mockReset()
    elMessageSuccess.mockReset()
  })

  function seedLoginIntent(): void {
    const auth = useAuthStore()
    auth.setLoginIntent({
      region: 'TW',
      accountId: 'alice',
      password: 'pw',
      rememberPassword: true,
      autoLogin: false,
    })
  }

  it('prefills verify code + Remember checkbox from the stored record', async () => {
    mockGetVerifyPageInfo.mockReturnValueOnce(ok(FAKE_PAGE))
    mockGetVerifyCaptcha.mockReturnValueOnce(ok(FAKE_CAPTCHA))

    const ctx = mountForm()
    seedLoginIntent()
    const account = useAccountStore()
    account.accounts = [STORED_ALICE]

    const wrapper = await ctx.mountIt()
    expect((wrapper.get('[data-test="verify-input"]').element as HTMLInputElement).value).toBe(
      'V0001',
    )
    const rememberInput = wrapper.get('[data-test="verify-remember"] .el-checkbox-stub__input')
    expect((rememberInput.element as HTMLInputElement).checked).toBe(true)
  })

  it('does not prefill when no loginIntent is set', async () => {
    mockGetVerifyPageInfo.mockReturnValueOnce(ok(FAKE_PAGE))
    mockGetVerifyCaptcha.mockReturnValueOnce(ok(FAKE_CAPTCHA))

    const ctx = mountForm()
    const account = useAccountStore()
    account.accounts = [STORED_ALICE]

    const wrapper = await ctx.mountIt()
    expect((wrapper.get('[data-test="verify-input"]').element as HTMLInputElement).value).toBe('')
  })

  it('does not prefill when stored record has empty verify', async () => {
    mockGetVerifyPageInfo.mockReturnValueOnce(ok(FAKE_PAGE))
    mockGetVerifyCaptcha.mockReturnValueOnce(ok(FAKE_CAPTCHA))

    const ctx = mountForm()
    seedLoginIntent()
    const account = useAccountStore()
    account.accounts = [{ ...STORED_ALICE, verify: '' }]

    const wrapper = await ctx.mountIt()
    expect((wrapper.get('[data-test="verify-input"]').element as HTMLInputElement).value).toBe('')
    const rememberInput = wrapper.get('[data-test="verify-remember"] .el-checkbox-stub__input')
    expect((rememberInput.element as HTMLInputElement).checked).toBe(false)
  })

  it('does not prefill when no stored record matches the intent', async () => {
    mockGetVerifyPageInfo.mockReturnValueOnce(ok(FAKE_PAGE))
    mockGetVerifyCaptcha.mockReturnValueOnce(ok(FAKE_CAPTCHA))

    const ctx = mountForm()
    seedLoginIntent()

    const wrapper = await ctx.mountIt()
    expect((wrapper.get('[data-test="verify-input"]').element as HTMLInputElement).value).toBe('')
  })

  it('on TW success resumes the same session and lands on the account list (#313/#315/#318)', async () => {
    mockGetVerifyPageInfo.mockReturnValueOnce(ok(FAKE_PAGE))
    mockGetVerifyCaptcha.mockReturnValueOnce(ok(FAKE_CAPTCHA))
    mockSubmitVerify.mockReturnValueOnce(ok(SUCCESS))
    // Same-session AccountLogin resubmit (token-replay §4) — must NOT go
    // through a fresh loginRegular.
    mockResumeTwLoginAfterVerify.mockReturnValueOnce(
      ok({ region: 'TW', account_id: 'alice', service_code: '610074', service_region: 'T9' }),
    )
    mockSaveAccount.mockReturnValueOnce(ok([]))

    const ctx = mountForm()
    seedLoginIntent()
    const auth = useAuthStore()

    const wrapper = await ctx.mountIt()
    await fillVerifyAndCaptcha(wrapper, '4321', 'CAPX')
    await wrapper.get('.el-form-stub').trigger('submit')
    await flushPromises()

    expect(commands.resumeTwLoginAfterVerify).toHaveBeenCalledTimes(1)
    // Credentials persisted, then intents cleared, and we land on /accounts.
    expect(commands.saveAccount).toHaveBeenCalledTimes(1)
    expect(auth.loginIntent).toBeNull()
    expect(ctx.router.currentRoute.value.path).toBe('/accounts')
  })

  it('on TW success that still needs a reCAPTCHA routes to the widget page', async () => {
    mockGetVerifyPageInfo.mockReturnValueOnce(ok(FAKE_PAGE))
    mockGetVerifyCaptcha.mockReturnValueOnce(ok(FAKE_CAPTCHA))
    mockSubmitVerify.mockReturnValueOnce(ok(SUCCESS))
    mockResumeTwLoginAfterVerify.mockReturnValueOnce(
      err({ code: 'auth.recaptcha_required', message: 'again', details: { step: 'login' } }),
    )

    const ctx = mountForm()
    seedLoginIntent()
    const auth = useAuthStore()

    const wrapper = await ctx.mountIt()
    await fillVerifyAndCaptcha(wrapper, '4321', 'CAPX')
    await wrapper.get('.el-form-stub').trigger('submit')
    await flushPromises()

    expect(auth.pendingRecaptcha).toBe(true)
    expect(ctx.router.currentRoute.value.path).toBe('/login/recaptcha')
  })

  it('on HK (non-TW) success stashes verifyIntent + routes to id-pass for manual re-login', async () => {
    mockGetVerifyPageInfo.mockReturnValueOnce(ok(FAKE_PAGE))
    mockGetVerifyCaptcha.mockReturnValueOnce(ok(FAKE_CAPTCHA))
    mockSubmitVerify.mockReturnValueOnce(ok(SUCCESS))

    const ctx = mountForm()
    const auth = useAuthStore()
    auth.setLoginIntent({
      region: 'HK',
      accountId: 'bob',
      password: 'pw',
      rememberPassword: true,
      autoLogin: false,
    })

    const wrapper = await ctx.mountIt()
    await fillVerifyAndCaptcha(wrapper, '4321', 'CAPX')
    const rememberInput = wrapper.get('[data-test="verify-remember"] .el-checkbox-stub__input')
    await rememberInput.setValue(true)
    await wrapper.get('.el-form-stub').trigger('submit')
    await flushPromises()

    expect(commands.resumeTwLoginAfterVerify).not.toHaveBeenCalled()
    expect(auth.verifyIntent).toEqual({ code: '4321', remember: true })
    expect(auth.loginIntent?.accountId).toBe('bob')
    expect(ctx.router.currentRoute.value.path).toBe('/login/id-pass')
  })
})
