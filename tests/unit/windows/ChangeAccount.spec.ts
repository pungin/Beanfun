/**
 * P12.2 D8 — ChangeAccount modal behaviour.
 *
 * What this spec locks down (matches the D8 scope outlined in
 * `windows/ChangeAccount.vue`):
 *
 * 1. The form pre-fills `account_name` + `auto_login` from the prop
 *    record when the dialog opens, and re-primes when the prop
 *    record changes between opens (different rows can be edited
 *    in sequence without leaking values across opens).
 * 2. The account-id display is rendered as **read-only chrome**
 *    (no `<input>` bound to it). This is the D8 Q5 = mockup parity
 *    invariant — editing the id requires the delete + re-add
 *    flow routed through D9, not in-place rename.
 * 3. Submit calls `commands.save_account` with **only**
 *    `account_name` + `auto_login` mutated; `region` /
 *    `account_id` / `password` / `verify` / `method` are forwarded
 *    verbatim from the prop record so the upsert preserves the
 *    fields the user can't see / edit here.
 * 4. Cancel button closes without invoking `commands.save_account`.
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
    Check: stub('CheckStub'),
    CircleClose: stub('CircleCloseStub'),
    EditPen: stub('EditPenStub'),
    InfoFilled: stub('InfoFilledStub'),
  }
})

vi.mock('../../../src/types/bindings', () => ({
  commands: {
    saveAccount: vi.fn(),
    loadAccounts: vi.fn(),
    removeAccount: vi.fn(),
  },
}))

import { commands } from '../../../src/types/bindings'
import ChangeAccount from '../../../src/windows/ChangeAccount.vue'
import { createAppI18n } from '../../../src/i18n'

const ok = <T>(data: T): Result<T, CommandError> => ({ status: 'ok', data })

const RECORD_ALICE: Account = {
  region: 'TW',
  account_id: 'alice_tw',
  account_name: 'Main Char',
  password: 'p@ssw0rd',
  verify: 'VRFY-1',
  method: 0,
  auto_login: true,
}

const RECORD_BOB: Account = {
  region: 'HK',
  account_id: 'bob_hk',
  account_name: 'Alt',
  password: '',
  verify: '',
  method: 0,
  auto_login: false,
}

function buildHarness(initial: { visible: boolean; account: Account | null }) {
  const visibleRef = ref(initial.visible)
  const accountRef = ref<Account | null>(initial.account)
  const Host = defineComponent({
    name: 'ChangeAccountHost',
    components: { ChangeAccount },
    setup() {
      return { visible: visibleRef, account: accountRef }
    },
    template: `<ChangeAccount v-model:visible="visible" :account="account" />`,
  })
  return { visibleRef, accountRef, Host }
}

describe('ChangeAccount modal', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    vi.spyOn(console, 'error').mockImplementation(() => {})
    vi.spyOn(console, 'warn').mockImplementation(() => {})
    for (const fn of Object.values(commands) as ReturnType<typeof vi.fn>[]) fn.mockReset()
    elMessage.error.mockReset()
    elMessage.success.mockReset()
    elMessage.warning.mockReset()
  })

  it('pre-fills account_name + auto_login from the prop record on open', async () => {
    const { Host } = buildHarness({ visible: true, account: RECORD_ALICE })
    const wrapper = mount(Host, { global: { plugins: [createAppI18n()] } })
    await flushPromises()

    const nameInput = wrapper.get('[data-test="change-account-name"]').element as HTMLInputElement
    const autoLoginInput = wrapper.get('[data-test="change-account-autologin"] input')
      .element as HTMLInputElement
    expect(nameInput.value).toBe('Main Char')
    expect(autoLoginInput.checked).toBe(true)
  })

  it('re-primes the form when the prop record changes between opens', async () => {
    const { visibleRef, accountRef, Host } = buildHarness({ visible: true, account: RECORD_ALICE })
    const wrapper = mount(Host, { global: { plugins: [createAppI18n()] } })
    await flushPromises()

    /* Close, swap the prop record, re-open. */
    visibleRef.value = false
    await flushPromises()
    accountRef.value = RECORD_BOB
    visibleRef.value = true
    await flushPromises()

    const nameInput = wrapper.get('[data-test="change-account-name"]').element as HTMLInputElement
    const autoLoginInput = wrapper.get('[data-test="change-account-autologin"] input')
      .element as HTMLInputElement
    expect(nameInput.value).toBe('Alt')
    expect(autoLoginInput.checked).toBe(false)

    /*
     * The account-id chrome must reflect the new record too — the
     * read-only display is the only surface the user can see for
     * the (region, id) identity, so a stale cache here would let
     * the user save bob_hk's name change against the alice_tw row.
     */
    expect(wrapper.get('[data-test="change-account-id-display"]').text()).toBe('bob_hk')
  })

  it('renders the account ID as read-only chrome (no input bound to it)', async () => {
    const { Host } = buildHarness({ visible: true, account: RECORD_ALICE })
    const wrapper = mount(Host, { global: { plugins: [createAppI18n()] } })
    await flushPromises()

    /*
     * Two locked invariants:
     * 1. The display element exists and shows the id text.
     * 2. There is **no** editable input wired to the id (otherwise
     *    a future refactor could silently re-introduce in-place
     *    ID editing — exactly the WPF behaviour D8 Q5 = mockup
     *    parity decided to drop).
     */
    expect(wrapper.get('[data-test="change-account-id-display"]').text()).toBe('alice_tw')
    expect(wrapper.find('[data-test="change-account-id-input"]').exists()).toBe(false)
  })

  it('on submit: forwards password / verify / method verbatim and updates only name + auto_login', async () => {
    vi.mocked(commands.saveAccount).mockResolvedValueOnce(ok([]))

    const { visibleRef, Host } = buildHarness({ visible: true, account: RECORD_ALICE })
    const wrapper = mount(Host, { global: { plugins: [createAppI18n()] } })
    await flushPromises()

    /* User changes only the alias and turns off auto-login. */
    await wrapper.get('[data-test="change-account-name"]').setValue('Renamed')
    await wrapper.get('[data-test="change-account-autologin"] input').setValue(false)
    await wrapper.get('[data-test="change-account-submit"]').trigger('click')
    await flushPromises()

    expect(commands.saveAccount).toHaveBeenCalledTimes(1)
    expect(commands.saveAccount).toHaveBeenCalledWith({
      region: 'TW',
      account_id: 'alice_tw',
      account_name: 'Renamed',
      password: 'p@ssw0rd',
      verify: 'VRFY-1',
      method: 0,
      auto_login: false,
    })

    const changeEmits = wrapper.findComponent(ChangeAccount).emitted()
    expect(changeEmits.updated).toBeTruthy()
    expect(changeEmits.updated?.[0]).toEqual([
      { region: 'TW', accountId: 'alice_tw', accountName: 'Renamed', autoLogin: false },
    ])
    expect(visibleRef.value).toBe(false)
  })

  it('cancel button closes the dialog without invoking commands.saveAccount', async () => {
    const { visibleRef, Host } = buildHarness({ visible: true, account: RECORD_ALICE })
    const wrapper = mount(Host, { global: { plugins: [createAppI18n()] } })
    await flushPromises()

    await wrapper.get('[data-test="change-account-name"]').setValue('Renamed')
    await wrapper.get('[data-test="change-account-cancel"]').trigger('click')
    await flushPromises()

    expect(commands.saveAccount).not.toHaveBeenCalled()
    expect(visibleRef.value).toBe(false)
  })
})
