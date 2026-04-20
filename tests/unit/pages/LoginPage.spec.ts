/**
 * P12.1 D1 — login shell render guard.
 *
 * Three concerns this spec locks down:
 *
 * 1. The shell renders the localized brand heading + subline (proves
 *    `loginShell.*` keys flow through and the template binds to them).
 * 2. The shell exposes a `<RouterView />` slot — D2-D8 will rely on
 *    this to mount their respective forms.
 * 3. Locale switches re-render the brand text live (proves the shell
 *    isn't accidentally caching the initial translation).
 */

import { describe, expect, it } from 'vitest'
import { defineComponent, h, nextTick } from 'vue'
import { mount } from '@vue/test-utils'
import { createMemoryHistory, createRouter } from 'vue-router'

import LoginPage from '../../../src/pages/LoginPage.vue'
import { createAppI18n, i18nMessages, setLocale } from '../../../src/i18n'

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

  return {
    router,
    i18n,
    mount: async () => {
      await router.push(initialPath)
      await router.isReady()
      const wrapper = mount(LoginPage, {
        global: { plugins: [router, i18n] },
      })
      return wrapper
    },
  }
}

describe('LoginPage shell', () => {
  it('renders the localized brand heading + subline', async () => {
    const ctx = mountLoginPage()
    const wrapper = await ctx.mount()

    expect(wrapper.find('.login-shell__title').text()).toBe(
      i18nMessages['zh-TW'].loginShell.heading,
    )
    expect(wrapper.find('.login-shell__subline').text()).toBe(
      i18nMessages['zh-TW'].loginShell.subline,
    )
  })

  it('exposes a <RouterView /> slot for child routes', async () => {
    const ctx = mountLoginPage('/login/_test')
    const wrapper = await ctx.mount()

    expect(wrapper.find('[data-testid="child"]').exists()).toBe(true)
    expect(wrapper.find('[data-testid="child"]').text()).toBe('child route content')
  })

  it('re-renders brand text after a runtime locale switch', async () => {
    const ctx = mountLoginPage()
    const wrapper = await ctx.mount()

    expect(wrapper.find('.login-shell__title').text()).toBe(
      i18nMessages['zh-TW'].loginShell.heading,
    )

    setLocale(ctx.i18n, 'en-US')
    await nextTick()

    expect(wrapper.find('.login-shell__title').text()).toBe(
      i18nMessages['en-US'].loginShell.heading,
    )
    expect(wrapper.find('.login-shell__subline').text()).toBe(
      i18nMessages['en-US'].loginShell.subline,
    )
  })
})
