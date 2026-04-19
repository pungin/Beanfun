/**
 * P12.5 D7 — `ToolsDialogStack` wrapper component behaviour.
 *
 * The wrapper is the SPA-side mirror of WPF's
 * `AccountList.xaml.cs::btn_Tools_Click` (L237-250) dispatch
 * switch + the per-game Tools window mounting. It owns five
 * sibling dialogs (MapleTools / KartTools / WebBrowser /
 * EquipCalculator / CoreCalculator) and exposes one imperative
 * `openForGame(gameCode)` entry point both AccountList and
 * Settings call from their Tools button click handlers.
 *
 * What this spec locks down:
 *
 * 1. `openForGame` dispatch — the three WPF-whitelisted codes
 *    each route to the correct dialog (`610074_T9` /
 *    `610075_T9` → MapleTools; `610096_TE` → KartTools); a
 *    code outside the whitelist is a no-op (matches WPF switch
 *    fallthrough).
 * 2. MapleTools open path resolves `gamePath` via
 *    `commands.detectGamePath` with the active `selectedIni`'s
 *    `dir_value_name` / `dir_reg`, and snapshots
 *    `auth.session?.region` for the WPF HK PlayerReport advisory.
 * 3. KartTools open path does NOT call `detectGamePath` (KartTools
 *    needs no game path).
 * 4. `gamePath` defaults to empty string when:
 *    - `selectedIni` is missing (no active game).
 *    - `selectedIni.dir_value_name === ''` (no Config key shape).
 *    - The backend `detectGamePath` command fails.
 *    Each empty-string path matches WPF's "open the dialog
 *    regardless; surface the error on Recycling click" pattern.
 * 5. Child dialog event chain:
 *    - MapleTools `open-web-browser` → WebBrowser visible + URL
 *      threaded through.
 *    - MapleTools `open-equip-calculator` → EquipCalculator visible.
 *    - MapleTools `open-core-calculator` → CoreCalculator visible.
 *    - KartTools `open-web-browser` → WebBrowser visible + URL.
 *
 * The internal behaviour of MapleTools / KartTools / WebBrowser /
 * EquipCalculator / CoreCalculator is intentionally NOT asserted
 * here — each gets its own test surface in their respective
 * D-step (D2 / D3 / D4 / D5+D6 already written; component-level
 * specs would be a future hardening pass). This spec stays
 * focused on the wrapper's dispatch + state-threading contract.
 *
 * # Stub strategy
 *
 * The five child dialogs are stubbed via `vi.mock` factories that
 * create inline `defineComponent` placeholders. The factories
 * cannot reference top-level constants (Vitest hoists `vi.mock`
 * above all imports — see the `vi.hoisted` docs), so each stub is
 * created locally inside its factory and identified in tests via
 * `wrapper.findComponent({ name: 'MapleTools' })` (etc.). Each
 * stub forwards its props into `data-*` attributes so the spec
 * can read the values the wrapper threaded through without
 * touching component internals.
 */

import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { flushPromises, mount } from '@vue/test-utils'

import type {
  CommandError,
  GameIniEntry,
  GameService,
  Result,
  SessionInfo,
} from '../../../src/types/bindings'

vi.mock('../../../src/types/bindings', () => ({
  commands: {
    detectGamePath: vi.fn(),
  },
}))

/*
 * Stub `MapleTools.vue`. The wrapper binds `visible`, `gamePath`,
 * and `loginRegion` props + listens to `open-web-browser`,
 * `open-equip-calculator`, and `open-core-calculator` events
 * (see `windows/ToolsDialogStack.vue` template). Forward all
 * three props into `data-*` attributes for inspection; declare
 * the four emits so `vm.$emit(...)` from tests round-trips
 * through the parent's listeners.
 */
vi.mock('../../../src/windows/MapleTools.vue', async () => {
  const { defineComponent, h } = await import('vue')
  return {
    default: defineComponent({
      name: 'MapleTools',
      props: {
        visible: { type: Boolean, default: false },
        gamePath: { type: String, default: '' },
        loginRegion: { type: String, default: undefined },
      },
      emits: [
        'update:visible',
        'open-web-browser',
        'open-equip-calculator',
        'open-core-calculator',
      ],
      setup(props) {
        return () =>
          h('div', {
            class: 'maple-tools-stub',
            'data-test': 'maple-tools-stub',
            'data-visible': String(props.visible),
            'data-game-path': props.gamePath,
            'data-login-region': props.loginRegion ?? '',
          })
      },
    }),
  }
})

vi.mock('../../../src/windows/KartTools.vue', async () => {
  const { defineComponent, h } = await import('vue')
  return {
    default: defineComponent({
      name: 'KartTools',
      props: { visible: { type: Boolean, default: false } },
      emits: ['update:visible', 'open-web-browser'],
      setup(props) {
        return () =>
          h('div', {
            class: 'kart-tools-stub',
            'data-test': 'kart-tools-stub',
            'data-visible': String(props.visible),
          })
      },
    }),
  }
})

vi.mock('../../../src/windows/WebBrowser.vue', async () => {
  const { defineComponent, h } = await import('vue')
  return {
    default: defineComponent({
      name: 'WebBrowser',
      props: {
        visible: { type: Boolean, default: false },
        url: { type: String, default: '' },
      },
      emits: ['update:visible'],
      setup(props) {
        return () =>
          h('div', {
            class: 'web-browser-stub',
            'data-test': 'web-browser-stub',
            'data-visible': String(props.visible),
            'data-url': props.url,
          })
      },
    }),
  }
})

vi.mock('../../../src/windows/EquipCalculator.vue', async () => {
  const { defineComponent, h } = await import('vue')
  return {
    default: defineComponent({
      name: 'EquipCalculator',
      props: { visible: { type: Boolean, default: false } },
      emits: ['update:visible'],
      setup(props) {
        return () =>
          h('div', {
            class: 'equip-calc-stub',
            'data-test': 'equip-calc-stub',
            'data-visible': String(props.visible),
          })
      },
    }),
  }
})

vi.mock('../../../src/windows/CoreCalculator.vue', async () => {
  const { defineComponent, h } = await import('vue')
  return {
    default: defineComponent({
      name: 'CoreCalculator',
      props: { visible: { type: Boolean, default: false } },
      emits: ['update:visible'],
      setup(props) {
        return () =>
          h('div', {
            class: 'core-calc-stub',
            'data-test': 'core-calc-stub',
            'data-visible': String(props.visible),
          })
      },
    }),
  }
})

import { commands } from '../../../src/types/bindings'
import ToolsDialogStack from '../../../src/windows/ToolsDialogStack.vue'
import { useAuthStore } from '../../../src/stores/auth'
import { useGameStore } from '../../../src/stores/game'
import { createAppI18n } from '../../../src/i18n'

const ok = <T>(data: T): Promise<Result<T, CommandError>> => Promise.resolve({ status: 'ok', data })
const err = (error: CommandError): Promise<Result<never, CommandError>> =>
  Promise.resolve({ status: 'error', error })

const MAPLE_TW: GameService = {
  name: 'MapleStory TW',
  service_code: '610074',
  service_region: 'T9',
  website_url: 'https://maplestory.beanfun.com',
  xlarge_image_name: '610074_x.jpg',
  large_image_name: '610074_l.jpg',
  small_image_name: '610074_s.jpg',
  download_url: 'https://maplestory.beanfun.com/download',
}

const MAPLE_TW_INI: GameIniEntry = {
  exe: 'C:\\Beanfun\\MapleStory.exe',
  login_action_type: '8',
  win_class_name: 'MapleStoryClass',
  dir_value_name: 'ExecPath',
  dir_reg: 'SOFTWARE\\Gamania\\MapleStory',
}

const KART_TW: GameService = {
  ...MAPLE_TW,
  name: 'KartRider',
  service_code: '610096',
  service_region: 'TE',
}

const KART_TW_INI: GameIniEntry = {
  ...MAPLE_TW_INI,
  exe: 'C:\\Beanfun\\KartRider.exe',
  win_class_name: 'KartRiderClass',
}

const FAKE_TW_SESSION: SessionInfo = {
  region: 'TW',
  account_id: 'alice',
  service_code: '610074',
  service_region: 'T9',
}

const FAKE_HK_SESSION: SessionInfo = {
  ...FAKE_TW_SESSION,
  region: 'HK',
}

/**
 * Type for the wrapper's `defineExpose`d API. Mirrors the
 * `openForGame(gameCode: string): Promise<void>` shape declared
 * in `windows/ToolsDialogStack.vue::defineExpose`.
 */
type ToolsDialogStackApi = { openForGame: (gameCode: string) => Promise<void> }

/**
 * Seed a game into the store so `useGameStore.selectedIni`
 * resolves to the supplied INI for the supplied service. Used
 * to drive the `gamePath` resolution path under test.
 */
function seedGame(service: GameService, ini: GameIniEntry): void {
  const gameStore = useGameStore()
  gameStore.services = [service]
  gameStore.ini = { [`${service.service_code}_${service.service_region}`]: ini }
  gameStore.selectedGameCode = `${service.service_code}_${service.service_region}`
  gameStore.loadState = 'loaded'
}

function mountStack() {
  return mount(ToolsDialogStack, {
    global: { plugins: [createAppI18n()] },
  })
}

describe('ToolsDialogStack wrapper', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    vi.spyOn(console, 'error').mockImplementation(() => {})
    vi.spyOn(console, 'warn').mockImplementation(() => {})
    vi.mocked(commands.detectGamePath).mockReset()
    vi.mocked(commands.detectGamePath).mockReturnValue(ok('C:\\Beanfun\\MapleStory.exe'))
  })

  /* -------------------- openForGame dispatch -------------------- */

  it('routes 610074_T9 (MapleStory TW) to MapleTools (visible=true)', async () => {
    /*
     * WPF `AccountList.xaml.cs` L242-244: case "610074_T9":
     * new MapleTools().Show(). MapleTools must flip visible
     * after `openForGame` resolves; KartTools must stay closed.
     */
    seedGame(MAPLE_TW, MAPLE_TW_INI)
    useAuthStore().session = FAKE_TW_SESSION

    const wrapper = mountStack()
    await flushPromises()

    await (wrapper.vm as unknown as ToolsDialogStackApi).openForGame('610074_T9')
    await flushPromises()

    expect(wrapper.get('[data-test="maple-tools-stub"]').attributes('data-visible')).toBe('true')
    expect(wrapper.get('[data-test="kart-tools-stub"]').attributes('data-visible')).toBe('false')
  })

  it('routes 610075_T9 (MapleStory M) to MapleTools (visible=true)', async () => {
    /* WPF L243 — same MapleTools mount as TW (switch fallthrough). */
    seedGame(
      { ...MAPLE_TW, service_code: '610075' },
      { ...MAPLE_TW_INI, exe: 'C:\\Beanfun\\MapleM.exe' },
    )
    useAuthStore().session = FAKE_TW_SESSION

    const wrapper = mountStack()
    await flushPromises()

    await (wrapper.vm as unknown as ToolsDialogStackApi).openForGame('610075_T9')
    await flushPromises()

    expect(wrapper.get('[data-test="maple-tools-stub"]').attributes('data-visible')).toBe('true')
    expect(wrapper.get('[data-test="kart-tools-stub"]').attributes('data-visible')).toBe('false')
  })

  it('routes 610096_TE (KartRider) to KartTools (visible=true)', async () => {
    /*
     * WPF L246-247: case "610096_TE": new KartTools().Show().
     * KartTools opens; MapleTools stays closed; no `detectGamePath`
     * IPC fires (KartTools needs no game path).
     */
    seedGame(KART_TW, KART_TW_INI)
    useAuthStore().session = FAKE_TW_SESSION

    const wrapper = mountStack()
    await flushPromises()

    await (wrapper.vm as unknown as ToolsDialogStackApi).openForGame('610096_TE')
    await flushPromises()

    expect(wrapper.get('[data-test="kart-tools-stub"]').attributes('data-visible')).toBe('true')
    expect(wrapper.get('[data-test="maple-tools-stub"]').attributes('data-visible')).toBe('false')
    expect(commands.detectGamePath).not.toHaveBeenCalled()
  })

  it('is a no-op for a code outside the whitelist (matches WPF switch fallthrough)', async () => {
    /*
     * WPF L240-249: the switch has no `default:` arm — any code
     * not matching the three cases falls through and the method
     * returns. The SPA wrapper mirrors that with a defensive
     * no-op so a misuse from a future caller doesn't open a
     * stale dialog.
     */
    seedGame(MAPLE_TW, MAPLE_TW_INI)
    useAuthStore().session = FAKE_TW_SESSION

    const wrapper = mountStack()
    await flushPromises()

    await (wrapper.vm as unknown as ToolsDialogStackApi).openForGame('999999_XX')
    await flushPromises()

    expect(wrapper.get('[data-test="maple-tools-stub"]').attributes('data-visible')).toBe('false')
    expect(wrapper.get('[data-test="kart-tools-stub"]').attributes('data-visible')).toBe('false')
    expect(commands.detectGamePath).not.toHaveBeenCalled()
  })

  /* -------------------- MapleTools props threading -------------- */

  it('threads resolved gamePath + auth region into MapleTools props on TW open', async () => {
    /*
     * `gamePath` is read by `MapleTools::handleRecycling` (D2) for
     * the WPF-mirrored `commands.cleanMapleGameCache` IPC call —
     * the prop must reflect the freshly-resolved Config value.
     * `loginRegion` drives the WPF HK PlayerReport advisory
     * (`MapleTools.xaml.cs` L26 `App.LoginRegion == "HK"`).
     */
    seedGame(MAPLE_TW, MAPLE_TW_INI)
    useAuthStore().session = FAKE_TW_SESSION
    vi.mocked(commands.detectGamePath).mockReturnValueOnce(ok('D:\\Games\\MapleStory.exe'))

    const wrapper = mountStack()
    await flushPromises()

    await (wrapper.vm as unknown as ToolsDialogStackApi).openForGame('610074_T9')
    await flushPromises()

    /* detectGamePath called with the active INI's lookup metadata. */
    expect(commands.detectGamePath).toHaveBeenCalledTimes(1)
    expect(commands.detectGamePath).toHaveBeenCalledWith(
      '610074_T9',
      MAPLE_TW_INI.dir_value_name,
      MAPLE_TW_INI.dir_reg,
    )

    const stub = wrapper.get('[data-test="maple-tools-stub"]')
    expect(stub.attributes('data-game-path')).toBe('D:\\Games\\MapleStory.exe')
    expect(stub.attributes('data-login-region')).toBe('TW')
  })

  it('threads HK session region into MapleTools (drives WPF PlayerReport advisory)', async () => {
    /*
     * Pairs with the WPF `App.LoginRegion == "HK"` branch in
     * `MapleTools::btn_PlayerReport_Click` — the wrapper must
     * pass the live region through so the advisory toast fires.
     */
    seedGame(MAPLE_TW, MAPLE_TW_INI)
    useAuthStore().session = FAKE_HK_SESSION

    const wrapper = mountStack()
    await flushPromises()

    await (wrapper.vm as unknown as ToolsDialogStackApi).openForGame('610074_T9')
    await flushPromises()

    expect(wrapper.get('[data-test="maple-tools-stub"]').attributes('data-login-region')).toBe('HK')
  })

  /* -------------------- gamePath fallback paths ----------------- */

  it('passes empty gamePath to MapleTools when selectedIni is missing (no IPC fired)', async () => {
    /*
     * No game seeded → `useGameStore.selectedIni` is `null`. The
     * wrapper must short-circuit `detectGamePath` (it can't form
     * a Config key without `dir_value_name`) and pass empty
     * string as the prop. MapleTools then surfaces the WPF
     * `MsgCantFindGame` toast on Recycling click.
     */
    useAuthStore().session = FAKE_TW_SESSION

    const wrapper = mountStack()
    await flushPromises()

    /*
     * Seed selectedGameCode without seeding the INI map so the
     * routing branch fires for `610074_T9` while `selectedIni`
     * remains `null`.
     */
    const gameStore = useGameStore()
    gameStore.selectedGameCode = '610074_T9'

    await (wrapper.vm as unknown as ToolsDialogStackApi).openForGame('610074_T9')
    await flushPromises()

    expect(commands.detectGamePath).not.toHaveBeenCalled()
    const stub = wrapper.get('[data-test="maple-tools-stub"]')
    expect(stub.attributes('data-visible')).toBe('true')
    expect(stub.attributes('data-game-path')).toBe('')
  })

  it('passes empty gamePath when dir_value_name is empty (no Config key shape)', async () => {
    /*
     * Some INI rows ship with `dir_value_name === ''` (the entry
     * exists but the Config key isn't resolvable). Same fallback
     * path as the `null` INI case — short-circuit the IPC, pass
     * empty string downward.
     */
    seedGame(MAPLE_TW, { ...MAPLE_TW_INI, dir_value_name: '' })
    useAuthStore().session = FAKE_TW_SESSION

    const wrapper = mountStack()
    await flushPromises()

    await (wrapper.vm as unknown as ToolsDialogStackApi).openForGame('610074_T9')
    await flushPromises()

    expect(commands.detectGamePath).not.toHaveBeenCalled()
    expect(wrapper.get('[data-test="maple-tools-stub"]').attributes('data-game-path')).toBe('')
  })

  it('passes empty gamePath when detectGamePath fails (graceful fallback)', async () => {
    /*
     * Backend command failure (Config.xml parse error etc.) must
     * not block the dialog from opening — MapleTools owns the
     * "missing path → MsgCantFindGame" branch and that's the
     * right user-facing surface for this failure mode too.
     */
    seedGame(MAPLE_TW, MAPLE_TW_INI)
    useAuthStore().session = FAKE_TW_SESSION
    vi.mocked(commands.detectGamePath).mockReturnValueOnce(
      err({ code: 'config.read', message: 'parse error', details: null }),
    )

    const wrapper = mountStack()
    await flushPromises()

    await (wrapper.vm as unknown as ToolsDialogStackApi).openForGame('610074_T9')
    await flushPromises()

    expect(commands.detectGamePath).toHaveBeenCalledTimes(1)
    const stub = wrapper.get('[data-test="maple-tools-stub"]')
    expect(stub.attributes('data-visible')).toBe('true')
    expect(stub.attributes('data-game-path')).toBe('')
  })

  it('passes empty gamePath when detectGamePath returns null', async () => {
    /*
     * Backend returns `Ok(None)` for "key absent in Config.xml"
     * — the binding type is `string | null`. The wrapper coerces
     * `null` to empty string so the MapleTools prop type stays
     * `string` (the dialog's internal empty-string check covers
     * both "never set" and "explicitly empty" with one branch).
     */
    seedGame(MAPLE_TW, MAPLE_TW_INI)
    useAuthStore().session = FAKE_TW_SESSION
    vi.mocked(commands.detectGamePath).mockReturnValueOnce(ok(null))

    const wrapper = mountStack()
    await flushPromises()

    await (wrapper.vm as unknown as ToolsDialogStackApi).openForGame('610074_T9')
    await flushPromises()

    expect(wrapper.get('[data-test="maple-tools-stub"]').attributes('data-game-path')).toBe('')
  })

  /* -------------------- child-dialog event chain ---------------- */

  it('opens WebBrowser with the URL when MapleTools emits open-web-browser', async () => {
    seedGame(MAPLE_TW, MAPLE_TW_INI)
    useAuthStore().session = FAKE_TW_SESSION

    const wrapper = mountStack()
    await flushPromises()
    await (wrapper.vm as unknown as ToolsDialogStackApi).openForGame('610074_T9')
    await flushPromises()

    /* Synthesise the event MapleTools.vue would emit on PlayerReport click. */
    const url = 'https://event.beanfun.com/customerservice/PluginReporting/PlayerReport.aspx'
    wrapper.findComponent({ name: 'MapleTools' }).vm.$emit('open-web-browser', url)
    await flushPromises()

    const browserStub = wrapper.get('[data-test="web-browser-stub"]')
    expect(browserStub.attributes('data-visible')).toBe('true')
    expect(browserStub.attributes('data-url')).toBe(url)
  })

  it('opens WebBrowser with the URL when KartTools emits open-web-browser', async () => {
    /*
     * Same shared WebBrowser mount handles both pipelines —
     * locks down the "one mount, two parents" decision in the
     * wrapper docblock.
     */
    seedGame(KART_TW, KART_TW_INI)
    useAuthStore().session = FAKE_TW_SESSION

    const wrapper = mountStack()
    await flushPromises()
    await (wrapper.vm as unknown as ToolsDialogStackApi).openForGame('610096_TE')
    await flushPromises()

    const url = 'https://tw.beanfun.com/KartRider/guild/search_member.aspx'
    wrapper.findComponent({ name: 'KartTools' }).vm.$emit('open-web-browser', url)
    await flushPromises()

    const browserStub = wrapper.get('[data-test="web-browser-stub"]')
    expect(browserStub.attributes('data-visible')).toBe('true')
    expect(browserStub.attributes('data-url')).toBe(url)
  })

  it('opens EquipCalculator when MapleTools emits open-equip-calculator', async () => {
    seedGame(MAPLE_TW, MAPLE_TW_INI)
    useAuthStore().session = FAKE_TW_SESSION

    const wrapper = mountStack()
    await flushPromises()
    await (wrapper.vm as unknown as ToolsDialogStackApi).openForGame('610074_T9')
    await flushPromises()

    wrapper.findComponent({ name: 'MapleTools' }).vm.$emit('open-equip-calculator')
    await flushPromises()

    expect(wrapper.get('[data-test="equip-calc-stub"]').attributes('data-visible')).toBe('true')
  })

  it('opens CoreCalculator when MapleTools emits open-core-calculator', async () => {
    seedGame(MAPLE_TW, MAPLE_TW_INI)
    useAuthStore().session = FAKE_TW_SESSION

    const wrapper = mountStack()
    await flushPromises()
    await (wrapper.vm as unknown as ToolsDialogStackApi).openForGame('610074_T9')
    await flushPromises()

    wrapper.findComponent({ name: 'MapleTools' }).vm.$emit('open-core-calculator')
    await flushPromises()

    expect(wrapper.get('[data-test="core-calc-stub"]').attributes('data-visible')).toBe('true')
  })

  /* -------------------- snapshot freshness ---------------------- */

  it('re-resolves gamePath on every openForGame call (matches WPF "fresh on each open")', async () => {
    /*
     * WPF reads `App.MainWnd.settingPage.t_GamePath.Text` at
     * Recycling click time — the value is whatever Config.xml
     * currently holds. In the SPA the modal blocks navigation
     * mid-dialog, so resolving on each `openForGame` invocation
     * gives the same "value at click time" semantics. This test
     * ensures the lookup actually re-runs (not just on first
     * open).
     */
    seedGame(MAPLE_TW, MAPLE_TW_INI)
    useAuthStore().session = FAKE_TW_SESSION
    vi.mocked(commands.detectGamePath)
      .mockReturnValueOnce(ok('C:\\First.exe'))
      .mockReturnValueOnce(ok('D:\\Second.exe'))

    const wrapper = mountStack()
    await flushPromises()

    await (wrapper.vm as unknown as ToolsDialogStackApi).openForGame('610074_T9')
    await flushPromises()
    expect(wrapper.get('[data-test="maple-tools-stub"]').attributes('data-game-path')).toBe(
      'C:\\First.exe',
    )

    await (wrapper.vm as unknown as ToolsDialogStackApi).openForGame('610074_T9')
    await flushPromises()
    expect(wrapper.get('[data-test="maple-tools-stub"]').attributes('data-game-path')).toBe(
      'D:\\Second.exe',
    )

    expect(commands.detectGamePath).toHaveBeenCalledTimes(2)
  })
})
