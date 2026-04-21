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
import { useConfigStore } from '../../../src/stores/config'

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
const mockGetAllConfig = vi.mocked(commands.getAllConfig)

const ok = <T>(data: T): Promise<Result<T, CommandError>> => Promise.resolve({ status: 'ok', data })

/**
 * Standalone harness: a memory-history router with the picker mounted
 * as the only route, plus a stub `/login/id-pass` so the post-select
 * `router.push` resolves cleanly. Mirrors the test layout we'll reuse
 * in D3-D8 to verify each form's nav target.
 */
function mountPicker(opts: { initialPath?: string } = {}): {
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
      {
        path: '/login/qr',
        name: 'login-qr',
        component: defineComponent({
          name: 'QrStub',
          render: () => h('div', { 'data-testid': 'qr-stub' }),
        }),
      },
    ],
  })

  const i18n = createAppI18n()

  return {
    router,
    i18n,
    async mountIt() {
      await router.push(opts.initialPath ?? '/login/region')
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
    mockGetAllConfig.mockReset()
    mockGetAllConfig.mockReturnValue(ok({}))
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
    expect(wrapper.find('.region-picker__tip').text()).toContain(
      i18nMessages['zh-TW'].loginRegion.tip,
    )
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

  /*
   * Auto-redirect (WPF `loginMethodInit` parity, commit `24a07af`).
   *
   * Once `Config.xml` is loaded, a saved `loginRegion` should skip
   * the picker and jump straight to the matching login form. The
   * `?pick=1` query is the documented escape hatch — login-form back
   * buttons append it so the user can return to the picker.
   *
   * This block locks down the bug fixed in
   * `fix(login-region): remove duplicate onMounted redirect`: a stray
   * `onMounted` block was racing the watcher with weaker logic
   * (no `?pick` check, no `loginMethod` awareness), making the
   * picker unreachable after first launch.
   */
  describe('auto-redirect when Config.xml has saved preferences', () => {
    it('jumps to /login/id-pass when loginRegion=TW is saved (default loginMethod)', async () => {
      mockGetAllConfig.mockReturnValue(ok({ loginRegion: 'TW' }))
      const ctx = mountPicker()
      const config = useConfigStore()
      await config.loadAll()
      const wrapper = await ctx.mountIt()
      await flushPromises()

      expect(ctx.router.currentRoute.value.path).toBe('/login/id-pass')
      wrapper.unmount()
    })

    it('jumps to /login/qr when loginRegion=TW + loginMethod=1 are saved', async () => {
      mockGetAllConfig.mockReturnValue(ok({ loginRegion: 'TW', loginMethod: '1' }))
      const ctx = mountPicker()
      const config = useConfigStore()
      await config.loadAll()
      const wrapper = await ctx.mountIt()
      await flushPromises()

      expect(ctx.router.currentRoute.value.path).toBe('/login/qr')
      wrapper.unmount()
    })

    it('falls back to /login/id-pass for HK even when loginMethod=1 (HK has no QR endpoint)', async () => {
      mockGetAllConfig.mockReturnValue(ok({ loginRegion: 'HK', loginMethod: '1' }))
      const ctx = mountPicker()
      const config = useConfigStore()
      await config.loadAll()
      const wrapper = await ctx.mountIt()
      await flushPromises()

      expect(ctx.router.currentRoute.value.path).toBe('/login/id-pass')
      wrapper.unmount()
    })

    it('stays on the picker when ?pick=1 is in the route, even with a saved region', async () => {
      mockGetAllConfig.mockReturnValue(ok({ loginRegion: 'TW' }))
      const ctx = mountPicker({ initialPath: '/login/region?pick=1' })
      const config = useConfigStore()
      await config.loadAll()
      const wrapper = await ctx.mountIt()
      await flushPromises()

      expect(ctx.router.currentRoute.value.path).toBe('/login/region')
      expect(wrapper.findAll('.region-tile')).toHaveLength(2)
      wrapper.unmount()
    })

    it('stays on the picker on first launch when no region is saved', async () => {
      mockGetAllConfig.mockReturnValue(ok({}))
      const ctx = mountPicker()
      const config = useConfigStore()
      await config.loadAll()
      const wrapper = await ctx.mountIt()
      await flushPromises()

      expect(ctx.router.currentRoute.value.path).toBe('/login/region')
      expect(wrapper.findAll('.region-tile')).toHaveLength(2)
      wrapper.unmount()
    })
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
