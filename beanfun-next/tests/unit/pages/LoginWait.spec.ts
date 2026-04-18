/**
 * P12.1 D7 — "logging in…" wait page behaviour.
 *
 * WPF `Beanfun/Pages/LoginWait.xaml(.cs)` is a bare spinner/label/
 * cancel-button scaffold. The SPA port keeps the same user-visible
 * contract and defers worker-abort / app-auth-polling teardown to
 * later D-steps that actually own those concerns:
 *
 * 1. Renders the `MsgLogging` message (WPF shared locale key) and
 *    the shared `Cancel` button.
 * 2. Renders a visible spinner element tagged `role="status"` so
 *    screen-readers announce the loading state (WPF parity is purely
 *    visual; the a11y role is an SPA improvement).
 * 3. Cancel button navigates to `/login/id-pass` — WPF
 *    `return_page = loginPage` parity.
 * 4. Labels re-render on runtime locale switch — proves the page
 *    is routed through `createAppI18n()` like the other login
 *    children.
 */

import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { flushPromises, mount } from '@vue/test-utils'
import { createMemoryHistory, createRouter } from 'vue-router'
import { defineComponent, h } from 'vue'

/*
 * Stubs mirror `LoginTotp.spec.ts` shape — just the subset that
 * LoginWait actually pulls in (ElButton). No icon imports on this
 * page, so `@element-plus/icons-vue` is left out of the stub table.
 */
vi.mock('element-plus', () => ({
  ElButton: defineComponent({
    name: 'ElButtonStub',
    props: {
      loading: { type: Boolean, default: false },
      size: { type: String, default: '' },
    },
    emits: ['click'],
    setup(props, { slots, emit, attrs }) {
      return () =>
        h(
          'button',
          {
            ...attrs,
            class: 'el-button-stub',
            disabled: props.loading,
            onClick: (e: MouseEvent) => emit('click', e),
          },
          slots.default?.(),
        )
    },
  }),
  ElMessage: { error: vi.fn(), success: vi.fn(), warning: vi.fn() },
}))

/*
 * The Tauri bindings mock is kept around even though LoginWait
 * itself does not call into IPC — Pinia's auth store is still
 * instantiated via `createPinia()` in `beforeEach` and the store's
 * imports resolve through this mock module. Removing it would fail
 * the store import at mount time.
 */
vi.mock('../../../src/types/bindings', () => ({
  commands: {
    getAllConfig: vi.fn(),
    setConfig: vi.fn(),
    getConfigValue: vi.fn(),
    loginRegular: vi.fn(),
    loginTotp: vi.fn(),
    loginQrStart: vi.fn(),
    loginQrCheck: vi.fn(),
    loginGamepassStart: vi.fn(),
    openGamepassWindow: vi.fn(),
    getVerifyPageInfo: vi.fn(),
    getVerifyCaptcha: vi.fn(),
    submitVerify: vi.fn(),
    logout: vi.fn(),
  },
}))

import LoginWait from '../../../src/pages/LoginWait.vue'
import { createAppI18n, i18nMessages, setLocale } from '../../../src/i18n'

/**
 * Memory-router harness — only needs the wait page plus the Cancel
 * destination (`/login/id-pass`).
 */
function mountForm() {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: [
      { path: '/login/wait', name: 'login-wait', component: LoginWait },
      {
        path: '/login/id-pass',
        name: 'login-id-pass',
        component: defineComponent({ name: 'IdPassStub', render: () => h('div') }),
      },
    ],
  })

  const i18n = createAppI18n()
  return {
    router,
    i18n,
    async mountIt() {
      await router.push('/login/wait')
      await router.isReady()
      return mount(LoginWait, { global: { plugins: [router, i18n] } })
    },
  }
}

describe('LoginWait', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
  })

  it('renders the MsgLogging message and the Cancel button', async () => {
    const ctx = mountForm()
    const wrapper = await ctx.mountIt()

    expect(wrapper.find('[data-test="wait-message"]').text()).toBe(i18nMessages['zh-TW'].MsgLogging)
    expect(wrapper.find('[data-test="wait-cancel"]').text()).toBe(i18nMessages['zh-TW'].Cancel)
  })

  it('renders a spinner element with role="status" for a11y', async () => {
    const ctx = mountForm()
    const wrapper = await ctx.mountIt()

    const spinner = wrapper.find('[data-test="wait-spinner"]')
    expect(spinner.exists()).toBe(true)
    expect(spinner.attributes('role')).toBe('status')
  })

  it('Cancel button navigates to /login/id-pass (WPF return_page parity)', async () => {
    const ctx = mountForm()
    const wrapper = await ctx.mountIt()

    await wrapper.get('[data-test="wait-cancel"]').trigger('click')
    await flushPromises()

    expect(ctx.router.currentRoute.value.path).toBe('/login/id-pass')
  })

  it('re-renders labels after a runtime locale switch', async () => {
    const ctx = mountForm()
    const wrapper = await ctx.mountIt()

    expect(wrapper.find('[data-test="wait-message"]').text()).toBe(i18nMessages['zh-TW'].MsgLogging)

    setLocale(ctx.i18n, 'en-US')
    await flushPromises()

    expect(wrapper.find('[data-test="wait-message"]').text()).toBe(i18nMessages['en-US'].MsgLogging)
    expect(wrapper.find('[data-test="wait-cancel"]').text()).toBe(i18nMessages['en-US'].Cancel)
  })
})
