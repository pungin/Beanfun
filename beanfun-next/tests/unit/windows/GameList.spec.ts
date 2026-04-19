/**
 * P12.3 D5 — GameList dialog behaviour.
 *
 * Locks down WPF parity for the game-picker modal
 * (`Beanfun/Windows/GameList.xaml(.cs)` port to
 * `windows/GameList.vue`):
 *
 * 1. Visible `false → true` transition triggers
 *    `commands.listGames()` once via `useGameStore.loadGames()` —
 *    mirrors WPF's "open dialog after `reLoadGameInfo()` ran";
 *    we lift the fetch inside the dialog so the SPA can show its
 *    own load state instead of an empty `WrapPanel`.
 * 2. Loading state renders the loading placeholder; no grid /
 *    error / empty branches mounted concurrently.
 * 3. Error state renders the inline banner with the retry button;
 *    Retry click force-refreshes (`loadGames(true)` → second
 *    `commands.listGames` call) — mirrors `AccountList.vue`'s
 *    load-failure banner pattern.
 * 4. Loaded + zero services renders the empty-state placeholder
 *    (the legitimate "no games" branch — distinct from `error`,
 *    surfaces via `loadState === 'loaded'` + `services.length === 0`).
 * 5. Loaded + populated catalogue renders one card per service
 *    in catalogue order (preserving server ordering verbatim,
 *    matching `parse_service_list`'s contract on the backend).
 * 6. Click on a *different* card → `useGameStore.selectGame()`
 *    writes the joined gameCode, the `select` event fires with
 *    `(service_code, service_region)` for the parent (P12.3 D8
 *    `AccountList.vue`), and the dialog closes
 *    (`update:visible(false)`). Mirrors WPF's
 *    `l_GameList_SelectionChanged` mutation + `Close()` branch.
 * 7. Click on the **already-selected** card → no `select` event
 *    fires (mirrors WPF's `if (service_code != … || service_region
 *    != …)` early-exit) but the dialog still closes (WPF
 *    `this.Close()` runs unconditionally).
 *
 * # Stub design
 *
 * Element Plus stubs follow the same shape as
 * `AccRecovery.spec.ts` / `Contract.spec.ts`:
 *
 * - `ElDialog` conditionally renders on `modelValue`, so a
 *   close-event-driven assertion can read
 *   `wrapper.find('[data-test="game-list-dialog"]').exists()`
 *   directly to confirm the dismissal happened.
 * - `ElButton` forwards `disabled` to the inner `<button>` so
 *   the retry-disabled-while-loading invariant is observable
 *   from the DOM.
 * - `ElIcon` is a passthrough span — we don't assert against
 *   its inner SVG.
 *
 * # Why we drive the store directly (not `commands.listGames`
 *   via `safeInvoke`)
 *
 * The dialog is a thin renderer over `useGameStore`; the
 * load-lifecycle invariants live in `tests/unit/stores/game.spec.ts`.
 * We mock `commands.listGames` only to fake the IPC return value
 * and assert call counts (the contract the dialog cares about: did
 * a fresh open kick off exactly one fetch?). The store's
 * 4-state machine + idempotency / force-reload semantics are
 * locked down in the store spec, so the dialog spec can stay
 * focused on rendering + click routing.
 */

import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { flushPromises, mount } from '@vue/test-utils'
import { defineComponent, h, ref, type Component } from 'vue'

import type { CommandError, GameInfoBundle, GameService, Result } from '../../../src/types/bindings'

vi.mock('element-plus', async () => {
  const { defineComponent: dc, h: hh } = await import('vue')

  const ElDialog = dc({
    name: 'ElDialogStub',
    props: { modelValue: { type: Boolean, default: false } },
    emits: ['update:modelValue', 'closed'],
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
    props: { disabled: { type: Boolean, default: false } },
    emits: ['click'],
    setup(props, { slots, emit, attrs }) {
      return () =>
        hh(
          'button',
          {
            ...attrs,
            class: 'el-button-stub',
            disabled: props.disabled || undefined,
            onClick: (e: MouseEvent) => {
              if (props.disabled) return
              emit('click', e)
            },
          },
          slots.default?.(),
        )
    },
  })

  const ElIcon = dc({
    name: 'ElIconStub',
    setup(_, { slots, attrs }) {
      return () => hh('span', { ...attrs, class: 'el-icon-stub' }, slots.default?.())
    },
  })

  return { ElDialog, ElButton, ElIcon, ElMessage: { error: vi.fn() } }
})

vi.mock('@element-plus/icons-vue', () => {
  const stub = (name: string): Component => defineComponent({ name, render: () => h('svg') })
  return {
    CircleClose: stub('CircleCloseStub'),
    Refresh: stub('RefreshStub'),
    VideoPlay: stub('VideoPlayStub'),
    Warning: stub('WarningStub'),
  }
})

vi.mock('../../../src/types/bindings', () => ({
  commands: {
    listGames: vi.fn(),
  },
}))

import { commands } from '../../../src/types/bindings'
import GameList from '../../../src/windows/GameList.vue'
import { useGameStore } from '../../../src/stores/game'
import { createAppI18n, i18nMessages } from '../../../src/i18n'

const ok = <T>(data: T): Promise<Result<T, CommandError>> => Promise.resolve({ status: 'ok', data })

const err = (error: CommandError): Promise<Result<never, CommandError>> =>
  Promise.resolve({ status: 'error', error })

const MAPLE: GameService = {
  name: 'MapleStory',
  service_code: '610074',
  service_region: 'T9',
  website_url: 'https://maplestory.beanfun.com/',
  xlarge_image_name: '610074.jpg',
  large_image_name: '610074_large.jpg',
  small_image_name: '610074_small.jpg',
}

const KART: GameService = {
  name: 'KartRider',
  service_code: '610075',
  service_region: 'T9',
  website_url: 'https://kart.beanfun.com/',
  xlarge_image_name: '610075.jpg',
  large_image_name: '610075_large.jpg',
  small_image_name: '610075_small.jpg',
}

const BUNDLE: GameInfoBundle = {
  ini: {} as Record<string, never>,
  services: [MAPLE, KART],
}

const EMPTY_BUNDLE: GameInfoBundle = {
  ini: {} as Record<string, never>,
  services: [],
}

const TRANSPORT_ERROR: CommandError = {
  code: 'beanfun.transport',
  message: 'connection lost',
  details: null,
}

/**
 * Wrap the dialog in a host that owns the `visible` ref so tests
 * can drive `v-model:visible` from the outside (mirrors the same
 * pattern `Contract.spec.ts` / `AccRecovery.spec.ts` use).
 */
function buildHarness(initialVisible = true) {
  const visibleRef = ref(initialVisible)
  const Host = defineComponent({
    name: 'GameListHost',
    components: { GameList },
    setup() {
      return { visible: visibleRef }
    },
    template: `<GameList v-model:visible="visible" region="TW" />`,
  })
  return { visibleRef, Host }
}

describe('GameList dialog', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    vi.spyOn(console, 'error').mockImplementation(() => {})
    vi.spyOn(console, 'warn').mockImplementation(() => {})
    vi.mocked(commands.listGames).mockReset()
  })

  it('triggers listGames once on first open (false→true) and renders the loading state', async () => {
    /*
     * Hold the IPC promise pending so the spec can observe the
     * loading branch *before* the mock resolves. Without this we
     * race the awaited `flushPromises` and only ever see the
     * `loaded` branch.
     */
    let resolveBundle!: (value: Result<GameInfoBundle, CommandError>) => void
    vi.mocked(commands.listGames).mockReturnValueOnce(
      new Promise<Result<GameInfoBundle, CommandError>>((res) => {
        resolveBundle = res
      }),
    )

    const { Host } = buildHarness(true)
    const wrapper = mount(Host, { global: { plugins: [createAppI18n()] } })
    await flushPromises()

    expect(commands.listGames).toHaveBeenCalledTimes(1)
    expect(wrapper.find('[data-test="game-list-loading"]').exists()).toBe(true)
    expect(wrapper.find('[data-test="game-list-grid"]').exists()).toBe(false)
    expect(wrapper.find('[data-test="game-list-error"]').exists()).toBe(false)
    expect(wrapper.find('[data-test="game-list-empty"]').exists()).toBe(false)

    resolveBundle({ status: 'ok', data: BUNDLE })
    await flushPromises()
  })

  it('renders the error banner with retry button on listGames failure; retry force-reloads', async () => {
    vi.mocked(commands.listGames)
      .mockReturnValueOnce(err(TRANSPORT_ERROR))
      .mockReturnValueOnce(ok(BUNDLE))

    const { Host } = buildHarness(true)
    const wrapper = mount(Host, { global: { plugins: [createAppI18n()] } })
    await flushPromises()

    expect(wrapper.get('[data-test="game-list-error"]').exists()).toBe(true)
    /*
     * Backend message bubbles through `loadError` (see store
     * docblock for the "two surface" rationale: inline banner +
     * toast). Asserting the literal string keeps the dialog
     * honest about *which* failure surface it picks.
     */
    expect(wrapper.get('[data-test="game-list-error"]').text()).toContain(TRANSPORT_ERROR.message)

    await wrapper.get('[data-test="game-list-retry"]').trigger('click')
    await flushPromises()

    expect(commands.listGames).toHaveBeenCalledTimes(2)
    expect(wrapper.find('[data-test="game-list-grid"]').exists()).toBe(true)
    expect(wrapper.find('[data-test="game-list-error"]').exists()).toBe(false)
  })

  it('renders the empty-state placeholder when the catalogue is loaded but contains zero services', async () => {
    vi.mocked(commands.listGames).mockReturnValueOnce(ok(EMPTY_BUNDLE))

    const { Host } = buildHarness(true)
    const wrapper = mount(Host, { global: { plugins: [createAppI18n()] } })
    await flushPromises()

    expect(wrapper.get('[data-test="game-list-empty"]').text()).toBe(
      i18nMessages['zh-TW'].gameList.empty,
    )
    expect(wrapper.find('[data-test="game-list-grid"]').exists()).toBe(false)
    expect(wrapper.find('[data-test="game-list-error"]').exists()).toBe(false)
  })

  it('renders one card per service in catalogue order with name + image src derived from region', async () => {
    vi.mocked(commands.listGames).mockReturnValueOnce(ok(BUNDLE))

    const { Host } = buildHarness(true)
    const wrapper = mount(Host, { global: { plugins: [createAppI18n()] } })
    await flushPromises()

    const items = wrapper.findAll('[data-test^="game-list-item-"]')
    expect(items).toHaveLength(2)

    expect(items[0].attributes('data-test')).toBe('game-list-item-610074_T9')
    expect(items[1].attributes('data-test')).toBe('game-list-item-610075_T9')

    expect(wrapper.get('[data-test="game-list-name-610074_T9"]').text()).toBe(MAPLE.name)
    expect(wrapper.get('[data-test="game-list-name-610075_T9"]').text()).toBe(KART.name)

    /*
     * Region prop = 'TW' → frontend `imageUrl` returns the TW
     * CDN base. WPF parity is locked down separately in
     * `tests/unit/stores/game.spec.ts`; here we only assert the
     * dialog actually wires the region prop through to the
     * helper (no hard-coded base in the template).
     */
    const img = wrapper.get('[data-test="game-list-image-610074_T9"]').attributes('src') ?? ''
    expect(img).toContain('tw.images.beanfun.com')
    expect(img.endsWith(MAPLE.large_image_name)).toBe(true)
  })

  it('clicking a card emits select(code, region), writes selectedGameCode, and closes the dialog', async () => {
    vi.mocked(commands.listGames).mockReturnValueOnce(ok(BUNDLE))

    const { visibleRef, Host } = buildHarness(true)
    const wrapper = mount(Host, { global: { plugins: [createAppI18n()] } })
    await flushPromises()

    await wrapper.get('[data-test="game-list-item-610074_T9"]').trigger('click')
    await flushPromises()

    const game = useGameStore()
    expect(game.selectedGameCode).toBe('610074_T9')

    const emits = wrapper.findComponent(GameList).emitted()
    expect(emits.select).toEqual([['610074', 'T9']])

    expect(visibleRef.value).toBe(false)
    /*
     * Dialog stub conditionally renders on `modelValue`, so the
     * dismissal cascading through the v-model proxy unmounts the
     * dialog body. Asserting the unmount proves the close path
     * round-tripped end-to-end (component → emit → host ref).
     */
    expect(wrapper.find('[data-test="game-list-dialog"]').exists()).toBe(false)
  })

  it('clicking the already-selected card closes without re-emitting select (WPF early-exit parity)', async () => {
    vi.mocked(commands.listGames).mockReturnValueOnce(ok(BUNDLE))

    const { visibleRef, Host } = buildHarness(true)
    const wrapper = mount(Host, { global: { plugins: [createAppI18n()] } })
    await flushPromises()

    /*
     * Pre-select the card the test will click. The store
     * mutation is the same one the dialog itself would have
     * performed on the first click — we short-circuit the setup
     * by writing it directly so we can isolate the
     * "re-click on the same card" branch in this case.
     */
    const game = useGameStore()
    game.selectGame(MAPLE.service_code, MAPLE.service_region)
    await flushPromises()

    await wrapper.get('[data-test="game-list-item-610074_T9"]').trigger('click')
    await flushPromises()

    const emits = wrapper.findComponent(GameList).emitted()
    expect(emits.select).toBeUndefined()
    expect(visibleRef.value).toBe(false)
    expect(wrapper.find('[data-test="game-list-dialog"]').exists()).toBe(false)
  })

  it('header close button emits update:visible(false) without firing select', async () => {
    vi.mocked(commands.listGames).mockReturnValueOnce(ok(BUNDLE))

    const { visibleRef, Host } = buildHarness(true)
    const wrapper = mount(Host, { global: { plugins: [createAppI18n()] } })
    await flushPromises()

    await wrapper.get('[data-test="game-list-close"]').trigger('click')
    await flushPromises()

    const emits = wrapper.findComponent(GameList).emitted()
    expect(emits.select).toBeUndefined()
    expect(visibleRef.value).toBe(false)
    expect(wrapper.find('[data-test="game-list-dialog"]').exists()).toBe(false)
  })
})
