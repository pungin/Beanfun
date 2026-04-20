/**
 * P12.2 D8 — AddAccount modal behaviour.
 *
 * What this spec locks down (matches the D8 scope outlined in
 * `windows/AddAccount.vue`):
 *
 * 1. Empty `account_id` → `AccountNeed` warning toast; no
 *    `commands.save_account` invocation; dialog stays open
 *    (mirror WPF L43-46 + SPA-tightened "stay open" UX).
 * 2. Region picker: TW → verify field visible; switching to HK
 *    hides the field **and** clears any value the user typed so a
 *    stale verify never persists under HK (mirror WPF
 *    `region_SelectionChanged` → `initPage` "set Text = '' when
 *    collapsed" line). The payload field `verify` reflects the
 *    cleared state on submit.
 * 3. Submit happy path → `commands.save_account` called with the
 *    full 7-field payload (including `method = LOGIN_METHOD.Regular = 0`
 *    hard-code that mirrors WPF L54), the `created` event fires
 *    with `(region, accountId)`, dialog closes via
 *    `update:visible(false)`.
 * 4. Duplicate `(region, account_id)` is blocked with the
 *    `addAccountDialog.duplicateExists` toast (D8 Q8 = B), and
 *    `commands.save_account` is NOT invoked. This is the single
 *    non-trivial WPF deviation in this dialog (WPF silently
 *    over-writes); the spec hard-locks the new SPA contract.
 * 5. Empty password coerces `auto_login` to `false` in the
 *    submitted payload regardless of the checkbox state (mirror
 *    WPF L55 ternary `t_Password.Text == "" ? false :
 *    autoLogin.IsChecked`).
 * 6. Cancel button closes without invoking `commands.save_account`.
 */

import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { flushPromises, mount } from '@vue/test-utils'
import { defineComponent, h, ref, type Component } from 'vue'

import type { Account, CommandError, Result } from '../../../src/types/bindings'

const { elMessage } = vi.hoisted(() => ({
  elMessage: { error: vi.fn(), success: vi.fn(), warning: vi.fn(), info: vi.fn() },
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
        /* no-op stub */
      },
    },
  })

  /*
   * `ElSelect` mirrors Element Plus's `v-model` contract by emitting
   * `update:modelValue` directly. We render it as a native `<select>`
   * so `wrapper.get('[data-test=...]').setValue('HK')` works
   * end-to-end (the setValue helper triggers a `change` event that
   * the stub forwards as the `v-model` update). Children
   * `<el-option>` slot rendering is preserved verbatim.
   */
  const ElSelect = dc({
    name: 'ElSelectStub',
    props: { modelValue: { type: String, default: '' } },
    emits: ['update:modelValue', 'change'],
    setup(props, { slots, emit, attrs }) {
      return () =>
        hh(
          'select',
          {
            ...attrs,
            class: 'el-select-stub',
            value: props.modelValue,
            onChange: (e: Event) => {
              const next = (e.target as HTMLSelectElement).value
              emit('update:modelValue', next)
              emit('change', next)
            },
          },
          slots.default?.(),
        )
    },
  })

  const ElOption = dc({
    name: 'ElOptionStub',
    props: { value: { type: [String, Number], default: '' }, label: { type: String, default: '' } },
    setup(props) {
      return () => hh('option', { value: String(props.value) }, props.label)
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
    ElSelect,
    ElOption,
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
  }
})

vi.mock('../../../src/types/bindings', () => ({
  commands: {
    saveAccount: vi.fn(),
    loadAccounts: vi.fn(),
    removeAccount: vi.fn(),
    importRecords: vi.fn(),
    exportRecords: vi.fn(),
  },
}))

import { commands } from '../../../src/types/bindings'
import AddAccount from '../../../src/windows/AddAccount.vue'
import { useAccountStore } from '../../../src/stores/account'
import { createAppI18n, i18nMessages } from '../../../src/i18n'
import { FRONTEND_ONLY_MESSAGES } from '../../../src/i18n/messages'

const ok = <T>(data: T): Result<T, CommandError> => ({ status: 'ok', data })

function buildHarness(initialVisible = true) {
  const visibleRef = ref(initialVisible)
  const Host = defineComponent({
    name: 'AddAccountHost',
    components: { AddAccount },
    setup() {
      return { visible: visibleRef }
    },
    template: `<AddAccount v-model:visible="visible" />`,
  })
  return { visibleRef, Host }
}

describe('AddAccount modal', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    vi.spyOn(console, 'error').mockImplementation(() => {})
    vi.spyOn(console, 'warn').mockImplementation(() => {})
    for (const fn of Object.values(commands) as ReturnType<typeof vi.fn>[]) fn.mockReset()
    elMessage.error.mockReset()
    elMessage.success.mockReset()
    elMessage.warning.mockReset()
  })

  it('blocks submit and toasts AccountNeed when account ID is empty', async () => {
    const { Host } = buildHarness()
    const wrapper = mount(Host, { global: { plugins: [createAppI18n()] } })
    await flushPromises()

    await wrapper.get('[data-test="add-account-submit"]').trigger('click')
    await flushPromises()

    expect(commands.saveAccount).not.toHaveBeenCalled()
    expect(elMessage.warning).toHaveBeenCalledWith(i18nMessages['zh-TW'].AccountNeed)
    expect(wrapper.find('[data-test="add-account-dialog"]').exists()).toBe(true)
  })

  it('switching region from TW to HK hides the verify input and clears its value', async () => {
    const { Host } = buildHarness()
    const wrapper = mount(Host, { global: { plugins: [createAppI18n()] } })
    await flushPromises()

    /* Default region is TW so verify is visible at mount. */
    expect(wrapper.find('[data-test="add-account-verify"]').exists()).toBe(true)

    /* Type a verify token under TW… */
    await wrapper.get('[data-test="add-account-verify"]').setValue('VRFY-123')
    /* …then switch to HK. The watcher must clear the bound value. */
    await wrapper.get('[data-test="add-account-region"]').setValue('HK')
    await flushPromises()

    expect(wrapper.find('[data-test="add-account-verify"]').exists()).toBe(false)

    /*
     * Submit under HK and assert the saved payload's `verify` is
     * `""` — that's the entire point of the WPF parity clear: a
     * lingering `VRFY-123` in the form ref would otherwise leak
     * into the HK record.
     */
    vi.mocked(commands.saveAccount).mockResolvedValueOnce(ok([]))
    await wrapper.get('[data-test="add-account-id"]').setValue('hk_user_1')
    await wrapper.get('[data-test="add-account-submit"]').trigger('click')
    await flushPromises()

    expect(commands.saveAccount).toHaveBeenCalledTimes(1)
    const payload = vi.mocked(commands.saveAccount).mock.calls[0]![0] as Account
    expect(payload.region).toBe('HK')
    expect(payload.verify).toBe('')
  })

  it('on full success: invokes commands.saveAccount with the 7-field payload, emits created, closes', async () => {
    vi.mocked(commands.saveAccount).mockResolvedValueOnce(ok([]))

    const { visibleRef, Host } = buildHarness()
    const wrapper = mount(Host, { global: { plugins: [createAppI18n()] } })
    await flushPromises()

    /* Trim sanity: leading / trailing spaces are stripped. */
    await wrapper.get('[data-test="add-account-id"]').setValue('  alice_tw  ')
    await wrapper.get('[data-test="add-account-name"]').setValue('  Main  ')
    await wrapper.get('[data-test="add-account-password"]').setValue('p@ssw0rd')
    await wrapper.get('[data-test="add-account-verify"]').setValue('VRFY-1')
    await wrapper.get('[data-test="add-account-autologin"] input').setValue(true)
    await wrapper.get('[data-test="add-account-submit"]').trigger('click')
    await flushPromises()

    expect(commands.saveAccount).toHaveBeenCalledTimes(1)
    expect(commands.saveAccount).toHaveBeenCalledWith({
      region: 'TW',
      account_id: 'alice_tw',
      account_name: 'Main',
      password: 'p@ssw0rd',
      verify: 'VRFY-1',
      method: 0,
      auto_login: true,
    })

    const addEmits = wrapper.findComponent(AddAccount).emitted()
    expect(addEmits.created).toBeTruthy()
    expect(addEmits.created?.[0]).toEqual([{ region: 'TW', accountId: 'alice_tw' }])
    expect(visibleRef.value).toBe(false)
  })

  it('blocks submit when (region, account_id) already exists in the store, toasts duplicateExists', async () => {
    /*
     * Seed the store with a pre-existing record. The dialog reads
     * via `accountStore.findStoredAccount` (a pure local-cache
     * lookup, no IPC) so `loadAccounts` doesn't need to be mocked
     * — we drop the row directly into `accounts.value`.
     */
    setActivePinia(createPinia())
    const accountStore = useAccountStore()
    accountStore.accounts = [
      {
        region: 'TW',
        account_id: 'alice_tw',
        account_name: '',
        password: '',
        verify: '',
        method: 0,
        auto_login: false,
      },
    ]

    const { Host } = buildHarness()
    const wrapper = mount(Host, { global: { plugins: [createAppI18n()] } })
    await flushPromises()

    await wrapper.get('[data-test="add-account-id"]').setValue('alice_tw')
    await wrapper.get('[data-test="add-account-submit"]').trigger('click')
    await flushPromises()

    expect(commands.saveAccount).not.toHaveBeenCalled()
    expect(elMessage.error).toHaveBeenCalledWith(
      FRONTEND_ONLY_MESSAGES['zh-TW'].addAccountDialog.duplicateExists,
    )
    /* Dialog stays open so the user can adjust the input. */
    expect(wrapper.find('[data-test="add-account-dialog"]').exists()).toBe(true)
  })

  it('empty password coerces auto_login to false in the saved payload (WPF L55 quirk)', async () => {
    vi.mocked(commands.saveAccount).mockResolvedValueOnce(ok([]))

    const { Host } = buildHarness()
    const wrapper = mount(Host, { global: { plugins: [createAppI18n()] } })
    await flushPromises()

    /*
     * User ticks the auto-login checkbox first, then leaves the
     * password empty. WPF L55 explicitly forces `auto_login` to
     * `false` in this state to prevent a meaningless "auto-login
     * with no password" record from being saved.
     */
    await wrapper.get('[data-test="add-account-id"]').setValue('alice_tw')
    await wrapper.get('[data-test="add-account-autologin"] input').setValue(true)
    await wrapper.get('[data-test="add-account-submit"]').trigger('click')
    await flushPromises()

    expect(commands.saveAccount).toHaveBeenCalledTimes(1)
    const payload = vi.mocked(commands.saveAccount).mock.calls[0]![0] as Account
    expect(payload.password).toBe('')
    expect(payload.auto_login).toBe(false)
  })

  it('cancel button closes the dialog without invoking commands.saveAccount', async () => {
    const { visibleRef, Host } = buildHarness()
    const wrapper = mount(Host, { global: { plugins: [createAppI18n()] } })
    await flushPromises()

    await wrapper.get('[data-test="add-account-id"]').setValue('alice_tw')
    await wrapper.get('[data-test="add-account-cancel"]').trigger('click')
    await flushPromises()

    expect(commands.saveAccount).not.toHaveBeenCalled()
    expect(visibleRef.value).toBe(false)
  })
})
