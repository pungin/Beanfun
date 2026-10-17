import { beforeEach, describe, expect, it, vi } from 'vitest'
import { flushPromises, mount } from '@vue/test-utils'
import { defineComponent, h } from 'vue'

const context = vi.hoisted(() => ({ label: 'main' }))

vi.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: () => ({ label: context.label }),
}))
vi.mock('vue-i18n', () => ({ useI18n: () => ({ t: (key: string) => key }) }))
vi.mock('../../src/stores/account', () => ({
  useAccountStore: () => ({ loadAccounts: vi.fn().mockResolvedValue(undefined) }),
}))
vi.mock('../../src/stores/config', () => ({
  useConfigStore: () => ({ loadAll: vi.fn().mockResolvedValue(undefined) }),
}))
vi.mock('../../src/stores/ui', () => ({
  useUiStore: () => ({ language: 'zh-TW', applyAll: vi.fn() }),
}))
vi.mock('element-plus', () => ({
  ElConfigProvider: defineComponent({
    inheritAttrs: false,
    setup:
      (_, { slots }) =>
      () =>
        slots.default?.(),
  }),
  ElMessage: { warning: vi.fn() },
}))
vi.mock('../../src/components/AnnouncementModal.vue', () => ({
  default: defineComponent({ setup: () => () => h('div', { 'data-test': 'announcement-owner' }) }),
}))
vi.mock('../../src/windows/ClassicAccountPicker.vue', () => ({
  default: defineComponent({
    setup: () => () => h('div', { 'data-test': 'classic-picker-owner' }),
  }),
}))

import App from '../../src/App.vue'

describe('main-window overlay ownership', () => {
  beforeEach(() => {
    context.label = 'main'
  })

  it('mounts the announcement and account picker in the main window', async () => {
    const wrapper = mount(App, { global: { stubs: { RouterView: true } } })
    await flushPromises()
    expect(wrapper.find('[data-test="announcement-owner"]').exists()).toBe(true)
    expect(wrapper.find('[data-test="classic-picker-owner"]').exists()).toBe(true)
    wrapper.unmount()
  })

  it('does not create main-window overlays inside a browser toolbar', async () => {
    context.label = 'web-browser-1234'
    const wrapper = mount(App, { global: { stubs: { RouterView: true } } })
    await flushPromises()
    expect(wrapper.find('[data-test="announcement-owner"]').exists()).toBe(false)
    expect(wrapper.find('[data-test="classic-picker-owner"]').exists()).toBe(false)
    wrapper.unmount()
  })
})
