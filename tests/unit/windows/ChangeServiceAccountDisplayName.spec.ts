/**
 * P12.2 D4 — ChangeServiceAccountDisplayName modal behaviour.
 *
 * What this spec locks down (matches the D4 scope outlined in
 * `windows/ChangeServiceAccountDisplayName.vue`):
 *
 * 1. Pre-fills the input with `props.account.sname` on open.
 * 2. Empty input → `MsgDisplayNameNeed` warning toast, dialog
 *    stays open, command never invoked. (SPA-tightened parity:
 *    WPF returns false → MsgChangeDisplayNameError after dialog
 *    close; we keep the dialog open with a warning toast instead.)
 * 3. Unchanged input short-circuit (trimmed === sname) →
 *    dialog closes, command never invoked, `updated` not emitted.
 *    Mirrors WPF L2068-69:
 *      `if (newName == account.sname) return true;`
 * 4. Changed input → `account.changeServiceAccountName(trimmed,
 *    account)` invoked, dialog closes, `updated` emitted with the
 *    new sname.
 * 5. Server returns `false` → `MsgChangeDisplayNameError` error
 *    toast, dialog stays open, `updated` not emitted.
 * 6. Transport throws → dialog stays open, `updated` not emitted,
 *    `MsgChangeDisplayNameError` is **not** additionally toasted
 *    (wrapCommand already toasted the cause).
 * 7. Cancel + header-close emit `update:visible(false)` and never
 *    invoke the command.
 * 8. Re-opening the dialog after a previous close re-prefills
 *    with the *current* account's sname (so the dialog is safe
 *    to reuse across rows).
 * 9. Submitting twice in quick succession only fires one command
 *    invocation (in-flight guard).
 * 10. The store action is invoked (not the raw command), so the
 *     post-success refresh path is never bypassed.
 */

import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { flushPromises, mount } from '@vue/test-utils'
import { defineComponent, h, ref, type Component } from 'vue'

import type { CommandError, Result, ServiceAccount } from '../../../src/types/bindings'

const { elMessage } = vi.hoisted(() => ({
  elMessage: { error: vi.fn(), success: vi.fn(), warning: vi.fn(), info: vi.fn() },
}))

vi.mock('element-plus', async () => {
  const { defineComponent: dc, h: hh, watch: w, nextTick: nt } = await import('vue')
  /*
   * Stubs intentionally mirror Element Plus's two-way contract
   * for `v-model` (modelValue / update:modelValue) so the
   * consumer's computed proxy (`visible.set` →
   * `emit('update:visible', ...)`) round-trips correctly through
   * the dialog stub. The `closed` event is fired on the
   * `true → false` transition so the SUT's `handleClosed` reset
   * hook runs (the real Element Plus dialog fires it after the
   * fade-out animation; we collapse that to a `nextTick` here).
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
    ElButton,
    ElIcon,
    ElMessage: elMessage,
  }
})

vi.mock('@element-plus/icons-vue', () => {
  const stub = (name: string): Component => defineComponent({ name, render: () => h('svg') })
  return {
    Check: stub('CheckStub'),
    CircleClose: stub('CircleCloseStub'),
    EditPen: stub('EditPenStub'),
  }
})

vi.mock('../../../src/types/bindings', () => ({
  commands: {
    changeDisplayName: vi.fn(),
    refresh: vi.fn(),
    getAccounts: vi.fn(),
    addServiceAccount: vi.fn(),
    loadAccounts: vi.fn(),
    saveAccount: vi.fn(),
  },
}))

import { commands } from '../../../src/types/bindings'
import ChangeServiceAccountDisplayName from '../../../src/windows/ChangeServiceAccountDisplayName.vue'
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

const SAMPLE_ACCOUNT: ServiceAccount = {
  is_enable: true,
  visible: true,
  is_inherited: false,
  sid: 'sid-1',
  ssn: '00001',
  sname: 'Main Toon',
  screatetime: null,
  slastusedtime: null,
  sauthtype: null,
}

const OTHER_ACCOUNT: ServiceAccount = {
  ...SAMPLE_ACCOUNT,
  sid: 'sid-2',
  ssn: '00002',
  sname: 'Mule Account',
}

/**
 * Wrap the dialog in a host that owns both `visible` and the
 * target `account`, so tests can drive the v-model + the row
 * selection from the outside (mirrors how `AccountList.vue`
 * consumes the modal — a `ref<boolean>` for visibility and a
 * `ref<ServiceAccount | null>` for the in-flight target).
 */
function buildHarness(
  initialAccount: ServiceAccount | null = SAMPLE_ACCOUNT,
  initialVisible = true,
) {
  const visibleRef = ref(initialVisible)
  const accountRef = ref<ServiceAccount | null>(initialAccount)
  const Host = defineComponent({
    name: 'ChangeServiceAccountDisplayNameHost',
    components: { ChangeServiceAccountDisplayName },
    setup() {
      return { visible: visibleRef, account: accountRef }
    },
    template: `<ChangeServiceAccountDisplayName v-model:visible="visible" :account="account" />`,
  })
  return { visibleRef, accountRef, Host }
}

describe('ChangeServiceAccountDisplayName modal', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    vi.spyOn(console, 'error').mockImplementation(() => {})
    vi.spyOn(console, 'warn').mockImplementation(() => {})
    for (const fn of Object.values(commands) as ReturnType<typeof vi.fn>[]) fn.mockReset()
    elMessage.error.mockReset()
    elMessage.success.mockReset()
    elMessage.warning.mockReset()
  })

  it('pre-fills the input with the target account sname on open', async () => {
    const { Host } = buildHarness()
    const wrapper = mount(Host, { global: { plugins: [createAppI18n()] } })
    await flushPromises()

    const input = wrapper.get('[data-test="change-display-name-input"]').element as HTMLInputElement
    expect(input.value).toBe('Main Toon')
  })

  it('blocks submit and toasts MsgDisplayNameNeed when input is empty', async () => {
    const { Host } = buildHarness()
    const wrapper = mount(Host, { global: { plugins: [createAppI18n()] } })
    await flushPromises()

    await wrapper.get('[data-test="change-display-name-input"]').setValue('')
    await wrapper.get('[data-test="change-display-name-submit"]').trigger('click')
    await flushPromises()

    expect(commands.changeDisplayName).not.toHaveBeenCalled()
    expect(elMessage.warning).toHaveBeenCalledWith(i18nMessages['zh-TW'].MsgDisplayNameNeed)
    expect(wrapper.find('[data-test="change-display-name-dialog"]').exists()).toBe(true)
  })

  it('blocks submit and toasts MsgDisplayNameNeed when input is whitespace only', async () => {
    const { Host } = buildHarness()
    const wrapper = mount(Host, { global: { plugins: [createAppI18n()] } })
    await flushPromises()

    await wrapper.get('[data-test="change-display-name-input"]').setValue('   ')
    await wrapper.get('[data-test="change-display-name-submit"]').trigger('click')
    await flushPromises()

    expect(commands.changeDisplayName).not.toHaveBeenCalled()
    expect(elMessage.warning).toHaveBeenCalledWith(i18nMessages['zh-TW'].MsgDisplayNameNeed)
  })

  it('unchanged-name short-circuit: closes the dialog without invoking the command', async () => {
    /*
     * WPF L2068-69:
     *   if (newName == account.sname) return true;
     * Closing without a server round-trip avoids burning a Beanfun
     * `gamezone.ashx` call when the user clicks Save without
     * editing — the dialog should silently close.
     */
    const { visibleRef, Host } = buildHarness()
    const wrapper = mount(Host, { global: { plugins: [createAppI18n()] } })
    await flushPromises()

    /* Input is already pre-filled with 'Main Toon'; submit unchanged. */
    await wrapper.get('[data-test="change-display-name-submit"]').trigger('click')
    await flushPromises()

    expect(commands.changeDisplayName).not.toHaveBeenCalled()
    const emits = wrapper.findComponent(ChangeServiceAccountDisplayName).emitted()
    expect(emits.updated).toBeUndefined()
    expect(visibleRef.value).toBe(false)
  })

  it('unchanged-name short-circuit: trims input before comparing (whitespace-only diff is a no-op)', async () => {
    const { visibleRef, Host } = buildHarness()
    const wrapper = mount(Host, { global: { plugins: [createAppI18n()] } })
    await flushPromises()

    await wrapper.get('[data-test="change-display-name-input"]').setValue('  Main Toon  ')
    await wrapper.get('[data-test="change-display-name-submit"]').trigger('click')
    await flushPromises()

    expect(commands.changeDisplayName).not.toHaveBeenCalled()
    expect(visibleRef.value).toBe(false)
  })

  it('on full success: invokes changeDisplayName, emits updated, closes dialog', async () => {
    vi.mocked(commands.changeDisplayName).mockResolvedValueOnce({ status: 'ok', data: true })
    vi.mocked(commands.refresh).mockResolvedValueOnce({
      status: 'ok',
      data: { accounts: [], amount_limit_notice: { kind: 'none' } },
    })

    const { visibleRef, Host } = buildHarness()
    const wrapper = mount(Host, { global: { plugins: [createAppI18n()] } })
    await flushPromises()

    /*
     * Trim sanity: pad the new name with whitespace and verify
     * the trimmed value reaches the command. Mirrors the D3
     * Add-dialog convention where the SPA tightens the WPF
     * "verbatim TextBox text" contract.
     */
    await wrapper.get('[data-test="change-display-name-input"]').setValue('  Hero  ')
    await wrapper.get('[data-test="change-display-name-submit"]').trigger('click')
    await flushPromises()

    expect(commands.changeDisplayName).toHaveBeenCalledWith('Hero', SAMPLE_ACCOUNT)

    const emits = wrapper.findComponent(ChangeServiceAccountDisplayName).emitted()
    expect(emits.updated).toBeTruthy()
    expect(emits.updated?.[0]).toEqual([{ sid: 'sid-1', newName: 'Hero' }])
    expect(visibleRef.value).toBe(false)
  })

  it('on business failure (returns false): toasts MsgChangeDisplayNameError and stays open', async () => {
    vi.mocked(commands.changeDisplayName).mockResolvedValueOnce({ status: 'ok', data: false })

    const { visibleRef, Host } = buildHarness()
    const wrapper = mount(Host, { global: { plugins: [createAppI18n()] } })
    await flushPromises()

    await wrapper.get('[data-test="change-display-name-input"]').setValue('Hero')
    await wrapper.get('[data-test="change-display-name-submit"]').trigger('click')
    await flushPromises()

    expect(elMessage.error).toHaveBeenCalledWith(i18nMessages['zh-TW'].MsgChangeDisplayNameError)
    const emits = wrapper.findComponent(ChangeServiceAccountDisplayName).emitted()
    expect(emits.updated).toBeUndefined()
    expect(visibleRef.value).toBe(true)
  })

  it('on transport throw: stays open and does not emit updated (wrapCommand toasted)', async () => {
    vi.mocked(commands.changeDisplayName).mockReturnValueOnce(failure(TRANSPORT_ERROR))

    const { visibleRef, Host } = buildHarness()
    const wrapper = mount(Host, { global: { plugins: [createAppI18n()] } })
    await flushPromises()

    await wrapper.get('[data-test="change-display-name-input"]').setValue('Hero')
    await wrapper.get('[data-test="change-display-name-submit"]').trigger('click')
    await flushPromises()

    /*
     * `wrapCommand` already toasted the underlying cause through
     * its own ElMessage.error — the dialog must NOT additionally
     * toast MsgChangeDisplayNameError because that's the WPF copy
     * for a server-business-rule rejection, not for a transport
     * blow-up.
     */
    expect(elMessage.error).not.toHaveBeenCalledWith(
      i18nMessages['zh-TW'].MsgChangeDisplayNameError,
    )
    const emits = wrapper.findComponent(ChangeServiceAccountDisplayName).emitted()
    expect(emits.updated).toBeUndefined()
    expect(visibleRef.value).toBe(true)
  })

  it('cancel button closes the dialog without invoking changeDisplayName', async () => {
    const { visibleRef, Host } = buildHarness()
    const wrapper = mount(Host, { global: { plugins: [createAppI18n()] } })
    await flushPromises()

    await wrapper.get('[data-test="change-display-name-cancel"]').trigger('click')
    await flushPromises()

    expect(commands.changeDisplayName).not.toHaveBeenCalled()
    expect(visibleRef.value).toBe(false)
  })

  it('header close button also closes the dialog', async () => {
    const { visibleRef, Host } = buildHarness()
    const wrapper = mount(Host, { global: { plugins: [createAppI18n()] } })
    await flushPromises()

    await wrapper.get('[data-test="change-display-name-close"]').trigger('click')
    await flushPromises()

    expect(visibleRef.value).toBe(false)
  })

  it('reopening the dialog re-prefills with the current account sname', async () => {
    vi.mocked(commands.changeDisplayName).mockResolvedValue({ status: 'ok', data: true })
    vi.mocked(commands.refresh).mockResolvedValue({
      status: 'ok',
      data: { accounts: [], amount_limit_notice: { kind: 'none' } },
    })

    const { visibleRef, accountRef, Host } = buildHarness()
    const wrapper = mount(Host, { global: { plugins: [createAppI18n()] } })
    await flushPromises()

    /* Edit + submit so the dialog closes via the success path. */
    await wrapper.get('[data-test="change-display-name-input"]').setValue('Renamed')
    await wrapper.get('[data-test="change-display-name-submit"]').trigger('click')
    await flushPromises()

    expect(visibleRef.value).toBe(false)

    /*
     * Swap the target account and re-open. The input must show
     * the new account's sname, NOT the previous one's edited
     * value, NOT the previous one's original sname. The reset
     * runs on the `closed` event so a single tick is enough
     * before re-priming on the visibility flip.
     */
    accountRef.value = OTHER_ACCOUNT
    visibleRef.value = true
    await flushPromises()

    const reopened = wrapper.get('[data-test="change-display-name-input"]')
      .element as HTMLInputElement
    expect(reopened.value).toBe('Mule Account')
  })

  it('does not invoke changeDisplayName twice when the user double-clicks submit', async () => {
    let resolveChange!: (r: Result<boolean, CommandError>) => void
    const pending = new Promise<Result<boolean, CommandError>>((resolve) => {
      resolveChange = resolve
    })
    vi.mocked(commands.changeDisplayName).mockReturnValueOnce(pending)
    vi.mocked(commands.refresh).mockResolvedValueOnce({
      status: 'ok',
      data: { accounts: [], amount_limit_notice: { kind: 'none' } },
    })

    const { Host } = buildHarness()
    const wrapper = mount(Host, { global: { plugins: [createAppI18n()] } })
    await flushPromises()

    await wrapper.get('[data-test="change-display-name-input"]').setValue('Hero')
    await wrapper.get('[data-test="change-display-name-submit"]').trigger('click')
    await wrapper.get('[data-test="change-display-name-submit"]').trigger('click')

    expect(commands.changeDisplayName).toHaveBeenCalledTimes(1)

    resolveChange(ok(true))
    await flushPromises()
  })

  it('store integration: useAccountStore.changeServiceAccountName is invoked (not bypassed)', async () => {
    /*
     * Sanity guard against a regression where the dialog calls
     * `commands.changeDisplayName` directly instead of the store
     * action — the latter owns the post-success refresh, so
     * skipping it would silently break the AccountList live
     * update.
     */
    vi.mocked(commands.changeDisplayName).mockResolvedValueOnce({ status: 'ok', data: true })
    vi.mocked(commands.refresh).mockResolvedValueOnce({
      status: 'ok',
      data: { accounts: [], amount_limit_notice: { kind: 'none' } },
    })

    const { Host } = buildHarness()
    const wrapper = mount(Host, { global: { plugins: [createAppI18n()] } })
    await flushPromises()

    const accountStore = useAccountStore()
    const spy = vi.spyOn(accountStore, 'changeServiceAccountName')

    await wrapper.get('[data-test="change-display-name-input"]').setValue('Hero')
    await wrapper.get('[data-test="change-display-name-submit"]').trigger('click')
    await flushPromises()

    expect(spy).toHaveBeenCalledWith('Hero', SAMPLE_ACCOUNT)
  })
})
