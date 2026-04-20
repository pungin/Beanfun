/**
 * P12.2 D3 — AddServiceAccount modal behaviour.
 *
 * What this spec locks down (matches the D3 scope outlined in
 * `windows/AddServiceAccount.vue`):
 *
 * 1. Validation order matches WPF `ButtonOk_Click`:
 *    empty display name → `MsgDisplayNameNeed` toast (no submit);
 *    unchecked terms → `MsgTermsOfServiceNeed` toast (no submit).
 * 2. Both checks pass → `account.addServiceAccount(name)` called
 *    with the trimmed name, `created` event emitted, dialog closes
 *    via `update:visible(false)`.
 * 3. `account.addServiceAccount` returns `false` →
 *    `MsgCreateServiceAccountFailed` toast, dialog stays open,
 *    `created` not emitted.
 * 4. `account.addServiceAccount` throws (transport / auth error
 *    that `wrapCommand` already toasted) → dialog stays open and
 *    `created` not emitted.
 * 5. Cancel button emits `update:visible(false)` and never invokes
 *    `addServiceAccount`.
 * 6. Terms-of-service hyperlink opens the dedicated `Contract`
 *    component (P12.2 D10.2) when `getContract` returns non-empty
 *    text; opens nothing and toasts `UnknownError` when it returns
 *    `""` (WPF parity for the empty branch in `aContract_Click`).
 *    Selector contracts (`contract-dialog`, `contract-text`) are
 *    owned by `Contract.vue` — `Contract.spec.ts` covers the dialog
 *    chrome itself.
 * 7. Re-opening the dialog after a previous close resets the form
 *    (display name + terms checkbox) so stale input doesn't leak
 *    across sessions.
 */

import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { flushPromises, mount } from '@vue/test-utils'
import { defineComponent, h, ref, type Component } from 'vue'

import type { CommandError, Result } from '../../../src/types/bindings'

const { elMessage } = vi.hoisted(() => ({
  elMessage: { error: vi.fn(), success: vi.fn(), warning: vi.fn(), info: vi.fn() },
}))

vi.mock('element-plus', async () => {
  const { defineComponent: dc, h: hh, watch: w, nextTick: nt } = await import('vue')
  /*
   * Stubs intentionally mirror Element Plus's two-way contract for
   * `v-model` (modelValue / update:modelValue) so the consumer's
   * computed proxy (`visible.set` → `emit('update:visible', ...)`)
   * round-trips correctly through the dialog stub. The `closed`
   * event is fired on the `true → false` transition so the SUT's
   * `handleClosed` reset hook runs (the real Element Plus dialog
   * fires it after the fade-out animation; we collapse that to a
   * `nextTick` here).
   */
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
    props: { modelValue: { type: String, default: '' } },
    emits: ['update:modelValue'],
    setup(props, { emit, attrs }) {
      return () =>
        hh('input', {
          ...attrs,
          class: 'el-input-stub',
          value: props.modelValue,
          onInput: (e: Event) => emit('update:modelValue', (e.target as HTMLInputElement).value),
        })
    },
    methods: {
      focus(): void {
        /* no-op stub for the imperative focus call */
      },
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
    emits: ['click'],
    setup(_, { slots, emit, attrs }) {
      return () =>
        hh(
          'button',
          {
            ...attrs,
            class: 'el-button-stub',
            onClick: (e: MouseEvent) => emit('click', e),
          },
          slots.default?.(),
        )
    },
  })

  const ElIcon = dc({
    name: 'ElIconStub',
    setup(_, { slots }) {
      return () => hh('span', { class: 'el-icon-stub' }, slots.default?.())
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
    addServiceAccount: vi.fn(),
    getContract: vi.fn(),
    refresh: vi.fn(),
    getAccounts: vi.fn(),
    loadAccounts: vi.fn(),
    saveAccount: vi.fn(),
  },
}))

import { commands } from '../../../src/types/bindings'
import AddServiceAccount from '../../../src/windows/AddServiceAccount.vue'
import { useAccountStore } from '../../../src/stores/account'
import { createAppI18n, i18nMessages } from '../../../src/i18n'

const ok = <T>(data: T): Promise<Result<T, CommandError>> => Promise.resolve({ status: 'ok', data })

const failure = (error: CommandError): Promise<Result<never, CommandError>> =>
  Promise.resolve({ status: 'error', error })

const TRANSPORT_ERROR: CommandError = {
  code: 'beanfun.transport',
  message: 'connection lost',
  details: null,
}

/**
 * Wrap the dialog in a host that owns the `visible` ref so tests
 * can drive `v-model:visible` from the outside (mirrors how a real
 * page like `AccountList.vue` consumes the modal).
 */
function buildHarness(initialVisible = true) {
  const visibleRef = ref(initialVisible)
  const Host = defineComponent({
    name: 'AddServiceAccountHost',
    components: { AddServiceAccount },
    setup() {
      return { visible: visibleRef }
    },
    template: `<AddServiceAccount v-model:visible="visible" />`,
  })
  return { visibleRef, Host }
}

describe('AddServiceAccount modal', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    vi.spyOn(console, 'error').mockImplementation(() => {})
    vi.spyOn(console, 'warn').mockImplementation(() => {})
    for (const fn of Object.values(commands) as ReturnType<typeof vi.fn>[]) fn.mockReset()
    elMessage.error.mockReset()
    elMessage.success.mockReset()
    elMessage.warning.mockReset()
  })

  it('blocks submit and toasts MsgDisplayNameNeed when display name is empty', async () => {
    const { Host } = buildHarness()
    const wrapper = mount(Host, { global: { plugins: [createAppI18n()] } })
    await flushPromises()

    await wrapper.get('[data-test="add-service-account-submit"]').trigger('click')
    await flushPromises()

    expect(commands.addServiceAccount).not.toHaveBeenCalled()
    expect(elMessage.warning).toHaveBeenCalledWith(i18nMessages['zh-TW'].MsgDisplayNameNeed)
    expect(wrapper.find('[data-test="add-service-account-dialog"]').exists()).toBe(true)
  })

  it('blocks submit and toasts MsgTermsOfServiceNeed when terms checkbox is unchecked', async () => {
    const { Host } = buildHarness()
    const wrapper = mount(Host, { global: { plugins: [createAppI18n()] } })
    await flushPromises()

    await wrapper.get('[data-test="add-service-account-name"]').setValue('Toon')
    await wrapper.get('[data-test="add-service-account-submit"]').trigger('click')
    await flushPromises()

    expect(commands.addServiceAccount).not.toHaveBeenCalled()
    expect(elMessage.warning).toHaveBeenCalledWith(i18nMessages['zh-TW'].MsgTermsOfServiceNeed)
    expect(wrapper.find('[data-test="add-service-account-dialog"]').exists()).toBe(true)
  })

  it('on full success: invokes addServiceAccount, emits created, closes dialog', async () => {
    vi.mocked(commands.addServiceAccount).mockResolvedValueOnce({ status: 'ok', data: true })
    vi.mocked(commands.refresh).mockResolvedValueOnce({
      status: 'ok',
      data: { accounts: [], amount_limit_notice: { kind: 'none' } },
    })

    const { visibleRef, Host } = buildHarness()
    const wrapper = mount(Host, { global: { plugins: [createAppI18n()] } })
    await flushPromises()

    /*
     * Trim sanity: pad the input with whitespace and verify the
     * store action receives the trimmed value (matches the WPF
     * `txtNewServiceAccountDisplayName.Text` round-trip — WPF
     * happens to not trim, but the SPA tightens the contract
     * because Element Plus inputs preserve leading/trailing space
     * far more readily than WPF TextBoxes).
     */
    await wrapper.get('[data-test="add-service-account-name"]').setValue('  Toon  ')
    await wrapper.get('[data-test="add-service-account-agree"] input').setValue(true)
    await wrapper.get('[data-test="add-service-account-submit"]').trigger('click')
    await flushPromises()

    expect(commands.addServiceAccount).toHaveBeenCalledWith('Toon')

    const addSvcEmits = wrapper.findComponent(AddServiceAccount).emitted()
    expect(addSvcEmits.created).toBeTruthy()
    expect(addSvcEmits.created?.[0]).toEqual(['Toon'])
    expect(visibleRef.value).toBe(false)
  })

  it('on business failure (returns false): toasts MsgCreateServiceAccountFailed and stays open', async () => {
    vi.mocked(commands.addServiceAccount).mockResolvedValueOnce({ status: 'ok', data: false })

    const { visibleRef, Host } = buildHarness()
    const wrapper = mount(Host, { global: { plugins: [createAppI18n()] } })
    await flushPromises()

    await wrapper.get('[data-test="add-service-account-name"]').setValue('Toon')
    await wrapper.get('[data-test="add-service-account-agree"] input').setValue(true)
    await wrapper.get('[data-test="add-service-account-submit"]').trigger('click')
    await flushPromises()

    expect(elMessage.error).toHaveBeenCalledWith(
      i18nMessages['zh-TW'].MsgCreateServiceAccountFailed,
    )
    const addSvcEmits = wrapper.findComponent(AddServiceAccount).emitted()
    expect(addSvcEmits.created).toBeUndefined()
    expect(visibleRef.value).toBe(true)
  })

  it('on transport throw: stays open and does not emit created (wrapCommand toasted)', async () => {
    vi.mocked(commands.addServiceAccount).mockReturnValueOnce(failure(TRANSPORT_ERROR))

    const { visibleRef, Host } = buildHarness()
    const wrapper = mount(Host, { global: { plugins: [createAppI18n()] } })
    await flushPromises()

    await wrapper.get('[data-test="add-service-account-name"]').setValue('Toon')
    await wrapper.get('[data-test="add-service-account-agree"] input').setValue(true)
    await wrapper.get('[data-test="add-service-account-submit"]').trigger('click')
    await flushPromises()

    /*
     * `wrapCommand` already toasted the cause through its own
     * ElMessage.error — the dialog must NOT additionally toast
     * MsgCreateServiceAccountFailed because that's the WPF copy
     * for a server-business-rule rejection, not for a transport
     * blow-up.
     */
    expect(elMessage.error).not.toHaveBeenCalledWith(
      i18nMessages['zh-TW'].MsgCreateServiceAccountFailed,
    )
    const addSvcEmits = wrapper.findComponent(AddServiceAccount).emitted()
    expect(addSvcEmits.created).toBeUndefined()
    expect(visibleRef.value).toBe(true)
  })

  it('cancel button closes the dialog without invoking addServiceAccount', async () => {
    const { visibleRef, Host } = buildHarness()
    const wrapper = mount(Host, { global: { plugins: [createAppI18n()] } })
    await flushPromises()

    await wrapper.get('[data-test="add-service-account-cancel"]').trigger('click')
    await flushPromises()

    expect(commands.addServiceAccount).not.toHaveBeenCalled()
    expect(visibleRef.value).toBe(false)
  })

  it('header close button also closes the dialog', async () => {
    const { visibleRef, Host } = buildHarness()
    const wrapper = mount(Host, { global: { plugins: [createAppI18n()] } })
    await flushPromises()

    await wrapper.get('[data-test="add-service-account-close"]').trigger('click')
    await flushPromises()

    expect(visibleRef.value).toBe(false)
  })

  it('terms hyperlink: opens the nested contract preview with the fetched text', async () => {
    vi.mocked(commands.getContract).mockResolvedValueOnce({
      status: 'ok',
      data: 'service contract body line 1\nline 2',
    })

    const { Host } = buildHarness()
    const wrapper = mount(Host, { global: { plugins: [createAppI18n()] } })
    await flushPromises()

    await wrapper.get('[data-test="add-service-account-terms"]').trigger('click')
    await flushPromises()

    expect(commands.getContract).toHaveBeenCalledTimes(1)
    expect(wrapper.find('[data-test="contract-dialog"]').exists()).toBe(true)
    expect(wrapper.get('[data-test="contract-text"]').text()).toContain(
      'service contract body line 1',
    )
  })

  it('terms hyperlink: empty contract surfaces UnknownError and does not open preview', async () => {
    vi.mocked(commands.getContract).mockResolvedValueOnce({ status: 'ok', data: '' })

    const { Host } = buildHarness()
    const wrapper = mount(Host, { global: { plugins: [createAppI18n()] } })
    await flushPromises()

    await wrapper.get('[data-test="add-service-account-terms"]').trigger('click')
    await flushPromises()

    expect(elMessage.error).toHaveBeenCalledWith(i18nMessages['zh-TW'].UnknownError)
    expect(wrapper.find('[data-test="contract-dialog"]').exists()).toBe(false)
  })

  it('reopening the dialog resets display name and terms checkbox', async () => {
    vi.mocked(commands.addServiceAccount).mockResolvedValue({ status: 'ok', data: true })
    vi.mocked(commands.refresh).mockResolvedValue({
      status: 'ok',
      data: { accounts: [], amount_limit_notice: { kind: 'none' } },
    })

    const { visibleRef, Host } = buildHarness()
    const wrapper = mount(Host, { global: { plugins: [createAppI18n()] } })
    await flushPromises()

    /* Fill + submit so the dialog closes via the success path. */
    await wrapper.get('[data-test="add-service-account-name"]').setValue('First')
    await wrapper.get('[data-test="add-service-account-agree"] input').setValue(true)
    await wrapper.get('[data-test="add-service-account-submit"]').trigger('click')
    await flushPromises()

    expect(visibleRef.value).toBe(false)

    /*
     * Re-open and assert the form is pristine. The reset runs on
     * the dialog's `closed` event (after the fade-out animation in
     * the real component); the stub fires it synchronously when
     * `modelValue` flips to false, so a single tick is enough.
     */
    visibleRef.value = true
    await flushPromises()

    const reopenedName = wrapper.get('[data-test="add-service-account-name"]')
      .element as HTMLInputElement
    const reopenedAgree = wrapper.get('[data-test="add-service-account-agree"] input')
      .element as HTMLInputElement
    expect(reopenedName.value).toBe('')
    expect(reopenedAgree.checked).toBe(false)
  })

  it('does not invoke addServiceAccount twice when the user double-clicks submit', async () => {
    /*
     * `submitting.value` guard: while a submit is in flight the
     * second click must be a no-op. Without the guard the user
     * could trigger duplicate `add_service_account` POSTs to the
     * Beanfun server, each potentially racing the `redrawSAccountList`
     * refresh — exactly the kind of WPF-vs-SPA divergence we want
     * to lock down explicitly.
     */
    let resolveAdd!: (r: Result<boolean, CommandError>) => void
    const pending = new Promise<Result<boolean, CommandError>>((resolve) => {
      resolveAdd = resolve
    })
    vi.mocked(commands.addServiceAccount).mockReturnValueOnce(pending)
    vi.mocked(commands.refresh).mockResolvedValueOnce({
      status: 'ok',
      data: { accounts: [], amount_limit_notice: { kind: 'none' } },
    })

    const { Host } = buildHarness()
    const wrapper = mount(Host, { global: { plugins: [createAppI18n()] } })
    await flushPromises()

    await wrapper.get('[data-test="add-service-account-name"]').setValue('Toon')
    await wrapper.get('[data-test="add-service-account-agree"] input').setValue(true)
    await wrapper.get('[data-test="add-service-account-submit"]').trigger('click')
    await wrapper.get('[data-test="add-service-account-submit"]').trigger('click')

    expect(commands.addServiceAccount).toHaveBeenCalledTimes(1)

    resolveAdd(ok(true))
    await flushPromises()
  })

  it('store integration: useAccountStore.addServiceAccount is invoked (not bypassed via raw command)', async () => {
    /*
     * Sanity guard against a regression where the dialog calls
     * `commands.addServiceAccount` directly instead of the store
     * action — the latter owns the post-success refresh, so
     * skipping it would silently break the AccountList live update.
     */
    vi.mocked(commands.addServiceAccount).mockResolvedValueOnce({ status: 'ok', data: true })
    vi.mocked(commands.refresh).mockResolvedValueOnce({
      status: 'ok',
      data: { accounts: [], amount_limit_notice: { kind: 'none' } },
    })

    const { Host } = buildHarness()
    const wrapper = mount(Host, { global: { plugins: [createAppI18n()] } })
    await flushPromises()

    const account = useAccountStore()
    const spy = vi.spyOn(account, 'addServiceAccount')

    await wrapper.get('[data-test="add-service-account-name"]').setValue('Toon')
    await wrapper.get('[data-test="add-service-account-agree"] input').setValue(true)
    await wrapper.get('[data-test="add-service-account-submit"]').trigger('click')
    await flushPromises()

    expect(spy).toHaveBeenCalledWith('Toon')
  })
})
