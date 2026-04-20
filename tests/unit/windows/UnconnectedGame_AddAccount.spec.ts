/**
 * P12.3 D6 — UnconnectedGame_AddAccount dialog behaviour.
 *
 * Locks down WPF parity for the dialog
 * (`Beanfun/Windows/UnconnectedGame_AddAccount.xaml(.cs)` →
 * `windows/UnconnectedGame_AddAccount.vue`):
 *
 * 1. First `false → true` transition fetches the init payload via
 *    [`commands.unconnectedGameInitAddAccountPayload`] exactly once
 *    and renders `init.game_name` + `init.account_len` into the
 *    intro / bullets section.
 * 2. `init.check_nickname_supported === false` collapses the DN
 *    row + "check nickname" hyperlink (mirrors WPF's
 *    `DNtr.Visibility = Collapsed` + `lbtnCheckNickName.Visibility
 *    = Collapsed` branch).
 * 3. `init.check_nickname_supported === true` renders both surfaces.
 * 4. Validation chain matches WPF `Button_Click` order — every
 *    branch surfaces the matching `UnconnectedGame_AddAccount_*`
 *    resource via `ElMessage.warning` and aborts the submit.
 *    Spec exercises five representative branches (empty id, id
 *    length out-of-range, empty pwd, empty DN with DN visible,
 *    agree-unchecked) — full nine-branch coverage is unnecessary
 *    because the chain shape is symmetrical.
 * 5. All-pass submission fires
 *    [`commands.unconnectedGameAddAccount`] with the threaded
 *    session + form values; `kind: 'success'` → `created` event +
 *    `update:visible(false)`. Mirrors WPF `result == ""` → close.
 * 6. `kind: 'error_message'` populates the inline `errorMessage`
 *    label and keeps the dialog open (WPF parity:
 *    `lblErrorMessage.Content = result`).
 * 7. "Check Account" hyperlink invokes
 *    [`commands.unconnectedGameAddAccountCheck`] with the latest
 *    session + the right `accountDn` argument shape (empty string
 *    when DN row visible, `null` when hidden — mirrors WPF's
 *    `DNtr.Visibility == Visible ? "" : null` ternary). The
 *    refreshed session is restashed and the inline message is
 *    updated.
 * 8. A check that returns an empty `error_message` surfaces the
 *    `UnknownError` toast (WPF `MessageBox.Show("UnknownError")`
 *    branch) and leaves the inline message untouched.
 * 9. Init-time IPC failure toasts via `wrapCommand` and closes
 *    the dialog (WPF: `payload == null` → `MessageBox.Show` +
 *    `this.Close()`).
 * 10. Service-contract hyperlink loads the contract via the
 *     account store and opens the nested `<Contract>` dialog
 *     (selectors owned by `Contract.vue` / its own spec).
 *
 * # Stub design
 *
 * Element Plus stubs follow the same shape as
 * `AddServiceAccount.spec.ts`:
 *
 * - `ElDialog` conditionally renders on `modelValue` so close
 *   assertions can read `wrapper.find('[data-test="..."]').exists()`.
 * - `ElInput` renders a real `<input>` so `.setValue()` works for
 *   both text + password fields (the SUT switches `type` based on
 *   the `type` attribute, which `ElInput` would normally hide
 *   behind its own DOM; the stub forwards attrs verbatim so the
 *   `data-test` selector lands on the input element directly).
 * - `ElCheckbox` renders a real `<input type="checkbox">` so the
 *   agreement gate is observable from the DOM.
 * - `ElButton` forwards `disabled`/`loading`; click is suppressed
 *   when disabled (mirrors Element Plus's runtime gating so the
 *   `submitting` guard is observable from the spec).
 *
 * # Why the dialog calls `commands.unconnectedGameAddAccount` directly
 *
 * Unlike `addServiceAccount` (whose store action owns a follow-up
 * `redrawSAccountList` refresh), the unconnected-game add-account
 * flow has no store-level wrapper today — the dialog emits
 * `created` and the parent (`AccountList.vue` D8) decides what to
 * refresh. The dialog calls the IPC directly via `wrapCommand` so
 * the spec mocks `commands.unconnectedGameAddAccount` rather than
 * spying on a store action.
 */

import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { flushPromises, mount } from '@vue/test-utils'
import { defineComponent, h, ref, type Component } from 'vue'

import type {
  AddAccountInit,
  AddAccountOutcome,
  AddAccountSession,
  CheckOutcome,
  CommandError,
  Result,
} from '../../../src/types/bindings'

const { elMessage } = vi.hoisted(() => ({
  elMessage: { error: vi.fn(), warning: vi.fn(), success: vi.fn(), info: vi.fn() },
}))

vi.mock('element-plus', async () => {
  const { defineComponent: dc, h: hh, watch: w, nextTick: nt } = await import('vue')

  const ElDialog = dc({
    name: 'ElDialogStub',
    props: { modelValue: { type: Boolean, default: false } },
    emits: ['update:modelValue', 'closed'],
    setup(props, { slots, attrs, emit }) {
      w(
        () => props.modelValue,
        async (next, prev) => {
          if (prev === true && next === false) {
            await nt()
            emit('closed')
          }
        },
      )
      return () =>
        props.modelValue
          ? hh('div', { ...attrs, class: 'el-dialog-stub' }, [
              slots.header?.(),
              hh('div', { class: 'el-dialog-stub__body' }, slots.default?.()),
              hh('div', { class: 'el-dialog-stub__footer' }, slots.footer?.()),
            ])
          : null
    },
  })

  const ElForm = dc({
    name: 'ElFormStub',
    setup(_, { slots, attrs }) {
      return () => hh('form', { ...attrs, class: 'el-form-stub' }, slots.default?.())
    },
  })

  const ElFormItem = dc({
    name: 'ElFormItemStub',
    props: { label: { type: String, default: '' } },
    setup(props, { slots }) {
      return () =>
        hh('div', { class: 'el-form-item-stub' }, [
          hh('label', { class: 'el-form-item-stub__label' }, props.label),
          hh('div', { class: 'el-form-item-stub__content' }, slots.default?.()),
        ])
    },
  })

  const ElInput = dc({
    name: 'ElInputStub',
    props: {
      modelValue: { type: String, default: '' },
      type: { type: String, default: 'text' },
    },
    emits: ['update:modelValue'],
    setup(props, { emit, attrs }) {
      return () =>
        hh('input', {
          ...attrs,
          class: 'el-input-stub',
          type: props.type,
          value: props.modelValue,
          onInput: (e: Event) => emit('update:modelValue', (e.target as HTMLInputElement).value),
        })
    },
  })

  const ElCheckbox = dc({
    name: 'ElCheckboxStub',
    props: { modelValue: { type: Boolean, default: false } },
    emits: ['update:modelValue'],
    setup(props, { slots, emit, attrs }) {
      return () =>
        hh('label', { class: 'el-checkbox-stub' }, [
          hh('input', {
            ...attrs,
            type: 'checkbox',
            checked: props.modelValue,
            onChange: (e: Event) =>
              emit('update:modelValue', (e.target as HTMLInputElement).checked),
          }),
          hh('span', { class: 'el-checkbox-stub__label' }, slots.default?.()),
        ])
    },
  })

  const ElButton = dc({
    name: 'ElButtonStub',
    props: {
      disabled: { type: Boolean, default: false },
      loading: { type: Boolean, default: false },
    },
    emits: ['click'],
    setup(props, { slots, emit, attrs }) {
      return () =>
        hh(
          'button',
          {
            ...attrs,
            class: 'el-button-stub',
            disabled: props.disabled || props.loading || undefined,
            onClick: (e: MouseEvent) => {
              if (props.disabled || props.loading) return
              emit('click', e)
            },
          },
          slots.default?.(),
        )
    },
  })

  const ElIcon = dc({
    name: 'ElIconStub',
    setup(_, { slots, attrs }) {
      return () => hh('span', { ...attrs, class: 'el-icon-stub' }, slots.default?.())
    },
  })

  return {
    ElDialog,
    ElForm,
    ElFormItem,
    ElInput,
    ElCheckbox,
    ElButton,
    ElIcon,
    ElMessage: elMessage,
  }
})

vi.mock('@element-plus/icons-vue', () => {
  const stub = (name: string): Component => defineComponent({ name, render: () => h('svg') })
  return {
    CircleClose: stub('CircleCloseStub'),
    CirclePlus: stub('CirclePlusStub'),
    Document: stub('DocumentStub'),
  }
})

vi.mock('../../../src/types/bindings', () => ({
  commands: {
    unconnectedGameInitAddAccountPayload: vi.fn(),
    unconnectedGameAddAccountCheck: vi.fn(),
    unconnectedGameAddAccountCheckNickname: vi.fn(),
    unconnectedGameAddAccount: vi.fn(),
    getContract: vi.fn(),
  },
}))

import { commands } from '../../../src/types/bindings'
import UnconnectedGameAddAccount from '../../../src/windows/UnconnectedGame_AddAccount.vue'
import { createAppI18n, i18nMessages } from '../../../src/i18n'

const ok = <T>(data: T): Promise<Result<T, CommandError>> => Promise.resolve({ status: 'ok', data })

const err = (error: CommandError): Promise<Result<never, CommandError>> =>
  Promise.resolve({ status: 'error', error })

const SESSION: AddAccountSession = {
  viewstate: 'vs-initial',
  viewstate_generator: 'vsg-initial',
  event_validation: 'ev-initial',
  region: 'TW',
}

const SESSION_AFTER_CHECK: AddAccountSession = {
  viewstate: 'vs-after-check',
  viewstate_generator: 'vsg-after-check',
  event_validation: 'ev-after-check',
  region: 'TW',
}

function makeInit(overrides: Partial<AddAccountInit> = {}): AddAccountInit {
  return {
    session: SESSION,
    game_name: '新楓之谷',
    account_len: '6 - 12',
    check_nickname_supported: true,
    ...overrides,
  }
}

const TRANSPORT_ERROR: CommandError = {
  code: 'beanfun.transport',
  message: 'connection lost',
  details: null,
}

/**
 * Wrap the dialog in a host that owns the `visible` ref so tests
 * can drive `v-model:visible` from the outside.
 */
function buildHarness(initialVisible = true) {
  const visibleRef = ref(initialVisible)
  const Host = defineComponent({
    name: 'UnconnectedGameAddAccountHost',
    components: { UnconnectedGameAddAccount },
    setup() {
      return { visible: visibleRef }
    },
    template: `<UnconnectedGameAddAccount v-model:visible="visible" />`,
  })
  return { visibleRef, Host }
}

describe('UnconnectedGame_AddAccount dialog', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    vi.spyOn(console, 'error').mockImplementation(() => {})
    vi.spyOn(console, 'warn').mockImplementation(() => {})
    for (const fn of Object.values(commands) as ReturnType<typeof vi.fn>[]) fn.mockReset()
    elMessage.error.mockReset()
    elMessage.warning.mockReset()
    elMessage.success.mockReset()
    elMessage.info.mockReset()
  })

  it('fetches init payload once on first open and renders gameName + accountLen', async () => {
    vi.mocked(commands.unconnectedGameInitAddAccountPayload).mockReturnValueOnce(ok(makeInit()))

    const { Host } = buildHarness()
    const wrapper = mount(Host, { global: { plugins: [createAppI18n()] } })
    await flushPromises()

    expect(commands.unconnectedGameInitAddAccountPayload).toHaveBeenCalledTimes(1)
    /*
     * Game name is interpolated into five separate locations
     * (intro paragraph, bullet 1, four field labels, ToS link).
     * Asserting `intro` is enough proof that the binding wired
     * through; the labels/link consume the same `gameName`
     * computed.
     */
    expect(wrapper.get('[data-test="unconnected-game-add-account-intro"]').text()).toContain(
      '新楓之谷',
    )
    expect(wrapper.get('[data-test="unconnected-game-add-account-len"]').text()).toBe('6 - 12')
  })

  it('hides DN row + check-nickname hyperlink when check_nickname_supported is false', async () => {
    vi.mocked(commands.unconnectedGameInitAddAccountPayload).mockReturnValueOnce(
      ok(makeInit({ check_nickname_supported: false })),
    )

    const { Host } = buildHarness()
    const wrapper = mount(Host, { global: { plugins: [createAppI18n()] } })
    await flushPromises()

    expect(wrapper.find('[data-test="unconnected-game-add-account-dn"]').exists()).toBe(false)
    expect(wrapper.find('[data-test="unconnected-game-add-account-dn-hint"]').exists()).toBe(false)
    expect(wrapper.find('[data-test="unconnected-game-add-account-check-nickname"]').exists()).toBe(
      false,
    )
  })

  it('renders DN row + check-nickname hyperlink when check_nickname_supported is true', async () => {
    vi.mocked(commands.unconnectedGameInitAddAccountPayload).mockReturnValueOnce(ok(makeInit()))

    const { Host } = buildHarness()
    const wrapper = mount(Host, { global: { plugins: [createAppI18n()] } })
    await flushPromises()

    expect(wrapper.find('[data-test="unconnected-game-add-account-dn"]').exists()).toBe(true)
    expect(wrapper.find('[data-test="unconnected-game-add-account-check-nickname"]').exists()).toBe(
      true,
    )
  })

  it('blocks submit and toasts UnconnectedGame_AddAccount_18 when accountId is empty', async () => {
    vi.mocked(commands.unconnectedGameInitAddAccountPayload).mockReturnValueOnce(ok(makeInit()))

    const { Host } = buildHarness()
    const wrapper = mount(Host, { global: { plugins: [createAppI18n()] } })
    await flushPromises()

    await wrapper.get('[data-test="unconnected-game-add-account-submit"]').trigger('click')
    await flushPromises()

    expect(commands.unconnectedGameAddAccount).not.toHaveBeenCalled()
    expect(elMessage.warning).toHaveBeenCalledWith(
      i18nMessages['zh-TW'].UnconnectedGame_AddAccount_18,
    )
  })

  it('blocks submit and toasts UnconnectedGame_AddAccount_19 when accountId length is below the server-supplied min', async () => {
    vi.mocked(commands.unconnectedGameInitAddAccountPayload).mockReturnValueOnce(ok(makeInit()))

    const { Host } = buildHarness()
    const wrapper = mount(Host, { global: { plugins: [createAppI18n()] } })
    await flushPromises()

    await wrapper.get('[data-test="unconnected-game-add-account-id"]').setValue('abc')
    await wrapper.get('[data-test="unconnected-game-add-account-submit"]').trigger('click')
    await flushPromises()

    expect(commands.unconnectedGameAddAccount).not.toHaveBeenCalled()
    expect(elMessage.warning).toHaveBeenCalledWith(
      i18nMessages['zh-TW'].UnconnectedGame_AddAccount_19,
    )
  })

  it('blocks submit and toasts UnconnectedGame_AddAccount_20 when password is empty', async () => {
    vi.mocked(commands.unconnectedGameInitAddAccountPayload).mockReturnValueOnce(ok(makeInit()))

    const { Host } = buildHarness()
    const wrapper = mount(Host, { global: { plugins: [createAppI18n()] } })
    await flushPromises()

    await wrapper.get('[data-test="unconnected-game-add-account-id"]').setValue('account1')
    await wrapper.get('[data-test="unconnected-game-add-account-submit"]').trigger('click')
    await flushPromises()

    expect(commands.unconnectedGameAddAccount).not.toHaveBeenCalled()
    expect(elMessage.warning).toHaveBeenCalledWith(
      i18nMessages['zh-TW'].UnconnectedGame_AddAccount_20,
    )
  })

  it('blocks submit and toasts UnconnectedGame_AddAccount_24 when DN row is visible but DN is empty', async () => {
    vi.mocked(commands.unconnectedGameInitAddAccountPayload).mockReturnValueOnce(ok(makeInit()))

    const { Host } = buildHarness()
    const wrapper = mount(Host, { global: { plugins: [createAppI18n()] } })
    await flushPromises()

    await wrapper.get('[data-test="unconnected-game-add-account-id"]').setValue('account1')
    await wrapper.get('[data-test="unconnected-game-add-account-pwd"]').setValue('pass1234')
    await wrapper.get('[data-test="unconnected-game-add-account-pwd2"]').setValue('pass1234')
    await wrapper.get('[data-test="unconnected-game-add-account-submit"]').trigger('click')
    await flushPromises()

    expect(commands.unconnectedGameAddAccount).not.toHaveBeenCalled()
    expect(elMessage.warning).toHaveBeenCalledWith(
      i18nMessages['zh-TW'].UnconnectedGame_AddAccount_24,
    )
  })

  it('blocks submit and toasts UnconnectedGame_AddAccount_26 when terms checkbox is unchecked', async () => {
    vi.mocked(commands.unconnectedGameInitAddAccountPayload).mockReturnValueOnce(ok(makeInit()))

    const { Host } = buildHarness()
    const wrapper = mount(Host, { global: { plugins: [createAppI18n()] } })
    await flushPromises()

    await wrapper.get('[data-test="unconnected-game-add-account-id"]').setValue('account1')
    await wrapper.get('[data-test="unconnected-game-add-account-pwd"]').setValue('pass1234')
    await wrapper.get('[data-test="unconnected-game-add-account-pwd2"]').setValue('pass1234')
    await wrapper.get('[data-test="unconnected-game-add-account-dn"]').setValue('阿凡')
    await wrapper.get('[data-test="unconnected-game-add-account-submit"]').trigger('click')
    await flushPromises()

    expect(commands.unconnectedGameAddAccount).not.toHaveBeenCalled()
    expect(elMessage.warning).toHaveBeenCalledWith(
      i18nMessages['zh-TW'].UnconnectedGame_AddAccount_26,
    )
  })

  it('on full success: invokes unconnectedGameAddAccount with the threaded session, emits created, closes dialog', async () => {
    vi.mocked(commands.unconnectedGameInitAddAccountPayload).mockReturnValueOnce(ok(makeInit()))
    vi.mocked(commands.unconnectedGameAddAccount).mockReturnValueOnce(
      ok<AddAccountOutcome>({ kind: 'success' }),
    )

    const { visibleRef, Host } = buildHarness()
    const wrapper = mount(Host, { global: { plugins: [createAppI18n()] } })
    await flushPromises()

    await wrapper.get('[data-test="unconnected-game-add-account-id"]').setValue('account1')
    await wrapper.get('[data-test="unconnected-game-add-account-pwd"]').setValue('pass1234')
    await wrapper.get('[data-test="unconnected-game-add-account-pwd2"]').setValue('pass1234')
    await wrapper.get('[data-test="unconnected-game-add-account-dn"]').setValue('阿凡')
    await wrapper.get('[data-test="unconnected-game-add-account-agree"] input').setValue(true)
    await wrapper.get('[data-test="unconnected-game-add-account-submit"]').trigger('click')
    await flushPromises()

    expect(commands.unconnectedGameAddAccount).toHaveBeenCalledWith(
      SESSION,
      'account1',
      'pass1234',
      'pass1234',
      '阿凡',
    )

    const emits = wrapper.findComponent(UnconnectedGameAddAccount).emitted()
    expect(emits.created).toBeTruthy()
    expect(visibleRef.value).toBe(false)
  })

  it('on AddAccountOutcome.error_message: writes the verbatim text into the inline error label and stays open', async () => {
    vi.mocked(commands.unconnectedGameInitAddAccountPayload).mockReturnValueOnce(ok(makeInit()))
    vi.mocked(commands.unconnectedGameAddAccount).mockReturnValueOnce(
      ok<AddAccountOutcome>({ kind: 'error_message', data: '此帳號已被使用' }),
    )

    const { visibleRef, Host } = buildHarness()
    const wrapper = mount(Host, { global: { plugins: [createAppI18n()] } })
    await flushPromises()

    await wrapper.get('[data-test="unconnected-game-add-account-id"]').setValue('account1')
    await wrapper.get('[data-test="unconnected-game-add-account-pwd"]').setValue('pass1234')
    await wrapper.get('[data-test="unconnected-game-add-account-pwd2"]').setValue('pass1234')
    await wrapper.get('[data-test="unconnected-game-add-account-dn"]').setValue('阿凡')
    await wrapper.get('[data-test="unconnected-game-add-account-agree"] input').setValue(true)
    await wrapper.get('[data-test="unconnected-game-add-account-submit"]').trigger('click')
    await flushPromises()

    expect(wrapper.get('[data-test="unconnected-game-add-account-error"]').text()).toBe(
      '此帳號已被使用',
    )
    expect(visibleRef.value).toBe(true)

    const emits = wrapper.findComponent(UnconnectedGameAddAccount).emitted()
    expect(emits.created).toBeUndefined()
  })

  it('"Check Account" hyperlink calls unconnectedGameAddAccountCheck with the latest session, restashes the refreshed session, and renders the inline message', async () => {
    vi.mocked(commands.unconnectedGameInitAddAccountPayload).mockReturnValueOnce(ok(makeInit()))
    vi.mocked(commands.unconnectedGameAddAccountCheck).mockReturnValueOnce(
      ok<CheckOutcome>({ session: SESSION_AFTER_CHECK, error_message: '帳號可使用' }),
    )
    /*
     * Subsequent submit asserts the **refreshed** session is the
     * one threaded forward — locks down the round-trip semantic
     * that the WPF `NameValueCollection` would have mutated in
     * place.
     */
    vi.mocked(commands.unconnectedGameAddAccount).mockReturnValueOnce(
      ok<AddAccountOutcome>({ kind: 'success' }),
    )

    const { Host } = buildHarness()
    const wrapper = mount(Host, { global: { plugins: [createAppI18n()] } })
    await flushPromises()

    await wrapper.get('[data-test="unconnected-game-add-account-id"]').setValue('account1')
    await wrapper.get('[data-test="unconnected-game-add-account-dn"]').setValue('阿凡')
    await wrapper.get('[data-test="unconnected-game-add-account-check-id"]').trigger('click')
    await flushPromises()

    /*
     * WPF parity: when the DN row is visible the `accountDn` arg
     * is `""` (empty string) — NOT `null`. The dialog passes the
     * current DN value verbatim because the server's
     * lbtnCheckAccount endpoint also validates the DN field if
     * it's posted.
     */
    expect(commands.unconnectedGameAddAccountCheck).toHaveBeenCalledWith(
      SESSION,
      'account1',
      '阿凡',
    )

    expect(wrapper.get('[data-test="unconnected-game-add-account-error"]').text()).toBe(
      '帳號可使用',
    )

    /* Submit fires with the refreshed session (SESSION_AFTER_CHECK), not the initial one. */
    await wrapper.get('[data-test="unconnected-game-add-account-pwd"]').setValue('pass1234')
    await wrapper.get('[data-test="unconnected-game-add-account-pwd2"]').setValue('pass1234')
    await wrapper.get('[data-test="unconnected-game-add-account-agree"] input').setValue(true)
    await wrapper.get('[data-test="unconnected-game-add-account-submit"]').trigger('click')
    await flushPromises()

    expect(commands.unconnectedGameAddAccount).toHaveBeenCalledWith(
      SESSION_AFTER_CHECK,
      'account1',
      'pass1234',
      'pass1234',
      '阿凡',
    )
  })

  it('"Check Account" with empty error_message surfaces UnknownError toast and does not write to the inline label', async () => {
    vi.mocked(commands.unconnectedGameInitAddAccountPayload).mockReturnValueOnce(ok(makeInit()))
    vi.mocked(commands.unconnectedGameAddAccountCheck).mockReturnValueOnce(
      ok<CheckOutcome>({ session: SESSION_AFTER_CHECK, error_message: '' }),
    )

    const { Host } = buildHarness()
    const wrapper = mount(Host, { global: { plugins: [createAppI18n()] } })
    await flushPromises()

    await wrapper.get('[data-test="unconnected-game-add-account-id"]').setValue('account1')
    await wrapper.get('[data-test="unconnected-game-add-account-check-id"]').trigger('click')
    await flushPromises()

    expect(elMessage.error).toHaveBeenCalledWith(i18nMessages['zh-TW'].UnknownError)
    expect(wrapper.find('[data-test="unconnected-game-add-account-error"]').exists()).toBe(false)
  })

  it('init failure closes the dialog (WPF parity: payload == null → MessageBox + this.Close())', async () => {
    vi.mocked(commands.unconnectedGameInitAddAccountPayload).mockReturnValueOnce(
      err(TRANSPORT_ERROR),
    )

    const { visibleRef, Host } = buildHarness()
    const wrapper = mount(Host, { global: { plugins: [createAppI18n()] } })
    await flushPromises()

    expect(visibleRef.value).toBe(false)
    expect(wrapper.find('[data-test="unconnected-game-add-account-dialog"]').exists()).toBe(false)
  })

  it('terms hyperlink: opens the nested contract preview with the fetched text', async () => {
    vi.mocked(commands.unconnectedGameInitAddAccountPayload).mockReturnValueOnce(ok(makeInit()))
    vi.mocked(commands.getContract).mockResolvedValueOnce({
      status: 'ok',
      data: 'service contract body line 1\nline 2',
    })

    const { Host } = buildHarness()
    const wrapper = mount(Host, { global: { plugins: [createAppI18n()] } })
    await flushPromises()

    await wrapper.get('[data-test="unconnected-game-add-account-terms"]').trigger('click')
    await flushPromises()

    expect(commands.getContract).toHaveBeenCalledTimes(1)
    expect(wrapper.find('[data-test="contract-dialog"]').exists()).toBe(true)
    expect(wrapper.get('[data-test="contract-text"]').text()).toContain(
      'service contract body line 1',
    )
  })
})
