/**
 * P12.3 D7 — UnconnectedGame_ChangePassword dialog behaviour.
 *
 * Locks down WPF parity for the dialog
 * (`Beanfun/Windows/UnconnectedGame_ChangePassword.xaml(.cs)` →
 * `windows/UnconnectedGame_ChangePassword.vue`):
 *
 * 1. Confirm fires
 *    [`commands.unconnectedGameChangePassword`] with
 *    `(accountIndex, email)` exactly mirroring the WPF
 *    `bfClient.UnconnectedGame_ChangePassword(service_code,
 *    service_region, list_Account.SelectedIndex, txtEmail.Text)`
 *    arg shape (service_code/region pulled by the backend from
 *    the active session — verified at the IPC contract layer).
 * 2. `kind: 'verify_code_sent'` triggers the WPF
 *    `MsgChangePassword` blocking confirm via
 *    `ElMessageBox.alert`. The verify token (`outcome.data`) is
 *    interpolated into the i18n `{0}` placeholder; literal `\r\n`
 *    sequences in the resource value are unescaped to actual
 *    newlines (mirrors WPF `Regex.Unescape`). After the user
 *    dismisses the alert the dialog closes
 *    (`update:visible(false)`) and `verify-code-sent` event
 *    fires for the parent (`AccountList.vue` D8).
 * 3. `kind: 'error_message'` writes the verbatim server text
 *    into the inline `errorMessage` red label and the dialog
 *    stays open so the user can adjust the email and retry.
 * 4. Backend transport / parse error throws via `wrapCommand`
 *    (which already toasted) and the dialog stays open. Mirrors
 *    WPF's `result == null` → `MessageBox(UnknownError)` branch
 *    but routed through the localized error pipeline instead of
 *    a duplicated generic alert.
 *
 * # Stub design
 *
 * Element Plus stubs follow the same shape as
 * `UnconnectedGame_AddAccount.spec.ts`. Additionally:
 *
 * - `ElMessageBox.alert` is a real promise-returning stub so the
 *   verify-code-success path can be observed end-to-end (we
 *   resolve the promise inside the test to simulate the user's
 *   confirm click). The mock captures the message + title for
 *   assertion against the WPF resource shape.
 */

import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { flushPromises, mount } from '@vue/test-utils'
import { defineComponent, h, ref, type Component, type VNode } from 'vue'

import type { ChangePasswordOutcome, CommandError, Result } from '../../../src/types/bindings'

const { elMessageBox } = vi.hoisted(() => ({
  elMessageBox: { alert: vi.fn() },
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
    ElButton,
    ElIcon,
    ElMessageBox: elMessageBox,
  }
})

vi.mock('@element-plus/icons-vue', () => {
  const stub = (name: string): Component => defineComponent({ name, render: () => h('svg') })
  return {
    CircleClose: stub('CircleCloseStub'),
    Key: stub('KeyStub'),
  }
})

vi.mock('../../../src/types/bindings', () => ({
  commands: {
    unconnectedGameChangePassword: vi.fn(),
  },
}))

import { commands } from '../../../src/types/bindings'
import UnconnectedGameChangePassword from '../../../src/windows/UnconnectedGame_ChangePassword.vue'
import { createAppI18n, i18nMessages } from '../../../src/i18n'

const ok = <T>(data: T): Promise<Result<T, CommandError>> => Promise.resolve({ status: 'ok', data })

const err = (error: CommandError): Promise<Result<never, CommandError>> =>
  Promise.resolve({ status: 'error', error })

const TRANSPORT_ERROR: CommandError = {
  code: 'beanfun.transport',
  message: 'connection lost',
  details: null,
}

/**
 * Wrap the dialog in a host that owns the `visible` ref so tests
 * can drive `v-model:visible` from the outside.
 */
function buildHarness(initialVisible = true, initialIndex = 2) {
  const visibleRef = ref(initialVisible)
  const Host = defineComponent({
    name: 'UnconnectedGameChangePasswordHost',
    components: { UnconnectedGameChangePassword },
    setup() {
      return { visible: visibleRef, accountIndex: initialIndex }
    },
    template: `<UnconnectedGameChangePassword v-model:visible="visible" :account-index="accountIndex" />`,
  })
  return { visibleRef, Host }
}

describe('UnconnectedGame_ChangePassword dialog', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    vi.spyOn(console, 'error').mockImplementation(() => {})
    vi.spyOn(console, 'warn').mockImplementation(() => {})
    vi.mocked(commands.unconnectedGameChangePassword).mockReset()
    elMessageBox.alert.mockReset()
  })

  it('on verify_code_sent: opens MsgChangePassword alert with unescaped CR/LF + token, then closes the dialog and emits verify-code-sent', async () => {
    vi.mocked(commands.unconnectedGameChangePassword).mockReturnValueOnce(
      ok<ChangePasswordOutcome>({ kind: 'verify_code_sent', data: 'TOKEN-123' }),
    )
    /*
     * Resolve the alert immediately to simulate the user clicking
     * Confirm. The SUT awaits this promise before closing the
     * dialog (mirrors WPF's MessageBox.Show being modal-blocking).
     */
    elMessageBox.alert.mockResolvedValueOnce('confirm')

    const { visibleRef, Host } = buildHarness()
    const wrapper = mount(Host, { global: { plugins: [createAppI18n()] } })
    await flushPromises()

    await wrapper
      .get('[data-test="unconnected-game-change-password-email"]')
      .setValue('user@example.com')
    await wrapper.get('[data-test="unconnected-game-change-password-submit"]').trigger('click')
    await flushPromises()

    expect(commands.unconnectedGameChangePassword).toHaveBeenCalledWith(2, 'user@example.com')
    expect(elMessageBox.alert).toHaveBeenCalledTimes(1)

    /*
     * First arg is a VNode (`h('pre', { ... }, body)`); the body
     * lives at `vnode.children`. Asserting the body proves both
     * the `{0}` interpolation and the `\r\n` unescape ran.
     */
    const [vnode, title] = elMessageBox.alert.mock.calls[0] as [VNode, string]
    expect(title).toBe(i18nMessages['zh-TW'].DataSended)
    const body = vnode.children as string
    expect(body).toContain('TOKEN-123')
    expect(body).not.toContain('\\r\\n')
    expect(body).toContain('\n')

    expect(visibleRef.value).toBe(false)
    expect(
      wrapper.findComponent(UnconnectedGameChangePassword).emitted()['verify-code-sent'],
    ).toBeTruthy()
  })

  it('on verify_code_sent: still closes when the user dismisses the alert via Esc / outside-click (alert promise rejects)', async () => {
    vi.mocked(commands.unconnectedGameChangePassword).mockReturnValueOnce(
      ok<ChangePasswordOutcome>({ kind: 'verify_code_sent', data: 'TOKEN-456' }),
    )
    /*
     * Element Plus rejects the alert promise on Esc / outside-click.
     * The SUT swallows that rejection because both dismissal paths
     * should behave identically (mirrors WPF's MessageBox where
     * Esc and OK both close the dialog).
     */
    elMessageBox.alert.mockRejectedValueOnce('cancel')

    const { visibleRef, Host } = buildHarness()
    const wrapper = mount(Host, { global: { plugins: [createAppI18n()] } })
    await flushPromises()

    await wrapper
      .get('[data-test="unconnected-game-change-password-email"]')
      .setValue('user@example.com')
    await wrapper.get('[data-test="unconnected-game-change-password-submit"]').trigger('click')
    await flushPromises()

    expect(visibleRef.value).toBe(false)
    expect(
      wrapper.findComponent(UnconnectedGameChangePassword).emitted()['verify-code-sent'],
    ).toBeTruthy()
  })

  it('on error_message: writes the verbatim server text into the inline error label and stays open', async () => {
    vi.mocked(commands.unconnectedGameChangePassword).mockReturnValueOnce(
      ok<ChangePasswordOutcome>({ kind: 'error_message', data: 'e-mail尚未認證' }),
    )

    const { visibleRef, Host } = buildHarness()
    const wrapper = mount(Host, { global: { plugins: [createAppI18n()] } })
    await flushPromises()

    await wrapper
      .get('[data-test="unconnected-game-change-password-email"]')
      .setValue('user@example.com')
    await wrapper.get('[data-test="unconnected-game-change-password-submit"]').trigger('click')
    await flushPromises()

    expect(wrapper.get('[data-test="unconnected-game-change-password-error"]').text()).toBe(
      'e-mail尚未認證',
    )
    expect(visibleRef.value).toBe(true)
    expect(elMessageBox.alert).not.toHaveBeenCalled()
    expect(
      wrapper.findComponent(UnconnectedGameChangePassword).emitted()['verify-code-sent'],
    ).toBeUndefined()
  })

  it('on backend throw (wrapCommand toasted): stays open and does not invoke the alert', async () => {
    vi.mocked(commands.unconnectedGameChangePassword).mockReturnValueOnce(err(TRANSPORT_ERROR))

    const { visibleRef, Host } = buildHarness()
    const wrapper = mount(Host, { global: { plugins: [createAppI18n()] } })
    await flushPromises()

    await wrapper
      .get('[data-test="unconnected-game-change-password-email"]')
      .setValue('user@example.com')
    await wrapper.get('[data-test="unconnected-game-change-password-submit"]').trigger('click')
    await flushPromises()

    expect(visibleRef.value).toBe(true)
    expect(elMessageBox.alert).not.toHaveBeenCalled()
    expect(
      wrapper.findComponent(UnconnectedGameChangePassword).emitted()['verify-code-sent'],
    ).toBeUndefined()
  })

  it('cancel button closes the dialog without invoking the change-password IPC', async () => {
    const { visibleRef, Host } = buildHarness()
    const wrapper = mount(Host, { global: { plugins: [createAppI18n()] } })
    await flushPromises()

    await wrapper.get('[data-test="unconnected-game-change-password-cancel"]').trigger('click')
    await flushPromises()

    expect(commands.unconnectedGameChangePassword).not.toHaveBeenCalled()
    expect(visibleRef.value).toBe(false)
  })

  it('reopening the dialog resets the email field and the inline error', async () => {
    vi.mocked(commands.unconnectedGameChangePassword).mockReturnValueOnce(
      ok<ChangePasswordOutcome>({ kind: 'error_message', data: '舊錯誤' }),
    )

    const { visibleRef, Host } = buildHarness()
    const wrapper = mount(Host, { global: { plugins: [createAppI18n()] } })
    await flushPromises()

    await wrapper
      .get('[data-test="unconnected-game-change-password-email"]')
      .setValue('user@example.com')
    await wrapper.get('[data-test="unconnected-game-change-password-submit"]').trigger('click')
    await flushPromises()

    expect(wrapper.get('[data-test="unconnected-game-change-password-error"]').text()).toBe(
      '舊錯誤',
    )

    /* Close + reopen via the host ref (mirrors how AccountList toggles `visible`). */
    visibleRef.value = false
    await flushPromises()
    visibleRef.value = true
    await flushPromises()

    const reopenedEmail = wrapper.get('[data-test="unconnected-game-change-password-email"]')
      .element as HTMLInputElement
    expect(reopenedEmail.value).toBe('')
    expect(wrapper.find('[data-test="unconnected-game-change-password-error"]').exists()).toBe(
      false,
    )
  })
})
