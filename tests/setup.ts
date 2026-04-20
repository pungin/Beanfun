/**
 * Global test setup — mocks for Tauri APIs that are unavailable
 * in the jsdom test environment.
 */

import { vi } from 'vitest'
import { config } from '@vue/test-utils'
import { defineComponent, h } from 'vue'

/* Mock @tauri-apps/api/window used by TitleBar.vue */
vi.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: () => ({
    minimize: vi.fn(),
    close: vi.fn(),
    startDragging: vi.fn(),
    setSize: vi.fn(),
  }),
}))

vi.mock('@tauri-apps/api/dpi', () => ({
  LogicalSize: class LogicalSize {
    width: number
    height: number
    constructor(w: number, h: number) {
      this.width = w
      this.height = h
    }
  },
}))

/*
 * Globally stub TitleBar so page-level tests that mock
 * element-plus / vue-router don't break on TitleBar's imports.
 */
config.global.stubs = {
  ...config.global.stubs,
  TitleBar: defineComponent({
    name: 'TitleBarStub',
    setup:
      (_, { slots }) =>
      () =>
        h('div', { class: 'bf-titlebar' }, slots.default?.()),
  }),
}
