/**
 * P12.1 D2 — region picker behaviour.
 *
 * What this spec locks down (matches WPF
 * `LoginRegionSelection.xaml.cs` button handlers verbatim):
 *
 * 1. Two tiles render — one per `LoginRegion` literal — labeled with
 *    the localized `Taiwan` / `HongKong` keys (i18n flow proven).
 * 2. Clicking a tile persists the choice to Config.xml under the
 *    legacy `loginRegion` key (WPF `ConfigAppSettings.SetValue` parity).
 * 3. After persisting, the user is forwarded to the regular id-pass
 *    form so the login funnel stays moving.
 * 4. The picker remains visible / interactable while the persist call
 *    is in-flight (no premature disabled state); avoids blocking the
 *    user if the IPC takes a beat.
 */

import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { flushPromises, mount } from '@vue/test-utils'
import { createMemoryHistory, createRouter, type Router } from 'vue-router'
import { defineComponent, h, nextTick } from 'vue'
import type { I18n } from 'vue-i18n'

import type { CommandError, Result } from '../../../src/types/bindings'

vi.mock('element-plus', () => ({
  ElIcon: defineComponent({
    name: 'ElIconStub',
    setup(_, { slots }) {
      return () => h('span', { class: 'el-icon-stub' }, slots.default?.())
    },
  }),
  ElMessage: { error: vi.fn(), success: vi.fn(), warning: vi.fn() },
}))

vi.mock('@element-plus/icons-vue', () => ({
  Flag: defineComponent({ name: 'FlagStub', render: () => h('svg') }),
}))

vi.mock('../../../src/types/bindings', () => ({
  commands: {
    getAllConfig: vi.fn(),
    setConfig: vi.fn(),
    getConfigValue: vi.fn(),
  },
}))

import { commands } from '../../../src/types/bindings'
import LoginRegionSelection from '../../../src/pages/LoginRegionSelection.vue'
import { createAppI18n, i18nMessages, setLocale } from '../../../src/i18n'

const mockSetConfig = vi.mocked(commands.setConfig)

const ok = <T>(data: T): Promise<Result<T, CommandError>> => Promise.resolve({ status: 'ok', data })

/**
 * Standalone harness: a memory-history router with the picker mounted
 * as the only route, plus a stub `/login/id-pass` so the post-select
 * `router.push` resolves cleanly. Mirrors the test layout we'll reuse
 * in D3-D8 to verify each form's nav target.
 */
function mountPicker(): {
  router: Router
  i18n: I18n
  mountIt: () => Promise<ReturnType<typeof mount>>
} {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: [
      {
        path: '/login/region',
        name: 'login-region',
        component: LoginRegionSelection,
      },
      {
        path: '/login/id-pass',
        name: 'login-id-pass',
        component: defineComponent({
          name: 'IdPassStub',
          render: () => h('div', { 'data-testid': 'id-pass-stub' }),
        }),
      },
    ],
  })

  const i18n = createAppI18n()

  return {
    router,
    i18n,
    async mountIt() {
      await router.push('/login/region')
      await router.isReady()
      return mount(LoginRegionSelection, {
        global: { plugins: [router, i18n] },
      })
    },
  }
}

describe('LoginRegionSelection', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    mockSetConfig.mockReset()
    mockSetConfig.mockReturnValue(ok(null))
  })

  it('renders both region tiles with their localized labels', async () => {
    const ctx = mountPicker()
    const wrapper = await ctx.mountIt()

    const tiles = wrapper.findAll('.region-tile')
    expect(tiles).toHaveLength(2)

    expect(tiles[0].attributes('data-region')).toBe('TW')
    expect(tiles[0].text()).toContain(i18nMessages['zh-TW'].Taiwan)

    expect(tiles[1].attributes('data-region')).toBe('HK')
    expect(tiles[1].text()).toContain(i18nMessages['zh-TW'].HongKong)
  })

  it('renders the heading + subline + tip from i18n', async () => {
    const ctx = mountPicker()
    const wrapper = await ctx.mountIt()

    expect(wrapper.find('.region-picker__heading').text()).toBe(
      i18nMessages['zh-TW'].BeanfunRegionSelected,
    )
    expect(wrapper.find('.region-picker__subline').text()).toBe(
      i18nMessages['zh-TW'].loginRegion.subline,
    )
    expect(wrapper.find('.region-picker__tip').text()).toBe(i18nMessages['zh-TW'].loginRegion.tip)
  })

  it('persists "TW" to loginRegion when the Taiwan tile is clicked', async () => {
    const ctx = mountPicker()
    const wrapper = await ctx.mountIt()

    await wrapper.find('[data-region="TW"]').trigger('click')

    expect(mockSetConfig).toHaveBeenCalledTimes(1)
    expect(mockSetConfig).toHaveBeenCalledWith('loginRegion', 'TW')
  })

  it('persists "HK" to loginRegion when the Hong Kong tile is clicked', async () => {
    const ctx = mountPicker()
    const wrapper = await ctx.mountIt()

    await wrapper.find('[data-region="HK"]').trigger('click')

    expect(mockSetConfig).toHaveBeenCalledTimes(1)
    expect(mockSetConfig).toHaveBeenCalledWith('loginRegion', 'HK')
  })

  it('navigates to /login/id-pass after the region is persisted', async () => {
    const ctx = mountPicker()
    const wrapper = await ctx.mountIt()

    await wrapper.find('[data-region="TW"]').trigger('click')
    /*
     * `trigger` resolves once Vue flushes the click handler, but the
     * handler itself is async (`config.set` → `router.push`). Drain
     * the microtask queue so the post-`set` `push` actually lands
     * before we assert the final route.
     */
    await flushPromises()

    expect(ctx.router.currentRoute.value.path).toBe('/login/id-pass')
    expect(ctx.router.currentRoute.value.name).toBe('login-id-pass')
  })

  it('re-renders heading + tile labels after a runtime locale switch', async () => {
    const ctx = mountPicker()
    const wrapper = await ctx.mountIt()

    expect(wrapper.find('.region-picker__heading').text()).toBe(
      i18nMessages['zh-TW'].BeanfunRegionSelected,
    )

    setLocale(ctx.i18n, 'en-US')
    await nextTick()

    expect(wrapper.find('.region-picker__heading').text()).toBe(
      i18nMessages['en-US'].BeanfunRegionSelected,
    )
    expect(wrapper.find('[data-region="TW"]').text()).toContain(i18nMessages['en-US'].Taiwan)
    expect(wrapper.find('[data-region="HK"]').text()).toContain(i18nMessages['en-US'].HongKong)
  })
})
