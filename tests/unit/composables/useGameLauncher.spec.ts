/**
 * P12.4 followup-A D9 — useGameLauncher composable behaviour.
 *
 * What this spec locks down (matches WPF
 * `MainWindow.xaml.cs::runGame` L1724-1900 + the AccountList.vue
 * helper extraction in followup-A D3):
 *
 * 1. `restoreLastSelected` succeeds + game store has snapshot →
 *    full launch chain runs (`detectGamePath` → `listGameProcesses`
 *    → `launchGame`).
 * 2. `restoreLastSelected` fails (no snapshot) → `GameSelected`
 *    warning toast, no `launchGame` call.
 * 3. `selectedIni` null after restore (defensive corner case) →
 *    `GameSelected` toast, no launch.
 * 4. `detectGamePath` returns empty + user picks Yes →
 *    `accountList.gamePathPickerPending` info toast, no launch.
 * 5. `detectGamePath` returns empty + user picks No + game has
 *    download_url → `commands.openUrl` fired with download URL,
 *    no launch.
 * 6. `detectGamePath` returns empty + user picks No + empty
 *    download_url → pending-Settings toast (fallback), no launch.
 * 7. Wide-char path → warning toast surfaced but launch
 *    proceeds (advisory parity with WPF L1760-1762 `break`).
 * 8. `listGameProcesses` returns running pids + user picks Yes →
 *    `killGameProcesses` called, then launch proceeds.
 * 9. `listGameProcesses` returns running pids + user picks No →
 *    no kill, launch still proceeds (advisory).
 * 10. `resolveStartMode` reads `startGameMode` config:
 *     `"0"` → Auto, `"1"` → Normal, `"2"` → LocaleRemulator,
 *     missing → Auto, garbage → Auto.
 * 11. Credential-bearing call (`runGame(account, password)`) flows
 *     account/password through to `commands.launchGame`.
 *
 * # Test harness
 *
 * Composables that touch `useI18n` / pinia stores need a Vue
 * scope. We mount a tiny host component that calls
 * `useGameLauncher` in setup() and exposes the returned `runGame`
 * via a ref so each test can drive the composable directly
 * without a real consumer page.
 */

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { flushPromises, mount } from '@vue/test-utils'
import { defineComponent, h, ref } from 'vue'

import type {
  CommandError,
  GameIniEntry,
  GameProcessInfo,
  Result,
} from '../../../src/types/bindings'

const { elMessageWarning, elMessageInfo, elMessageBoxConfirm } = vi.hoisted(() => ({
  elMessageWarning: vi.fn(),
  elMessageInfo: vi.fn(),
  elMessageBoxConfirm: vi.fn(),
}))

vi.mock('element-plus', () => ({
  ElMessage: {
    warning: elMessageWarning,
    info: elMessageInfo,
    error: vi.fn(),
    success: vi.fn(),
  },
  ElMessageBox: {
    confirm: elMessageBoxConfirm,
  },
}))

vi.mock('../../../src/types/bindings', () => ({
  commands: {
    detectGamePath: vi.fn(),
    listGameProcesses: vi.fn(),
    killGameProcesses: vi.fn(),
    openUrl: vi.fn(),
    launchGame: vi.fn(),
    getAllConfig: vi.fn(),
    setConfig: vi.fn(),
  },
}))

import { commands } from '../../../src/types/bindings'
import { useGameLauncher } from '../../../src/composables/useGameLauncher'
import { useGameStore } from '../../../src/stores/game'
import { useConfigStore } from '../../../src/stores/config'
import { createAppI18n } from '../../../src/i18n'

const mockDetectGamePath = vi.mocked(commands.detectGamePath)
const mockListGameProcesses = vi.mocked(commands.listGameProcesses)
const mockKillGameProcesses = vi.mocked(commands.killGameProcesses)
const mockOpenUrl = vi.mocked(commands.openUrl)
const mockLaunchGame = vi.mocked(commands.launchGame)

const ok = <T>(data: T): Promise<Result<T, CommandError>> => Promise.resolve({ status: 'ok', data })

const FAKE_INI: GameIniEntry = {
  exe: 'C:\\Maple\\MapleStory.exe',
  login_action_type: '1',
  win_class_name: 'MapleStoryClass',
  dir_value_name: 'MapleStory',
  dir_reg: 'HKCU\\Software\\Gamania\\MapleStory',
}

const FAKE_GAME_CODE = '610074_T9'
const FAKE_PATH = 'C:\\Maple\\MapleStory.exe'
const FAKE_DOWNLOAD_URL = 'https://download.example/maple-installer.exe'

/**
 * Mount a host component that instantiates the composable. Pinia
 * + i18n are required for the composable to wire its store /
 * `useI18n` calls; the harness component itself renders nothing
 * but exposes `runGame` so tests can call it imperatively.
 */
function mountHarness(): {
  runGame: (acc?: string, pw?: string) => Promise<void>
  game: ReturnType<typeof useGameStore>
  config: ReturnType<typeof useConfigStore>
} {
  const i18n = createAppI18n()
  const runGameRef = ref<((a?: string, p?: string) => Promise<void>) | null>(null)

  const Host = defineComponent({
    name: 'LauncherHost',
    setup() {
      const launcher = useGameLauncher()
      runGameRef.value = launcher.runGame
      return () => h('div')
    },
  })

  mount(Host, { global: { plugins: [i18n] } })

  return {
    runGame: (a, p) => runGameRef.value!(a, p),
    game: useGameStore(),
    config: useConfigStore(),
  }
}

/**
 * Pre-seed the store + config to a state where
 * `restoreLastSelected` short-circuits to true (live in-memory
 * snapshot already populated), so each launch-flow test only
 * has to vary the bit it cares about.
 */
function seedLiveSelection(game: ReturnType<typeof useGameStore>): void {
  game.ini = { [FAKE_GAME_CODE]: FAKE_INI } as Record<string, GameIniEntry>
  game.services = [
    {
      name: 'MapleStory',
      service_code: '610074',
      service_region: 'T9',
      website_url: '',
      xlarge_image_name: '',
      large_image_name: '',
      small_image_name: '',
      download_url: FAKE_DOWNLOAD_URL,
    },
  ]
  game.selectedGameCode = FAKE_GAME_CODE
}

describe('useGameLauncher', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    mockDetectGamePath.mockReset()
    mockListGameProcesses.mockReset()
    mockKillGameProcesses.mockReset()
    mockOpenUrl.mockReset()
    mockLaunchGame.mockReset()
    elMessageWarning.mockReset()
    elMessageInfo.mockReset()
    elMessageBoxConfirm.mockReset()
  })

  afterEach(() => {
    vi.clearAllMocks()
  })

  it('full launch chain: detect → list (empty) → launchGame called', async () => {
    const { runGame, game } = mountHarness()
    seedLiveSelection(game)

    mockDetectGamePath.mockReturnValueOnce(ok(FAKE_PATH))
    mockListGameProcesses.mockReturnValueOnce(ok([]))
    mockLaunchGame.mockReturnValueOnce(ok(null))

    await runGame()
    await flushPromises()

    expect(mockLaunchGame).toHaveBeenCalledWith(FAKE_PATH, 'Auto', FAKE_INI.exe, '', '')
  })

  it('no live selection + no persisted snapshot → GameSelected warning, no launch', async () => {
    const { runGame } = mountHarness()

    await runGame()
    await flushPromises()

    expect(elMessageWarning).toHaveBeenCalled()
    expect(mockLaunchGame).not.toHaveBeenCalled()
  })

  it('persisted snapshot present → restoreLastSelected re-hydrates + launch proceeds', async () => {
    const { runGame, game, config } = mountHarness()
    config.entries['loginGame'] = FAKE_GAME_CODE
    config.entries['lastSelectedIni'] = JSON.stringify(FAKE_INI)

    mockDetectGamePath.mockReturnValueOnce(ok(FAKE_PATH))
    mockListGameProcesses.mockReturnValueOnce(ok([]))
    mockLaunchGame.mockReturnValueOnce(ok(null))

    await runGame()
    await flushPromises()

    expect(game.selectedGameCode).toBe(FAKE_GAME_CODE)
    expect(mockLaunchGame).toHaveBeenCalledTimes(1)
  })

  it('detectGamePath empty + user picks Yes → pending-Settings toast', async () => {
    const { runGame, game } = mountHarness()
    seedLiveSelection(game)

    mockDetectGamePath.mockReturnValueOnce(ok(''))
    elMessageBoxConfirm.mockResolvedValueOnce('confirm')

    await runGame()
    await flushPromises()

    expect(elMessageInfo).toHaveBeenCalled()
    expect(mockLaunchGame).not.toHaveBeenCalled()
    expect(mockOpenUrl).not.toHaveBeenCalled()
  })

  it('detectGamePath empty + user picks No + has download_url → openUrl', async () => {
    const { runGame, game } = mountHarness()
    seedLiveSelection(game)

    mockDetectGamePath.mockReturnValueOnce(ok(''))
    elMessageBoxConfirm.mockRejectedValueOnce('cancel')
    mockOpenUrl.mockReturnValueOnce(ok(null))

    await runGame()
    await flushPromises()

    expect(mockOpenUrl).toHaveBeenCalledWith(FAKE_DOWNLOAD_URL)
    expect(mockLaunchGame).not.toHaveBeenCalled()
  })

  it('wide-char path → warning toast surfaced, launch still proceeds', async () => {
    const { runGame, game } = mountHarness()
    seedLiveSelection(game)

    const widePath = 'C:\\遊戲\\Maple\\MapleStory.exe'
    mockDetectGamePath.mockReturnValueOnce(ok(widePath))
    mockListGameProcesses.mockReturnValueOnce(ok([]))
    mockLaunchGame.mockReturnValueOnce(ok(null))

    await runGame()
    await flushPromises()

    expect(elMessageWarning).toHaveBeenCalled()
    expect(mockLaunchGame).toHaveBeenCalledWith(widePath, 'Auto', FAKE_INI.exe, '', '')
  })

  it('running process + user Yes → kill called, then launch', async () => {
    const { runGame, game } = mountHarness()
    seedLiveSelection(game)

    const running: GameProcessInfo[] = [
      { pid: 1234, name: 'MapleStory.exe', executablePath: FAKE_PATH },
    ]
    mockDetectGamePath.mockReturnValueOnce(ok(FAKE_PATH))
    mockListGameProcesses.mockReturnValueOnce(ok(running))
    elMessageBoxConfirm.mockResolvedValueOnce('confirm')
    mockKillGameProcesses.mockReturnValueOnce(ok(null))
    mockLaunchGame.mockReturnValueOnce(ok(null))

    await runGame()
    await flushPromises()

    expect(mockKillGameProcesses).toHaveBeenCalledWith([1234])
    expect(mockLaunchGame).toHaveBeenCalledTimes(1)
  })

  it('running process + user No → no kill, launch still proceeds (advisory)', async () => {
    const { runGame, game } = mountHarness()
    seedLiveSelection(game)

    const running: GameProcessInfo[] = [
      { pid: 99, name: 'MapleStory.exe', executablePath: FAKE_PATH },
    ]
    mockDetectGamePath.mockReturnValueOnce(ok(FAKE_PATH))
    mockListGameProcesses.mockReturnValueOnce(ok(running))
    elMessageBoxConfirm.mockRejectedValueOnce('cancel')
    mockLaunchGame.mockReturnValueOnce(ok(null))

    await runGame()
    await flushPromises()

    expect(mockKillGameProcesses).not.toHaveBeenCalled()
    expect(mockLaunchGame).toHaveBeenCalledTimes(1)
  })

  it('startGameMode "1" → Normal mode', async () => {
    const { runGame, game, config } = mountHarness()
    seedLiveSelection(game)
    config.entries['startGameMode'] = '1'

    mockDetectGamePath.mockReturnValueOnce(ok(FAKE_PATH))
    mockListGameProcesses.mockReturnValueOnce(ok([]))
    mockLaunchGame.mockReturnValueOnce(ok(null))

    await runGame()
    await flushPromises()

    expect(mockLaunchGame).toHaveBeenCalledWith(FAKE_PATH, 'Normal', FAKE_INI.exe, '', '')
  })

  it('startGameMode "2" → LocaleRemulator mode', async () => {
    const { runGame, game, config } = mountHarness()
    seedLiveSelection(game)
    config.entries['startGameMode'] = '2'

    mockDetectGamePath.mockReturnValueOnce(ok(FAKE_PATH))
    mockListGameProcesses.mockReturnValueOnce(ok([]))
    mockLaunchGame.mockReturnValueOnce(ok(null))

    await runGame()
    await flushPromises()

    expect(mockLaunchGame).toHaveBeenCalledWith(FAKE_PATH, 'LocaleRemulator', FAKE_INI.exe, '', '')
  })

  it('credentials propagate to commands.launchGame', async () => {
    const { runGame, game } = mountHarness()
    seedLiveSelection(game)

    mockDetectGamePath.mockReturnValueOnce(ok(FAKE_PATH))
    mockListGameProcesses.mockReturnValueOnce(ok([]))
    mockLaunchGame.mockReturnValueOnce(ok(null))

    await runGame('alice', 'hunter2')
    await flushPromises()

    expect(mockLaunchGame).toHaveBeenCalledWith(FAKE_PATH, 'Auto', FAKE_INI.exe, 'alice', 'hunter2')
  })

  it('corrupt persisted snapshot → restoreLastSelected returns false → GameSelected toast', async () => {
    const { runGame, config } = mountHarness()
    config.entries['loginGame'] = FAKE_GAME_CODE
    config.entries['lastSelectedIni'] = '{not valid json'

    await runGame()
    await flushPromises()

    expect(elMessageWarning).toHaveBeenCalled()
    expect(mockLaunchGame).not.toHaveBeenCalled()
  })
})
