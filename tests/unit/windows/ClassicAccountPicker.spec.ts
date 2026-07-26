/**
 * Specs for the Classic game-account picker — the native answer to the
 * TW GamaPass `SelectGameAccount` step (a single account never reaches
 * the frontend; the portal script submits it directly).
 */

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { flushPromises, mount } from '@vue/test-utils'
import { createPinia, setActivePinia } from 'pinia'
import { defineComponent, h } from 'vue'

import type { CommandError, Result } from '../../../src/types/bindings'

const { eventListeners } = vi.hoisted(() => ({
  eventListeners: {} as Record<string, (event: unknown) => void>,
}))

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn((event: string, cb: (event: unknown) => void) => {
    eventListeners[event] = cb
    return Promise.resolve(vi.fn())
  }),
}))

vi.mock('../../../src/types/bindings', () => ({
  commands: { classicSelectAccount: vi.fn() },
}))

vi.mock('element-plus', () => ({
  ElMessage: { error: vi.fn(), warning: vi.fn(), success: vi.fn(), info: vi.fn() },
  ElDialog: defineComponent({
    name: 'ElDialogStub',
    props: { modelValue: { type: Boolean, default: false } },
    setup(props, { slots, attrs }) {
      return () =>
        props.modelValue
          ? h('div', { ...attrs, class: 'el-dialog-stub' }, [slots.default?.(), slots.footer?.()])
          : null
    },
  }),
  ElRadioGroup: defineComponent({
    name: 'ElRadioGroupStub',
    props: { modelValue: { type: String, default: '' } },
    emits: ['update:modelValue'],
    setup(_, { slots }) {
      return () => h('div', { class: 'el-radio-group-stub' }, slots.default?.())
    },
  }),
  ElRadio: defineComponent({
    name: 'ElRadioStub',
    props: { value: { type: String, default: '' } },
    setup(props, { slots, attrs }) {
      return () => h('label', { ...attrs, 'data-value': props.value }, slots.default?.())
    },
  }),
  ElButton: defineComponent({
    name: 'ElButtonStub',
    props: {
      loading: { type: Boolean, default: false },
      disabled: { type: Boolean, default: false },
    },
    emits: ['click'],
    setup(props, { slots, emit, attrs }) {
      return () =>
        h(
          'button',
          {
            ...attrs,
            disabled: props.loading || props.disabled,
            onClick: (e: MouseEvent) => emit('click', e),
          },
          slots.default?.(),
        )
    },
  }),
}))

import { commands } from '../../../src/types/bindings'
import { createAppI18n } from '../../../src/i18n'
import ClassicAccountPicker from '../../../src/windows/ClassicAccountPicker.vue'

const ok = <T>(data: T): Promise<Result<T, CommandError>> => Promise.resolve({ status: 'ok', data })
const err = (error: CommandError): Promise<Result<never, CommandError>> =>
  Promise.resolve({ status: 'error', error })

const CHOICE_EVENT = 'classic-account-choice'
const TWO_ACCOUNTS = {
  accounts: [
    { value: 'appgame20260727laigc', name: 'William' },
    { value: 'appgame20260728other', name: 'Second' },
  ],
}

function mountPicker() {
  return mount(ClassicAccountPicker, { global: { plugins: [createAppI18n()] } })
}

describe('ClassicAccountPicker', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    for (const key of Object.keys(eventListeners)) delete eventListeners[key]
    vi.mocked(commands.classicSelectAccount).mockReturnValue(ok(null))
  })

  afterEach(() => {
    vi.clearAllMocks()
  })

  it('stays hidden until the backend offers a choice', async () => {
    const wrapper = mountPicker()
    await flushPromises()
    expect(wrapper.find('[data-test="classic-account-picker"]').exists()).toBe(false)
  })

  it('lists every offered account and preselects the first', async () => {
    const wrapper = mountPicker()
    await flushPromises()

    eventListeners[CHOICE_EVENT]?.({ payload: TWO_ACCOUNTS })
    await flushPromises()

    expect(wrapper.find('[data-test="classic-account-picker"]').exists()).toBe(true)
    expect(wrapper.text()).toContain('William')
    expect(wrapper.text()).toContain('Second')

    // Confirm without touching the radios → the preselected first one.
    await wrapper.get('[data-test="classic-account-confirm"]').trigger('click')
    await flushPromises()
    expect(commands.classicSelectAccount).toHaveBeenCalledWith('appgame20260727laigc')
  })

  it('closes after a successful selection', async () => {
    const wrapper = mountPicker()
    await flushPromises()
    eventListeners[CHOICE_EVENT]?.({ payload: TWO_ACCOUNTS })
    await flushPromises()

    await wrapper.get('[data-test="classic-account-confirm"]').trigger('click')
    await flushPromises()

    expect(wrapper.find('[data-test="classic-account-picker"]').exists()).toBe(false)
  })

  it('stays open when the portal rejects the selection so the user can retry', async () => {
    vi.mocked(commands.classicSelectAccount).mockReturnValue(
      err({ code: 'classic.portal_closed', message: 'gone', details: null }),
    )
    const wrapper = mountPicker()
    await flushPromises()
    eventListeners[CHOICE_EVENT]?.({ payload: TWO_ACCOUNTS })
    await flushPromises()

    await wrapper.get('[data-test="classic-account-confirm"]').trigger('click')
    await flushPromises()

    expect(wrapper.find('[data-test="classic-account-picker"]').exists()).toBe(true)
  })

  it('ignores an empty account list', async () => {
    const wrapper = mountPicker()
    await flushPromises()
    eventListeners[CHOICE_EVENT]?.({ payload: { accounts: [] } })
    await flushPromises()
    expect(wrapper.find('[data-test="classic-account-picker"]').exists()).toBe(false)
  })
})
