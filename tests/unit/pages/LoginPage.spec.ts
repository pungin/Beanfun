/**
 * P12.1 D1 — login shell render guard.
 *
 * Two concerns this spec locks down:
 *
 * 1. The shell exposes a `<RouterView />` slot — D2-D8 will rely on
 *    this to mount their respective forms.
 * 2. The shell renders a TitleBar component for window chrome.
 */

import { describe, expect, it } from 'vitest'
import { defineComponent, h } from 'vue'
import { mount } from '@vue/test-utils'
import { createMemoryHistory, createRouter } from 'vue-router'
import { createPinia } from 'pinia'

import LoginPage from '../../../src/pages/LoginPage.vue'
import { createAppI18n } from '../../../src/i18n'
import { setActivePinia } from 'pinia'
import { useUiStore } from '../../../src/stores/ui'
import { useConfigStore } from '../../../src/stores/config'

const ChildStub = defineComponent({
  name: 'ChildStub',
  render: () => h('span', { 'data-testid': 'child' }, 'child route content'),
})

function mountLoginPage(initialPath = '/login/_test') {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: [
      {
        path: '/login',
        component: LoginPage,
        children: [{ path: '_test', component: ChildStub }],
      },
    ],
  })

  const i18n = createAppI18n()
  const pinia = createPinia()
  // Make the harness pinia the active one so specs can reach the same
  // stores the mounted component uses.
  setActivePinia(pinia)

  return {
    router,
    i18n,
    mount: async () => {
      await router.push(initialPath)
      await router.isReady()
      const wrapper = mount(LoginPage, {
        global: { plugins: [router, i18n, pinia] },
      })
      return wrapper
    },
  }
}

describe('LoginPage shell', () => {
  it('renders the TitleBar component', async () => {
    const ctx = mountLoginPage()
    const wrapper = await ctx.mount()

    expect(wrapper.find('.bf-titlebar').exists()).toBe(true)
  })

  it('does not expose raw Material Symbols ligature names in the title bar', async () => {
    const ctx = mountLoginPage()
    const wrapper = await ctx.mount()

    const titleBarText = wrapper.get('.bf-titlebar').text()
    expect(titleBarText).not.toContain('settings')
    expect(titleBarText).not.toContain('info')
    expect(titleBarText).not.toContain('minimize')
    expect(titleBarText).not.toContain('close')
  })

  it('classic mode toggle renders and flips classicLoginMode', async () => {
    const ctx = mountLoginPage()
    const wrapper = await ctx.mount()

    const toggle = wrapper.get('[data-test="login-classic-mode"]')
    expect(toggle.classes()).not.toContain('login-shell__classic-btn--active')

    // setConfig IPC is unmocked in this spec's slim harness — seed the
    // cache directly and assert the active class follows the store.
    useConfigStore().entries['classicLoginMode'] = 'true'
    await wrapper.vm.$nextTick()
    expect(useUiStore().classicLoginMode).toBe(true)
    expect(wrapper.get('[data-test="login-classic-mode"]').classes()).toContain(
      'login-shell__classic-btn--active',
    )
  })

  it('exposes a <RouterView /> slot for child routes', async () => {
    const ctx = mountLoginPage('/login/_test')
    const wrapper = await ctx.mount()

    expect(wrapper.find('[data-testid="child"]').exists()).toBe(true)
    expect(wrapper.find('[data-testid="child"]').text()).toBe('child route content')
  })
})
