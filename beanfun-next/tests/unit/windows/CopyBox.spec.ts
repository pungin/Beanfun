/**
 * P12.2 D10.1 — CopyBox modal behaviour.
 *
 * Locks down WPF parity for the generic `(title, value) + Copy`
 * dialog (`Beanfun/Windows/CopyBox.xaml(.cs)` port):
 *
 *  1. `title` prop renders in the header.
 *  2. `value` prop renders inside the read-only `el-input`.
 *  3. Copy button click → `navigator.clipboard.writeText` called
 *     once with `value` (no extra side effects).
 *  4. Copy success → `ElMessage.success` with the translated
 *     `CopyFinished` string.
 *  5. Copy failure (Promise rejection from clipboard API) →
 *     `ElMessage.error` with the translated `CopyFailed` string;
 *     the dialog stays open.
 *  6. Header close button emits `update:visible(false)` and never
 *     touches the clipboard.
 *
 * # Why mock `navigator.clipboard`
 *
 * jsdom (vitest's default DOM) does not implement the async
 * Clipboard API (W3C-clipboard polyfill is gated behind a flag).
 * We assign a vi.fn() directly to `navigator.clipboard.writeText`
 * for the success / call-args cases, then reassign to a rejecting
 * fn for the failure case — this also exercises the runtime guard
 * inside `handleCopy` when the global is absent (case 5 reuses
 * the rejection path which is the real-world failure mode users
 * hit when document focus is lost).
 *
 * # Why no `commands` / store mocks
 *
 * The dialog is fully stateless beyond `visible`; it never calls
 * an IPC command and never touches the account store. The spec is
 * intentionally narrow.
 */

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { flushPromises, mount } from '@vue/test-utils'
import { defineComponent, h, ref, type Component } from 'vue'

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

  /*
   * Stub the Element Plus input as a real `<input>` so the
   * read-only `:model-value` binding renders into a `value`
   * attribute the spec can assert directly. `inheritAttrs:false`
   * + manual `...attrs` spread mirrors the working pattern from
   * `ManageAccount.spec.ts` so `data-test` / `readonly` etc. land
   * on the inner element.
   */
  const ElInput = dc({
    name: 'ElInputStub',
    inheritAttrs: false,
    props: {
      modelValue: { type: String, default: '' },
      readonly: { type: Boolean, default: false },
    },
    emits: ['update:modelValue'],
    setup(props, { attrs, emit }) {
      return () =>
        hh('input', {
          ...attrs,
          value: props.modelValue,
          readonly: props.readonly || undefined,
          onInput: (e: Event) => emit('update:modelValue', (e.target as HTMLInputElement).value),
        })
    },
  })

  const ElMessage = {
    success: vi.fn(),
    error: vi.fn(),
    warning: vi.fn(),
    info: vi.fn(),
  }

  return { ElDialog, ElButton, ElIcon, ElInput, ElMessage }
})

vi.mock('@element-plus/icons-vue', () => {
  const stub = (name: string): Component => defineComponent({ name, render: () => h('svg') })
  return {
    CircleClose: stub('CircleCloseStub'),
    CopyDocument: stub('CopyDocumentStub'),
  }
})

import { ElMessage } from 'element-plus'
import CopyBox from '../../../src/windows/CopyBox.vue'
import { createAppI18n, i18nMessages } from '../../../src/i18n'

/* ------------------------------------------------------------------ */
/* Harness                                                             */
/* ------------------------------------------------------------------ */

function buildHarness(initial: { visible?: boolean; title?: string; value?: string } = {}) {
  const visibleRef = ref(initial.visible ?? true)
  const titleRef = ref(initial.title ?? '驗證信箱')
  const valueRef = ref(initial.value ?? 'alice@example.com')
  const Host = defineComponent({
    name: 'CopyBoxHost',
    components: { CopyBox },
    setup() {
      return { visible: visibleRef, title: titleRef, value: valueRef }
    },
    template: `<CopyBox v-model:visible="visible" :title="title" :value="value" />`,
  })
  return { visibleRef, titleRef, valueRef, Host }
}

/* ------------------------------------------------------------------ */
/* Clipboard helpers                                                   */
/* ------------------------------------------------------------------ */

const originalClipboardDescriptor = Object.getOwnPropertyDescriptor(
  globalThis.navigator,
  'clipboard',
)

function installClipboard(writeText: (value: string) => Promise<void>): ReturnType<typeof vi.fn> {
  const writeFn = vi.fn(writeText)
  Object.defineProperty(globalThis.navigator, 'clipboard', {
    configurable: true,
    value: { writeText: writeFn },
  })
  return writeFn
}

function uninstallClipboard(): void {
  if (originalClipboardDescriptor) {
    Object.defineProperty(globalThis.navigator, 'clipboard', originalClipboardDescriptor)
  } else {
    /*
     * jsdom defines `clipboard` as a getter on Navigator.prototype
     * that throws "Not Implemented"; deleting our test-installed
     * own-property restores the prototype getter, which is the
     * "missing API" behaviour we want by default.
     */
    delete (globalThis.navigator as unknown as { clipboard?: unknown }).clipboard
  }
}

/* ------------------------------------------------------------------ */
/* Specs                                                               */
/* ------------------------------------------------------------------ */

describe('CopyBox modal', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    vi.spyOn(console, 'error').mockImplementation(() => {})
    vi.spyOn(console, 'warn').mockImplementation(() => {})
    ;(ElMessage.success as ReturnType<typeof vi.fn>).mockReset()
    ;(ElMessage.error as ReturnType<typeof vi.fn>).mockReset()
  })

  afterEach(() => {
    uninstallClipboard()
  })

  it('renders the title prop in the header', async () => {
    const { Host } = buildHarness({ title: 'Custom Title' })
    const wrapper = mount(Host, { global: { plugins: [createAppI18n()] } })
    await flushPromises()

    expect(wrapper.get('[data-test="copy-box-title"]').text()).toBe('Custom Title')
  })

  it('renders the value prop inside the read-only input', async () => {
    const { Host } = buildHarness({ value: 'totally-a-secret' })
    const wrapper = mount(Host, { global: { plugins: [createAppI18n()] } })
    await flushPromises()

    const input = wrapper.get('[data-test="copy-box-value"]').element as HTMLInputElement
    expect(input.value).toBe('totally-a-secret')
    /*
     * `readonly` HTML attribute is reflected as a string ('') by
     * jsdom when set via the property; presence-not-value is the
     * stable assertion for read-only state.
     */
    expect(input.hasAttribute('readonly')).toBe(true)
  })

  it('Copy button click calls navigator.clipboard.writeText with the value', async () => {
    const writeFn = installClipboard(() => Promise.resolve())
    const { Host } = buildHarness({ value: 'paste-me' })
    const wrapper = mount(Host, { global: { plugins: [createAppI18n()] } })
    await flushPromises()

    await wrapper.get('[data-test="copy-box-copy"]').trigger('click')
    await flushPromises()

    expect(writeFn).toHaveBeenCalledTimes(1)
    expect(writeFn).toHaveBeenCalledWith('paste-me')
  })

  it('Copy success toasts the translated CopyFinished string', async () => {
    installClipboard(() => Promise.resolve())
    const { Host } = buildHarness()
    const wrapper = mount(Host, { global: { plugins: [createAppI18n()] } })
    await flushPromises()

    await wrapper.get('[data-test="copy-box-copy"]').trigger('click')
    await flushPromises()

    expect(ElMessage.success).toHaveBeenCalledTimes(1)
    expect(ElMessage.success).toHaveBeenCalledWith(i18nMessages['zh-TW'].CopyFinished)
    expect(ElMessage.error).not.toHaveBeenCalled()
  })

  it('Copy failure (clipboard rejects) toasts CopyFailed and keeps dialog open', async () => {
    installClipboard(() => Promise.reject(new Error('focus lost')))
    const { visibleRef, Host } = buildHarness()
    const wrapper = mount(Host, { global: { plugins: [createAppI18n()] } })
    await flushPromises()

    await wrapper.get('[data-test="copy-box-copy"]').trigger('click')
    await flushPromises()

    expect(ElMessage.error).toHaveBeenCalledTimes(1)
    expect(ElMessage.error).toHaveBeenCalledWith(i18nMessages['zh-TW'].CopyFailed)
    expect(ElMessage.success).not.toHaveBeenCalled()
    /*
     * Dialog must stay open so the user can retry — losing focus
     * during the click is a transient failure (e.g. notification
     * popup stole focus mid-handler), and forcing the user to
     * re-open the dialog from the source row would be a worse
     * UX than the WPF MessageBox sibling, which also kept the
     * window open.
     */
    expect(visibleRef.value).toBe(true)
  })

  it('header close button emits update:visible(false) and skips clipboard', async () => {
    const writeFn = installClipboard(() => Promise.resolve())
    const { visibleRef, Host } = buildHarness()
    const wrapper = mount(Host, { global: { plugins: [createAppI18n()] } })
    await flushPromises()

    await wrapper.get('[data-test="copy-box-close"]').trigger('click')
    await flushPromises()

    expect(visibleRef.value).toBe(false)
    expect(writeFn).not.toHaveBeenCalled()
    expect(ElMessage.success).not.toHaveBeenCalled()
    expect(ElMessage.error).not.toHaveBeenCalled()
  })
})
