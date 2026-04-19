<script setup lang="ts">
/**
 * Per-game Tools dialog stack — single-mount wrapper that hosts
 * MapleTools / KartTools and the three child dialogs they can
 * delegate to (WebBrowser / EquipCalculator / CoreCalculator).
 *
 * P12.5 D7. Replaces the `console.warn` stubs in
 * `pages/AccountList.vue::handleTools` and
 * `pages/Settings.vue::handleTools` with the real WPF parity
 * dispatch.
 *
 * # WPF parity
 *
 * Mirrors `Beanfun/Pages/AccountList.xaml.cs::btn_Tools_Click`
 * L237-250:
 *
 * ```cs
 * string gameCode = service_code + "_" + service_region;
 * switch (gameCode)
 * {
 *     case "610074_T9":
 *     case "610075_T9": new MapleTools().Show(); break;
 *     case "610096_TE": new KartTools().Show();  break;
 * }
 * ```
 *
 * The WPF `Settings.xaml.cs::btn_Tools_Click` (L271-275)
 * delegates to the same switch via `App.MainWnd.accountList
 * .btn_Tools_Click(null, null)` — so both pages funnel through
 * one dispatch in WPF. We mirror that "one dispatch, two
 * mounts" shape by extracting the dispatch into this wrapper
 * and mounting one instance per page that has a Tools button.
 *
 * # Why a wrapper component (not a composable / store / global mount)
 *
 * The per-page Tools button has to host five dialogs — MapleTools,
 * KartTools, plus the three children (WebBrowser, EquipCalculator,
 * CoreCalculator) that MapleTools delegates to via
 * `open-web-browser` / `open-equip-calculator` /
 * `open-core-calculator` events. The children must sit as
 * **siblings** (not nested children) of MapleTools/KartTools so
 * Element Plus's natural mount-order z-index stacks them on top
 * (a nested `<el-dialog>` inside another `<el-dialog>`
 * teleports to `body` via `append-to-body` and ends up beneath
 * the parent — see D2 dialog hosting decision).
 *
 * Three options were considered:
 *
 * 1. **Inline in each parent page** — duplicates 5 dialog mounts
 *    + the dispatch logic across `AccountList.vue` and
 *    `Settings.vue`. Bad DRY.
 *
 * 2. **Composable returning refs** — composable can centralise
 *    the state but the parent template still has to render the
 *    five `<el-dialog>` mounts. Splits one concern across two
 *    files for no payoff (`<script>` ⇄ `<template>` round-trip).
 *
 * 3. **Wrapper component + `defineExpose`** — single-mount
 *    component owns dispatch, state, and dialog mounts; parent
 *    just calls `toolsDialog.value?.openForGame(gameCode)`.
 *    Imperative entry point matches the WPF "click → switch →
 *    `new MapleTools().Show()`" shape directly. Picked option 3.
 *
 * # Why imperative `defineExpose` instead of `v-model`
 *
 * The trigger is a one-shot click → "open the appropriate dialog
 * for the *current* gameCode". A `v-model:open` boolean would
 * either need a companion `:game-code` prop the parent threads
 * through (extra surface area) or a watcher on `gameCode` inside
 * the wrapper that opens on every store mutation (semantically
 * wrong — the user pressing the button is the trigger, not the
 * game switch). Exposing `openForGame(gameCode)` keeps the
 * "press → open" causation explicit and matches WPF's direct
 * `btn_Tools_Click` event-handler shape.
 *
 * No state needs to round-trip back to the parent (the dialogs
 * own their own visibility refs internally), so there's nothing
 * for `v-model` to express.
 *
 * # Why gamePath is resolved here (not threaded as a prop)
 *
 * `MapleTools` consumes `gamePath` for the Recycling action
 * (`commands.cleanMapleGameCache(gamePath)` — see
 * `windows/MapleTools.vue` L174-180). WPF reads
 * `App.MainWnd.settingPage.t_GamePath.Text` at Recycling click
 * time (`MapleTools.xaml.cs` L62-63), which is whatever value
 * Config.xml currently holds for the active game. Resolving on
 * `openForGame` invocation gives the SPA the same "fresh on
 * each open" semantics:
 *
 * - The dialog uses `:destroy-on-close` so each open re-mounts.
 *   Whatever `gamePath` was at open time is what the user gets —
 *   matching WPF's "value at click time".
 * - The `<el-dialog>` is modal, so the user can't navigate to
 *   Settings to change the path mid-dialog. The "stale prop"
 *   risk is therefore zero.
 *
 * Centralising the resolution here also keeps the two parent
 * pages from each having to re-derive `gamePath` from
 * `game.selectedIni` + `commands.detectGamePath` — a duplication
 * that would re-introduce the WPF `t_GamePath.Text` cross-page
 * coupling we're trying to avoid.
 *
 * # Why `loginRegion` is read here too
 *
 * `MapleTools` consumes `loginRegion` for the WPF-mirrored
 * `MsgPlayerReport` HK advisory (`windows/MapleTools.vue`
 * L130-132). WPF reads `App.LoginRegion` at click time
 * (`MapleTools.xaml.cs` L26). Same "fresh on each open"
 * argument as `gamePath` — the auth store is always available
 * and the modal-blocks-navigation guarantee makes the snapshot
 * safe.
 */

import { ref } from 'vue'
import MapleTools from './MapleTools.vue'
import KartTools from './KartTools.vue'
import WebBrowser from './WebBrowser.vue'
import CoreCalculator from './CoreCalculator.vue'
import EquipCalculator from './EquipCalculator.vue'

import { commands } from '../types/bindings'
import type { LoginRegion } from '../types/bindings'
import { safeInvoke } from '../services/invoke'
import { useAuthStore } from '../stores/auth'
import { useGameStore } from '../stores/game'
import { KART_TOOLS_CODE, MAPLE_TOOLS_CODES } from '../constants/tools'

defineOptions({ name: 'ToolsDialogStack' })

const auth = useAuthStore()
const game = useGameStore()

/* ------------------------------------------------------------------ */
/* MapleTools state                                                    */
/* ------------------------------------------------------------------ */

/**
 * Visibility ref bound to `<MapleTools v-model:visible>`. Set
 * to `true` by {@link openForGame} when the active gameCode is
 * in {@link MAPLE_TOOLS_CODES}; the dialog itself flips it back
 * to `false` on close (X / overlay click / Escape).
 */
const mapleToolsVisible = ref(false)

/**
 * Snapshot of `Config.xml::<dir_value_name>.<gameCode>` resolved
 * at {@link openForGame} time. Empty string means the user has
 * not yet picked a game path; `MapleTools` handles the empty
 * case with the WPF-mirrored `MsgCantFindGame` toast (see
 * `windows/MapleTools.vue` L174-180).
 */
const mapleToolsGamePath = ref('')

/**
 * Snapshot of `auth.session?.region` at {@link openForGame}
 * time. `undefined` matches the WPF `App.LoginRegion == null`
 * pre-login state and skips the HK PlayerReport advisory inside
 * `MapleTools`.
 */
const mapleToolsLoginRegion = ref<LoginRegion | undefined>(undefined)

/* ------------------------------------------------------------------ */
/* KartTools state                                                     */
/* ------------------------------------------------------------------ */

/**
 * Visibility ref bound to `<KartTools v-model:visible>`. Set to
 * `true` by {@link openForGame} when the active gameCode equals
 * {@link KART_TOOLS_CODE}. KartTools needs no extra props —
 * its six hyperlink targets are hard-coded URL constants inside
 * the component (mirrors WPF `KartTools.xaml.cs` static URLs).
 */
const kartToolsVisible = ref(false)

/* ------------------------------------------------------------------ */
/* Shared child dialog state (WebBrowser / EquipCalc / CoreCalc)       */
/* ------------------------------------------------------------------ */

/**
 * `<WebBrowser v-model:visible>` ref — receives `open-web-browser`
 * events from both MapleTools (PlayerReport / VideoReport) and
 * KartTools (six convoy/rider URLs). One shared mount handles
 * both pipelines because only one parent dialog can be open at
 * a time (the `<el-dialog>` modal overlay blocks the other
 * parent's buttons), so the WebBrowser instance can never be
 * needed concurrently.
 */
const webBrowserVisible = ref(false)
const webBrowserUrl = ref('')

/**
 * `<EquipCalculator v-model:visible>` — opened by MapleTools's
 * `open-equip-calculator` event (D5 + D6 wiring).
 */
const equipCalcVisible = ref(false)

/**
 * `<CoreCalculator v-model:visible>` — opened by MapleTools's
 * `open-core-calculator` event (D4 wiring).
 */
const coreCalcVisible = ref(false)

/* ------------------------------------------------------------------ */
/* Public API (defineExpose)                                           */
/* ------------------------------------------------------------------ */

/**
 * Open the Tools dialog matching `gameCode`. Mirrors WPF
 * `AccountList.xaml.cs::btn_Tools_Click` L237-250 verbatim:
 *
 * - `gameCode === '610096_TE'` → opens KartTools.
 * - `gameCode ∈ MAPLE_TOOLS_CODES` → opens MapleTools (after
 *   resolving `gamePath` via `commands.detectGamePath` and
 *   snapshotting `auth.session?.region`).
 * - any other code → no-op (matches WPF's switch falling
 *   through; in practice the parent's `showToolsButton` gate
 *   prevents this branch from being reached, but defensive
 *   no-op keeps the API safe for misuse / future codes added
 *   to the visibility set ahead of routing).
 *
 * @param gameCode — the active `<service_code>_<service_region>`
 *   string from `useGameStore::selectedGameCode`. Caller is
 *   responsible for ensuring it's non-null (the AccountList /
 *   Settings click handler short-circuits on null).
 */
async function openForGame(gameCode: string): Promise<void> {
  /*
   * Order matters: KartTools is checked first so an accidental
   * inclusion of the KartRider code in `MAPLE_TOOLS_CODES`
   * (e.g. typo during a future "add a 4th tools-bearing game"
   * edit) routes to the more-specific KartTools branch instead
   * of silently falling into the MapleTools path. The constants
   * file (`src/constants/tools.ts`) docblock pins this contract
   * and the test suite enforces "no overlap between MAPLE and
   * KART sets".
   */
  if (gameCode === KART_TOOLS_CODE) {
    kartToolsVisible.value = true
    return
  }

  if (MAPLE_TOOLS_CODES.has(gameCode)) {
    mapleToolsGamePath.value = await resolveGamePath(gameCode)
    mapleToolsLoginRegion.value = auth.session?.region ?? undefined
    mapleToolsVisible.value = true
    return
  }

  /*
   * Fallthrough = WPF switch's `default:` (which doesn't exist
   * in the C# source — the switch simply falls off the end and
   * returns). No-op intentionally.
   */
}

/**
 * Resolve `Config.xml::<dir_value_name>.<gameCode>` via the
 * backend `detect_game_path` command — same lookup used by
 * `pages/AccountList.vue::resolveGamePath` for the launch flow
 * (L955-958). Returns empty string when:
 *
 * - The selected game's INI entry is missing or has an empty
 *   `dir_value_name` (the backend can't form a valid Config
 *   key without it). MapleTools then surfaces `MsgCantFindGame`
 *   on the Recycling click — same WPF parity as an empty path.
 * - The backend command fails (rare — usually a Config.xml
 *   parse error). Same fallback.
 *
 * No retry / toast on failure: the dialog still opens with
 * empty `gamePath` and the user gets a clear error on the
 * Recycling action they actually care about. Matches WPF's
 * "open the dialog regardless; surface the error on use"
 * pattern.
 */
async function resolveGamePath(gameCode: string): Promise<string> {
  const ini = game.selectedIni
  if (!ini || ini.dir_value_name === '') return ''
  const result = await safeInvoke(
    commands.detectGamePath(gameCode, ini.dir_value_name, ini.dir_reg),
  )
  return result.ok ? (result.data ?? '') : ''
}

/* ------------------------------------------------------------------ */
/* Child-dialog event handlers                                         */
/* ------------------------------------------------------------------ */

function handleOpenWebBrowser(url: string): void {
  webBrowserUrl.value = url
  webBrowserVisible.value = true
}

function handleOpenEquipCalculator(): void {
  equipCalcVisible.value = true
}

function handleOpenCoreCalculator(): void {
  coreCalcVisible.value = true
}

defineExpose({ openForGame })
</script>

<template>
  <!--
    MapleTools / KartTools / WebBrowser / EquipCalculator /
    CoreCalculator are all `<el-dialog append-to-body>` so the
    parent fragment here is essentially a render-list of dialog
    mounts — they each teleport to `body` independently and the
    actual DOM hierarchy is flat. Sibling order in this template
    therefore controls neither layout nor stacking; it does
    however control the source-order Vue uses for keyed
    reconciliation, so we keep MapleTools first to mirror the
    WPF switch order (`case "610074_T9"` is the first arm).
   -->
  <MapleTools
    v-model:visible="mapleToolsVisible"
    :game-path="mapleToolsGamePath"
    :login-region="mapleToolsLoginRegion"
    @open-web-browser="handleOpenWebBrowser"
    @open-equip-calculator="handleOpenEquipCalculator"
    @open-core-calculator="handleOpenCoreCalculator"
  />
  <KartTools v-model:visible="kartToolsVisible" @open-web-browser="handleOpenWebBrowser" />
  <WebBrowser v-model:visible="webBrowserVisible" :url="webBrowserUrl" />
  <EquipCalculator v-model:visible="equipCalcVisible" />
  <CoreCalculator v-model:visible="coreCalcVisible" />
</template>
