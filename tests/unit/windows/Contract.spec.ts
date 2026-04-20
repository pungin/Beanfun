/**
 * P12.2 D10.2 — Contract dialog behaviour.
 *
 * What this spec locks down (matches the D10.2 scope outlined in
 * `windows/Contract.vue`):
 *
 * 1. Renders the supplied contract `text` verbatim inside the body
 *    `<pre>` (whitespace / line breaks preserved — WPF parity for
 *    the read-only TextBox with `TextWrapping="Wrap"`).
 * 2. Default title resolves through i18n to the `TermsOfService`
 *    label (mirrors WPF's `{DynamicResource TermsOfService}`).
 * 3. Custom title prop overrides the default — and a non-resource
 *    string is rendered verbatim (so callers can pass an
 *    already-translated server label without double lookup).
 * 4. Close button emits `update:visible(false)` (caller drives the
 *    actual dismissal via v-model).
 * 5. Confirm footer button emits `update:visible(false)` (parity
 *    with the established `windows/*.vue` "single accept action"
 *    pattern).
 */

import { beforeEach, describe, expect, it, vi } from 'vitest'
import { flushPromises, mount } from '@vue/test-utils'
import { defineComponent, h, ref, type Component } from 'vue'

vi.mock('element-plus', async () => {
  const { defineComponent: dc, h: hh } = await import('vue')

  /*
   * Same stub shape as `AddServiceAccount.spec.ts` and
   * `CopyBox.spec.ts` — we don't need the `closed` event here
   * because Contract has no internal reset hook (stateless beyond
   * the v-model proxy).
   */
  const ElDialog = dc({
    name: 'ElDialogStub',
    props: { modelValue: { type: Boolean, default: false } },
    emits: ['update:modelValue'],
    setup(props, { slots, attrs }) {
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

  return { ElDialog, ElButton, ElIcon }
})

vi.mock('@element-plus/icons-vue', () => {
  const stub = (name: string): Component => defineComponent({ name, render: () => h('svg') })
  return {
    CircleClose: stub('CircleCloseStub'),
    Document: stub('DocumentStub'),
  }
})

import Contract from '../../../src/windows/Contract.vue'
import { createAppI18n, i18nMessages } from '../../../src/i18n'

/**
 * Wrap the dialog in a host that owns the `visible` ref so tests
 * can drive `v-model:visible` from the outside (mirrors how a real
 * caller like `AddServiceAccount.vue` consumes the modal).
 */
function buildHarness(initialVisible = true, title?: string, text = '') {
  const visibleRef = ref(initialVisible)
  const Host = defineComponent({
    name: 'ContractHost',
    components: { Contract },
    setup() {
      return { visible: visibleRef, text, title }
    },
    template: title
      ? `<Contract v-model:visible="visible" :text="text" :title="title" />`
      : `<Contract v-model:visible="visible" :text="text" />`,
  })
  return { visibleRef, Host }
}

describe('Contract dialog', () => {
  beforeEach(() => {
    vi.spyOn(console, 'error').mockImplementation(() => {})
    vi.spyOn(console, 'warn').mockImplementation(() => {})
  })

  it('renders the contract text body verbatim, preserving whitespace', async () => {
    /*
     * Multi-line + leading/trailing whitespace round-trip — `<pre>`
     * + `white-space: pre-wrap` should preserve every byte the
     * caller passed in. Element Plus dialogs strip outer whitespace
     * via their slot wrappers, so we read `.text()` (normalised) and
     * assert the **content lines** show up in order, then assert the
     * raw `textContent` of the `<pre>` keeps the explicit `\n`
     * separators.
     */
    const body = ['line one', '  indented two', '', 'line four'].join('\n')
    const { Host } = buildHarness(true, undefined, body)
    const wrapper = mount(Host, { global: { plugins: [createAppI18n()] } })
    await flushPromises()

    const pre = wrapper.get('[data-test="contract-text"]')
    const raw = (pre.element as HTMLElement).textContent ?? ''
    expect(raw).toBe(body)
  })

  it('defaults the title to the TermsOfService i18n label', async () => {
    const { Host } = buildHarness(true, undefined, 'body')
    const wrapper = mount(Host, { global: { plugins: [createAppI18n()] } })
    await flushPromises()

    expect(wrapper.get('[data-test="contract-title"]').text()).toBe(
      i18nMessages['zh-TW'].TermsOfService,
    )
  })

  it('renders a custom title prop verbatim when it is not an i18n key', async () => {
    /*
     * Caller can pass an already-translated server-supplied label
     * (e.g. unconnected-game ToS variant). The component must not
     * double-look-up such literals — `t('Server-side title')` would
     * return the same string back, so the resolver returns the
     * literal as-is. We verify the literal makes it to the DOM.
     */
    const literal = '橘子數位 服務條款 v2024.08'
    const { Host } = buildHarness(true, literal, 'body')
    const wrapper = mount(Host, { global: { plugins: [createAppI18n()] } })
    await flushPromises()

    expect(wrapper.get('[data-test="contract-title"]').text()).toBe(literal)
  })

  it('header close button emits update:visible(false)', async () => {
    const { visibleRef, Host } = buildHarness(true, undefined, 'body')
    const wrapper = mount(Host, { global: { plugins: [createAppI18n()] } })
    await flushPromises()

    await wrapper.get('[data-test="contract-close"]').trigger('click')
    await flushPromises()

    expect(visibleRef.value).toBe(false)
    /*
     * Dialog stub conditionally renders on `modelValue`, so once
     * `visible` flips false the dialog (and its body) unmounts.
     * Verifying the unmount proves the v-model round-trip closed
     * the props loop end-to-end (host ref → child prop → child
     * emit → host ref).
     */
    expect(wrapper.find('[data-test="contract-dialog"]').exists()).toBe(false)
  })

  it('footer Confirm button emits update:visible(false)', async () => {
    const { visibleRef, Host } = buildHarness(true, undefined, 'body')
    const wrapper = mount(Host, { global: { plugins: [createAppI18n()] } })
    await flushPromises()

    await wrapper.get('[data-test="contract-confirm"]').trigger('click')
    await flushPromises()

    expect(visibleRef.value).toBe(false)
    expect(wrapper.find('[data-test="contract-dialog"]').exists()).toBe(false)
  })
})
