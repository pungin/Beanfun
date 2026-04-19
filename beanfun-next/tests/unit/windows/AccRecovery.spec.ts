/**
 * P12.2 D10.3 — AccRecovery dialog behaviour.
 *
 * Locks down WPF parity for the AES-128-CBC backup / restore dialog
 * (`Beanfun/Windows/AccRecovery.xaml(.cs)` port to
 * `windows/AccRecovery.vue`):
 *
 * 1. Initial render: password + data fields empty, both action
 *    buttons disabled (UX-tightening over WPF — see component
 *    docblock "Disabled buttons when password empty").
 * 2. Export happy path → `commands.backupExport(password)` called
 *    once, ciphertext fills the `data` field, `ExportDone` toast.
 * 3. Export backend failure → `wrapCommand` toasts the underlying
 *    cause; the data field stays unchanged.
 * 4. Recovery happy path → `commands.backupRestore(password,
 *    ciphertext)` called once, `RecoverySuccess` toast, `restored`
 *    event emitted, dialog auto-closes, store accounts updated to
 *    the post-restore array.
 * 5. Recovery wrong password (`storage.aes_backup_decrypt_failed`)
 *    → `MsgDecryptFailed` toast, dialog stays open.
 * 6. Recovery malformed base64 (`storage.aes_backup_invalid_ciphertext`)
 *    → `MsgDecryptFailed` toast (collapsed into the same WPF copy
 *    because the WPF catch-all doesn't distinguish them).
 * 7. Recovery JSON-invalid (`storage.json_failed`) → `RecoveryFailed`
 *    toast (the "decrypt OK but persistence path failed" branch
 *    that mirrors WPF `importRecord() == false`).
 * 8. Header close button → `update:visible(false)`, form resets
 *    on the dialog `closed` event (mirrors `AddServiceAccount.vue`
 *    reset-on-`@closed` pattern).
 * 9. Password empty → Export disabled; password set + data empty
 *    → Recovery still disabled (data also required).
 *
 * # Stub design
 *
 * Element Plus stubs follow the AddServiceAccount.spec /
 * CopyBox.spec pattern: dialog conditionally renders on
 * `modelValue`; buttons forward `disabled` to the inner element so
 * the spec can assert the disabled-state contract; inputs use
 * `inheritAttrs: false` + manual `...attrs` spread so `data-test`
 * lands on the inner DOM input.
 *
 * # Why no real `wrapCommand` translator
 *
 * The wrapCommand error toast for the Export failure case (case 3)
 * surfaces whatever the translator returns; without a registered
 * translator it falls back to `error.message`. We assert the toast
 * **was called** (i.e. the failure surface happened) without
 * pinning the exact string — that's the wrapCommand pipeline's
 * contract and lives in `tests/unit/services/invoke.spec.ts`.
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

  /*
   * Polymorphic input: `type=textarea` renders a `<textarea>` so
   * the `data` field test can `setValue` and inspect `.value`
   * exactly the same way `<input>` does. Behaviour parity with the
   * real Element Plus component which switches between
   * `<input>` / `<textarea>` based on the `type` prop.
   */
  const ElInput = dc({
    name: 'ElInputStub',
    inheritAttrs: false,
    props: {
      modelValue: { type: String, default: '' },
      type: { type: String, default: 'text' },
      disabled: { type: Boolean, default: false },
    },
    emits: ['update:modelValue'],
    setup(props, { attrs, emit }) {
      return () => {
        const tag = props.type === 'textarea' ? 'textarea' : 'input'
        return hh(tag, {
          ...attrs,
          value: props.modelValue,
          disabled: props.disabled || undefined,
          onInput: (e: Event) => emit('update:modelValue', (e.target as HTMLInputElement).value),
        })
      }
    },
  })

  const ElButton = dc({
    name: 'ElButtonStub',
    props: { disabled: { type: Boolean, default: false } },
    emits: ['click'],
    setup(props, { slots, emit, attrs }) {
      return () =>
        hh(
          'button',
          {
            ...attrs,
            class: 'el-button-stub',
            disabled: props.disabled || undefined,
            onClick: (e: MouseEvent) => {
              if (props.disabled) return
              emit('click', e)
            },
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
    CircleClose: stub('CircleCloseStub'),
    Download: stub('DownloadStub'),
    Lock: stub('LockStub'),
    Upload: stub('UploadStub'),
  }
})

vi.mock('../../../src/types/bindings', () => ({
  commands: {
    backupExport: vi.fn(),
    backupRestore: vi.fn(),
    loadAccounts: vi.fn(),
    saveAccount: vi.fn(),
    refresh: vi.fn(),
  },
}))

import { commands } from '../../../src/types/bindings'
import AccRecovery from '../../../src/windows/AccRecovery.vue'
import { useAccountStore } from '../../../src/stores/account'
import { createAppI18n, i18nMessages } from '../../../src/i18n'

const ok = <T>(data: T): Promise<Result<T, CommandError>> => Promise.resolve({ status: 'ok', data })

const err = (error: CommandError): Promise<Result<never, CommandError>> =>
  Promise.resolve({ status: 'error', error })

const TRANSPORT_ERROR: CommandError = {
  code: 'beanfun.transport',
  message: 'connection lost',
  details: null,
}
const DECRYPT_FAILED: CommandError = {
  code: 'storage.aes_backup_decrypt_failed',
  message: 'AES backup decryption failed (wrong password or tampered ciphertext)',
  details: null,
}
const INVALID_CIPHERTEXT: CommandError = {
  code: 'storage.aes_backup_invalid_ciphertext',
  message: 'AES backup ciphertext is not valid base64',
  details: { reason: 'invalid byte 33' },
}
const JSON_FAILED: CommandError = {
  code: 'storage.json_failed',
  message: 'failed to deserialize Users.dat content as JSON',
  details: { line: 3, column: 12 },
}

const SAMPLE_CIPHERTEXT = 'Zm9vYmFyMTIzNDU2Nzg5MA=='

function buildHarness(initialVisible = true) {
  const visibleRef = ref(initialVisible)
  const Host = defineComponent({
    name: 'AccRecoveryHost',
    components: { AccRecovery },
    setup() {
      return { visible: visibleRef }
    },
    template: `<AccRecovery v-model:visible="visible" />`,
  })
  return { visibleRef, Host }
}

describe('AccRecovery dialog', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    vi.spyOn(console, 'error').mockImplementation(() => {})
    vi.spyOn(console, 'warn').mockImplementation(() => {})
    for (const fn of Object.values(commands) as ReturnType<typeof vi.fn>[]) fn.mockReset()
    elMessage.error.mockReset()
    elMessage.success.mockReset()
    elMessage.warning.mockReset()
  })

  it('initial render: password + data empty, both action buttons disabled', async () => {
    const { Host } = buildHarness()
    const wrapper = mount(Host, { global: { plugins: [createAppI18n()] } })
    await flushPromises()

    const password = wrapper.get('[data-test="acc-recovery-password"]').element as HTMLInputElement
    const data = wrapper.get('[data-test="acc-recovery-data"]').element as HTMLTextAreaElement
    const exportBtn = wrapper.get('[data-test="acc-recovery-export"]').element as HTMLButtonElement
    const restoreBtn = wrapper.get('[data-test="acc-recovery-restore"]')
      .element as HTMLButtonElement

    expect(password.value).toBe('')
    expect(data.value).toBe('')
    expect(exportBtn.disabled).toBe(true)
    expect(restoreBtn.disabled).toBe(true)
  })

  it('Export happy: backupExport called, data filled, ExportDone toasted', async () => {
    vi.mocked(commands.backupExport).mockReturnValueOnce(ok(SAMPLE_CIPHERTEXT))

    const { Host } = buildHarness()
    const wrapper = mount(Host, { global: { plugins: [createAppI18n()] } })
    await flushPromises()

    await wrapper.get('[data-test="acc-recovery-password"]').setValue('s3cret')
    await wrapper.get('[data-test="acc-recovery-export"]').trigger('click')
    await flushPromises()

    expect(commands.backupExport).toHaveBeenCalledWith('s3cret')

    const data = wrapper.get('[data-test="acc-recovery-data"]').element as HTMLTextAreaElement
    expect(data.value).toBe(SAMPLE_CIPHERTEXT)
    expect(elMessage.success).toHaveBeenCalledWith(i18nMessages['zh-TW'].ExportDone)
  })

  it('Export backend failure: data unchanged, error surfaced via wrapCommand', async () => {
    vi.mocked(commands.backupExport).mockReturnValueOnce(err(TRANSPORT_ERROR))

    const { Host } = buildHarness()
    const wrapper = mount(Host, { global: { plugins: [createAppI18n()] } })
    await flushPromises()

    await wrapper.get('[data-test="acc-recovery-password"]').setValue('s3cret')
    await wrapper.get('[data-test="acc-recovery-export"]').trigger('click')
    await flushPromises()

    const data = wrapper.get('[data-test="acc-recovery-data"]').element as HTMLTextAreaElement
    expect(data.value).toBe('')
    expect(elMessage.success).not.toHaveBeenCalled()
    /*
     * `wrapCommand` always calls `ElMessage.error` (with the
     * translator-resolved string or the fallback message) on
     * failure unless `silent: true` is set — we don't pass it,
     * so the error surface is mandatory.
     */
    expect(elMessage.error).toHaveBeenCalledTimes(1)
  })

  it('Recovery happy: backupRestore called, RecoverySuccess toasted, restored emitted, dialog closes, store updated', async () => {
    const restoredAccounts = [
      {
        region: 'TW',
        account_id: 'restored',
        account_name: 'restored note',
        password: '',
        verify: '',
        method: 'IdPass',
        auto_login: false,
      },
    ]
    vi.mocked(commands.backupRestore).mockReturnValueOnce(ok(restoredAccounts))

    const { visibleRef, Host } = buildHarness()
    const wrapper = mount(Host, { global: { plugins: [createAppI18n()] } })
    await flushPromises()

    await wrapper.get('[data-test="acc-recovery-password"]').setValue('s3cret')
    await wrapper.get('[data-test="acc-recovery-data"]').setValue(SAMPLE_CIPHERTEXT)
    await wrapper.get('[data-test="acc-recovery-restore"]').trigger('click')
    await flushPromises()

    expect(commands.backupRestore).toHaveBeenCalledWith('s3cret', SAMPLE_CIPHERTEXT)
    expect(elMessage.success).toHaveBeenCalledWith(i18nMessages['zh-TW'].RecoverySuccess)
    const emits = wrapper.findComponent(AccRecovery).emitted()
    expect(emits.restored).toHaveLength(1)
    expect(visibleRef.value).toBe(false)

    const store = useAccountStore()
    expect(store.accounts).toEqual(restoredAccounts)
  })

  it('Recovery wrong password (aes_backup_decrypt_failed): MsgDecryptFailed toast, dialog stays open', async () => {
    vi.mocked(commands.backupRestore).mockReturnValueOnce(err(DECRYPT_FAILED))

    const { visibleRef, Host } = buildHarness()
    const wrapper = mount(Host, { global: { plugins: [createAppI18n()] } })
    await flushPromises()

    await wrapper.get('[data-test="acc-recovery-password"]').setValue('wrong')
    await wrapper.get('[data-test="acc-recovery-data"]').setValue(SAMPLE_CIPHERTEXT)
    await wrapper.get('[data-test="acc-recovery-restore"]').trigger('click')
    await flushPromises()

    expect(elMessage.error).toHaveBeenCalledWith(i18nMessages['zh-TW'].MsgDecryptFailed)
    expect(elMessage.success).not.toHaveBeenCalled()
    expect(visibleRef.value).toBe(true)
    const emits = wrapper.findComponent(AccRecovery).emitted()
    expect(emits.restored).toBeUndefined()
  })

  it('Recovery malformed base64 (aes_backup_invalid_ciphertext): MsgDecryptFailed toast (same WPF copy)', async () => {
    vi.mocked(commands.backupRestore).mockReturnValueOnce(err(INVALID_CIPHERTEXT))

    const { visibleRef, Host } = buildHarness()
    const wrapper = mount(Host, { global: { plugins: [createAppI18n()] } })
    await flushPromises()

    await wrapper.get('[data-test="acc-recovery-password"]').setValue('s3cret')
    await wrapper.get('[data-test="acc-recovery-data"]').setValue('not-valid-base64!!')
    await wrapper.get('[data-test="acc-recovery-restore"]').trigger('click')
    await flushPromises()

    expect(elMessage.error).toHaveBeenCalledWith(i18nMessages['zh-TW'].MsgDecryptFailed)
    expect(visibleRef.value).toBe(true)
  })

  it('Recovery import-failed (storage.json_failed): RecoveryFailed toast', async () => {
    vi.mocked(commands.backupRestore).mockReturnValueOnce(err(JSON_FAILED))

    const { visibleRef, Host } = buildHarness()
    const wrapper = mount(Host, { global: { plugins: [createAppI18n()] } })
    await flushPromises()

    await wrapper.get('[data-test="acc-recovery-password"]').setValue('s3cret')
    await wrapper.get('[data-test="acc-recovery-data"]').setValue(SAMPLE_CIPHERTEXT)
    await wrapper.get('[data-test="acc-recovery-restore"]').trigger('click')
    await flushPromises()

    expect(elMessage.error).toHaveBeenCalledWith(i18nMessages['zh-TW'].RecoveryFailed)
    /*
     * Distinct from `MsgDecryptFailed` — JSON-failed maps to
     * the "decrypt OK but persistence broken" branch (WPF
     * `importRecord() == false`). The user's call to action is
     * different: try a different backup file, not a different
     * password.
     */
    expect(elMessage.error).not.toHaveBeenCalledWith(i18nMessages['zh-TW'].MsgDecryptFailed)
    expect(visibleRef.value).toBe(true)
  })

  it('header close button: emits update:visible(false), form resets after closed', async () => {
    const { visibleRef, Host } = buildHarness()
    const wrapper = mount(Host, { global: { plugins: [createAppI18n()] } })
    await flushPromises()

    /* Fill the form first so the reset-on-close behaviour is observable. */
    await wrapper.get('[data-test="acc-recovery-password"]').setValue('seed')
    await wrapper.get('[data-test="acc-recovery-data"]').setValue('cipher')

    await wrapper.get('[data-test="acc-recovery-close"]').trigger('click')
    await flushPromises()

    expect(visibleRef.value).toBe(false)

    /*
     * Re-open and assert the form is pristine. The reset runs on
     * the dialog stub's `closed` event (which fires after a
     * `nextTick` on `true → false` transition), then the
     * component's `watch(visible)` defensive reset fires on
     * `false → true` — either route guarantees pristine state.
     */
    visibleRef.value = true
    await flushPromises()

    const password = wrapper.get('[data-test="acc-recovery-password"]').element as HTMLInputElement
    const data = wrapper.get('[data-test="acc-recovery-data"]').element as HTMLTextAreaElement
    expect(password.value).toBe('')
    expect(data.value).toBe('')
  })

  it('disabled-state contract: password empty → Export disabled; password set + data empty → Recovery disabled', async () => {
    const { Host } = buildHarness()
    const wrapper = mount(Host, { global: { plugins: [createAppI18n()] } })
    await flushPromises()

    const exportBtn = wrapper.get('[data-test="acc-recovery-export"]').element as HTMLButtonElement
    const restoreBtn = wrapper.get('[data-test="acc-recovery-restore"]')
      .element as HTMLButtonElement

    expect(exportBtn.disabled).toBe(true)
    expect(restoreBtn.disabled).toBe(true)

    await wrapper.get('[data-test="acc-recovery-password"]').setValue('seed')
    await flushPromises()

    expect(exportBtn.disabled).toBe(false)
    /*
     * Recovery additionally requires non-empty ciphertext — even
     * with a valid password, an empty `data` field would crash
     * the WPF `Convert.FromBase64String("")` call. The disabled
     * state is the visible feedback for that precondition.
     */
    expect(restoreBtn.disabled).toBe(true)

    await wrapper.get('[data-test="acc-recovery-data"]').setValue(SAMPLE_CIPHERTEXT)
    await flushPromises()

    expect(restoreBtn.disabled).toBe(false)
  })
})
