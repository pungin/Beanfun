/**
 * Game-launcher composable — encapsulates the 5-step WPF
 * `runGame(account, password)` chain (`MainWindow.xaml.cs`
 * L1724-1900) so multiple components can launch the active
 * game without re-implementing the same control flow.
 *
 * # Why a composable (not a service / not a store)
 *
 * - **Stateless** — every method is a pure function of `(args,
 *   game store, config store, i18n)`. There's nothing worth
 *   keeping in a Pinia store between calls; the composable is
 *   purely a code-organization tool that hoists the launch
 *   pipeline out of `pages/AccountList.vue` so
 *   `pages/IdPassForm.vue` + `pages/QrForm.vue` can re-use it
 *   for the WPF `btn_StartGame_Click` parity buttons (P12.4
 *   followup-A D7/D8).
 * - **Needs UI primitives** — `ElMessage` / `ElMessageBox` are
 *   Vue-context-only APIs (they read from the Element Plus
 *   context provider), which rules out a plain `services/*.ts`
 *   module. Wrapping in a composable keeps the `useI18n()` /
 *   store calls inside Vue's reactive scope where they're
 *   guaranteed to resolve.
 * - **No hidden state** — multiple instantiations of the
 *   composable in the same component tree share zero local
 *   state by design (each consumer holds its own closure).
 *   This matches the original AccountList behaviour where each
 *   `runGame()` call read fresh values off the stores.
 *
 * # WPF parity table
 *
 * | Step                               | This composable                     | WPF source              |
 * |------------------------------------|--------------------------------------|-------------------------|
 * | INI / game selection guard         | `runGame` first 4 lines              | L1727 `gamePath = settingPage.t_GamePath.Text` (path resolves via INI under the hood) |
 * | `t_GamePath` resolution + missing  | `resolveGamePath`                    | L1727-1751 |
 * | Wide-char path advisory warn       | `pathHasWideChar` + warn toast       | L1753-1763 |
 * | Already-running process check      | `checkAndKillRunningGameProcesses`   | L1765-1833 |
 * | Auto/Normal/LR mode resolution     | `resolveStartMode`                   | L1837-1864 |
 * | Final spawn                        | `commands.launchGame(...)`           | L1866-1892 |
 *
 * Each helper is internal to this module — only `runGame` is
 * exported via the returned object. Tests exercise `runGame`
 * end-to-end with mocked commands; per-helper tests would
 * couple to internal API and break on every refactor.
 *
 * # Why no try/catch around `commands.launchGame`
 *
 * Same rationale as the original AccountList implementation
 * (see deleted docblock in `pages/AccountList.vue` D4 history):
 * the IPC funnels failures through `wrapCommand` already
 * (toast + console). WPF L1895-1899 wraps the launch in a
 * try/catch that surfaces `MsgLocalePluginRunError` for
 * LR-mode failures; the backend `launch_game` already maps
 * LR-resource / spawn errors to structured `CommandError`
 * codes that the frontend toasts via `wrapCommand`.
 * Re-catching here would double-toast.
 *
 * # State pre-conditions (caller responsibility)
 *
 * `runGame` assumes `useGameStore().selectedGameCode` /
 * `selectedIni` / `selectedGame` are populated. When called
 * from a context where they may not be (e.g. `IdPassForm.vue`
 * after logout cleared `clearGameData`), the caller is
 * expected to first invoke `game.restoreLastSelected()` (P12.4
 * followup-A D6) to repopulate from the persisted config
 * snapshot. If the state is still unset after that attempt,
 * `runGame` short-circuits with the same `GameSelected` toast
 * WPF surfaces — no silent failure.
 */

import { ElMessage, ElMessageBox } from 'element-plus'
import { useI18n } from 'vue-i18n'

import { useConfigStore } from '../stores/config'
import { useGameStore } from '../stores/game'
import { commands, type GameStartMode } from '../types/bindings'
import { safeInvoke, wrapCommand } from './../services/invoke'

/**
 * Public surface of the launcher composable. Single method by
 * design — every consumer needs the full launch chain (no
 * partial use cases discovered after porting to two LoginForm
 * call sites + the original AccountList call site).
 */
export interface UseGameLauncherReturn {
  /**
   * Run the WPF `runGame(account, password)` chain to spawn the
   * currently-selected game. Optional credential args mirror
   * WPF L1724 default `null` → SPA empty-string default = "no
   * credential substitution into `command_line`".
   *
   * Returns `Promise<void>`; failures are toasted via
   * `wrapCommand` / advisory warnings, never thrown.
   */
  runGame: (accountId?: string, password?: string) => Promise<void>
}

const MAPLE_GUARD_GAME_CODES = new Set(['610074_T9', '610075_T9'])
const MAPLE_GUARD_INTERVAL_MS = 100
const MAPLE_GUARD_TICKS = 150

function parseBool(value: string | undefined, fallback: boolean): boolean {
  if (value === undefined || value === '') return fallback
  return value.toLowerCase() === 'true'
}

export function useGameLauncher(): UseGameLauncherReturn {
  const { t } = useI18n()
  const game = useGameStore()
  const configStore = useConfigStore()

  /**
   * Map the `startGameMode` config value (string-encoded integer
   * `"0"` / `"1"` / `"2"` per WPF `MainWindow.xaml.cs` L1837 +
   * `enum GameStartMode` L32-37) to the backend [`GameStartMode`]
   * tagged union.
   *
   * The backend enum re-serialises as PascalCase string literals
   * (`"Auto"` / `"Normal"` / `"LocaleRemulator"`) over IPC —
   * mapping here lets the frontend read the legacy integer
   * config without the backend having to accept either shape.
   * Defaults to `Auto` on missing / unparseable input, mirroring
   * WPF's `int.Parse` fallback path (default `"0"` is `Auto`).
   */
  function resolveStartMode(): GameStartMode {
    const raw = configStore.getOr('startGameMode', '0')
    const parsed = Number.parseInt(raw, 10)
    if (Number.isNaN(parsed) || parsed <= 0) return 'Auto'
    if (parsed === 1) return 'Normal'
    return 'LocaleRemulator'
  }

  /**
   * Test whether `gamePath` contains any non-ASCII char (>0x80) —
   * mirrors WPF `MainWindow.xaml.cs` L1753-1763 which warns on
   * "wide chars" in the path because Locale Emulator (the LR
   * launch mode) is known to choke on multi-byte path characters.
   *
   * The check is purely advisory — we surface
   * `MsgGamePathHaveWChar` but continue with the launch
   * (matching WPF's `break` after the MessageBox).
   */
  function pathHasWideChar(gamePath: string): boolean {
    for (let i = 0; i < gamePath.length; i += 1) {
      if (gamePath.charCodeAt(i) > 128) return true
    }
    return false
  }

  /**
   * Resolve the install path for the active game's executable.
   *
   * Mirrors WPF L1727-1751:
   * 1. Read from `settingPage.t_GamePath.Text` (= Config.xml under
   *    `<dir_value_name>.<gameCode>` in the SPA backend, see
   *    `commands/launcher.rs::detect_game_path`).
   * 2. If empty / file missing → `MsgCantFindGame` Yes/No prompt.
   *    - Yes (or no game selected) → would normally open the file
   *      picker (`btn_SetGamePath_Click`). The Settings page is
   *      P12.4 scope; until then we surface a
   *      `accountList.gamePathPickerPending` toast so the user
   *      knows the redirect target is missing.
   *    - No → open the game's `download_url` (`Process.Start` with
   *      `UseShellExecute = true` — IPC equivalent is
   *      [`commands.openUrl`]).
   *    Either branch returns `null` to signal "abort the launch".
   * 3. Re-read after the prompt (WPF L1747-1751 paranoid
   *    double-check in case the user picked a path mid-flow).
   *    Replicated for parity even though the SPA's prompt path
   *    doesn't currently mutate the config inline.
   *
   * Returns the resolved game path on success, `null` when the
   * launch should abort (user chose "No" / pending-Settings toast
   * surfaced / re-read still empty).
   */
  async function resolveGamePath(): Promise<string | null> {
    const ini = game.selectedIni
    if (!ini || game.selectedGameCode === null) {
      ElMessage.warning(t('GameSelected'))
      return null
    }

    /*
     * P12.4 followup-A D7 — `selectedGame` may be null when
     * `runGame` was invoked from a LoginPage form (post-logout,
     * `services` was wiped by `clearGameData`). The launch
     * subset (`selectedGameCode` + `selectedIni`) is enough to
     * detect the path + spawn; the only `selectedGame`-dependent
     * branch is the `download_url` fallback below, which already
     * has a graceful pending-Settings toast for empty URLs and
     * is widened to also cover the null-selected case here.
     * Mirrors the documented contract on
     * `useGameStore.restoreLastSelected` (game.ts L334-339).
     */
    const selected = game.selectedGame

    const detectResult = await safeInvoke(
      commands.detectGamePath(game.selectedGameCode, ini.dir_value_name, ini.dir_reg, ini.exe),
    )
    const detected = detectResult.ok ? (detectResult.data ?? '') : ''

    if (detected !== '') return detected

    /*
     * MsgCantFindGame prompt mirrors WPF L1730-1746:
     *   Yes → open the path picker (Settings page P12.4 stub).
     *   No  → open the download URL.
     * `ElMessageBox.confirm` rejects on Cancel (= the picker close
     * button); we treat that as "cancel the launch" rather than a
     * third option.
     */
    let prompt: 'yes' | 'no'
    try {
      await ElMessageBox.confirm(t('MsgCantFindGame'), '', {
        confirmButtonText: t('Yes'),
        cancelButtonText: t('No'),
        type: 'warning',
      })
      prompt = 'yes'
    } catch (cancelOrNo) {
      /*
       * `ElMessageBox.confirm` rejects with `'cancel'` on Cancel
       * and `'close'` on the X / Esc. Treat `'cancel'` as the
       * "No" branch so the user can use the cancel button as the
       * WPF `No` button (matches mockup parity); `'close'`
       * short-circuits the launch.
       */
      if (cancelOrNo === 'cancel') {
        prompt = 'no'
      } else {
        return null
      }
    }

    if (prompt === 'yes') {
      /*
       * Settings page (where `btn_SetGamePath_Click` lives) is
       * P12.4 scope. Surface a pending-Settings toast so the user
       * isn't left wondering why nothing happened — and so QA can
       * spot the missing redirect.
       */
      ElMessage.info(t('accountList.gamePathPickerPending'))
      return null
    }

    /*
     * `download_url` is empty for some unconnected games (the
     * Beanfun catalogue doesn't always populate it). Same
     * pending-Settings fallback also covers the
     * `selected === null` case (LoginPage post-logout — the
     * catalogue isn't loaded so `selectedGame` is null even
     * though the launch subset survives via `restoreLastSelected`).
     */
    if (!selected || selected.download_url === '') {
      ElMessage.info(t('accountList.gamePathPickerPending'))
      return null
    }
    await safeInvoke(commands.openUrl(selected.download_url))
    return null
  }

  /**
   * Already-running-process check + optional kill prompt.
   * Mirrors WPF L1765-1833:
   * 1. Enumerate processes whose `executable_path` matches the
   *    target `gamePath` (backend `list_game_processes`
   *    encapsulates the WPF process-name regex + WMI filter).
   * 2. If any match, prompt `MsgGameAlreadyRun` Yes/No.
   *    - Yes → `kill_game_processes(pids)` and continue.
   *    - No  → continue without killing (WPF launches anyway,
   *      treating the prompt as advisory).
   *
   * Returns `true` to proceed with the launch, `false` to abort
   * (currently never returns `false` — kept as the return shape
   * for forward-compat with any future "abort on cancel" branch
   * a Settings toggle might add).
   */
  async function checkAndKillRunningGameProcesses(gamePath: string): Promise<boolean> {
    const listResult = await safeInvoke(commands.listGameProcesses(gamePath))
    if (!listResult.ok) {
      /*
       * Process enumeration failed (rare — usually a WMI
       * permission issue). The standard wrapCommand-style toast
       * would be too loud here; log and continue so the user can
       * still launch.
       */
      console.warn('[useGameLauncher] listGameProcesses failed:', listResult.error)
      return true
    }
    if (listResult.data.length === 0) return true

    let confirmed: boolean
    try {
      await ElMessageBox.confirm(t('MsgGameAlreadyRun'), '', {
        confirmButtonText: t('Yes'),
        cancelButtonText: t('No'),
        type: 'warning',
      })
      confirmed = true
    } catch (cancelOrNo) {
      /*
       * Mirrors WPF: only the explicit Yes branch kills processes.
       * `cancel` (No) and `close` (Esc / X) both fall through to
       * "launch anyway" — WPF doesn't gate the launch on the kill
       * prompt either.
       */
      void cancelOrNo
      confirmed = false
    }

    if (confirmed) {
      const pids = listResult.data.map((p) => p.pid)
      await safeInvoke(commands.killGameProcesses(pids))
    }
    return true
  }

  function runMapleLaunchGuards(gamePath: string): void {
    const code = game.selectedGameCode
    if (code === null || !MAPLE_GUARD_GAME_CODES.has(code)) return

    const skipPlayWnd = parseBool(configStore.get('skipPlayWnd'), true)
    const autoKillPatcher = parseBool(configStore.get('autoKillPatcher'), true)
    if (!skipPlayWnd && !autoKillPatcher) return

    let remaining = MAPLE_GUARD_TICKS
    const tick = (): void => {
      if (skipPlayWnd) {
        void safeInvoke(commands.closeMaplePlayWindow())
      }
      if (autoKillPatcher) {
        void safeInvoke(commands.checkAndKillMaplePatcher(gamePath))
      }
      remaining -= 1
      if (remaining <= 0) window.clearInterval(timer)
    }

    const timer = window.setInterval(tick, MAPLE_GUARD_INTERVAL_MS)
    tick()
  }

  async function runGame(accountId = '', password = ''): Promise<void> {
    /*
     * P12.4 followup-A D6 — when invoked from the LoginPage
     * forms (`pages/IdPassForm.vue` / `pages/QrForm.vue` GameStart
     * buttons), the in-memory game store is empty (no
     * authenticated session has run `loadGames`, and a previous
     * logout fired `clearGameData`). `restoreLastSelected` patches
     * `selectedGameCode` + the matching `ini` entry from the
     * persisted Config.xml snapshot so the rest of this chain has
     * the same INI fields available as a post-login AccountList
     * launch. Returns `false` when the snapshot is absent / corrupt
     * — we surface the same `GameSelected` toast WPF shows when
     * `service_code` is empty.
     *
     * No-op short-circuit when the store already has a live
     * selection (the post-login AccountList launch path), so this
     * adds zero cost to the existing call sites.
     */
    if (!game.restoreLastSelected(configStore)) {
      ElMessage.warning(t('GameSelected'))
      return
    }

    const ini = game.selectedIni
    if (!ini) {
      ElMessage.warning(t('GameSelected'))
      return
    }

    const gamePath = await resolveGamePath()
    if (gamePath === null) return

    if (pathHasWideChar(gamePath)) {
      /*
       * Advisory-only — WPF L1760-1762 shows the MsgBox then
       * `break`s out of the scan loop and continues the launch.
       * We do the same with a non-blocking warning toast.
       */
      ElMessage.warning(t('MsgGamePathHaveWChar'))
    }

    const proceed = await checkAndKillRunningGameProcesses(gamePath)
    if (!proceed) return

    const mode = resolveStartMode()

    await wrapCommand(commands.launchGame(gamePath, mode, ini.exe, accountId, password))
    runMapleLaunchGuards(gamePath)
  }

  return { runGame }
}
