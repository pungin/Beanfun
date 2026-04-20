/**
 * Game store — per-region game catalogue (INI metadata + service
 * list) plus the active selection.
 *
 * # Scope (P12.3 D4)
 *
 * Owns:
 *
 * - `ini` — `Record<gameCode, GameIniEntry>` from
 *   `get_service_ini.ashx` (executable, login action type, win class,
 *   registry hints; everything the launcher needs to spawn the game
 *   binary).
 * - `services` — ordered `GameService[]` from `game_zone/`'s
 *   `Services.ServiceList` literal (display-side metadata: name,
 *   image file names, official site URL).
 * - `selectedGameCode` — the active `<service_code>_<service_region>`
 *   pair (e.g. `"610074_T9"`). Mirrors WPF
 *   `MainWindow.service_code + "_" + service_region`.
 * - `loadState` / `loadError` — 4-state load machine (`idle`,
 *   `loading`, `loaded`, `error`) that lets `GameList.vue` render a
 *   loading shimmer / inline error banner / loaded grid without each
 *   consumer re-implementing the booleans.
 *
 * # Why one store for both halves
 *
 * `ini` and `services` come from a single
 * [`commands.listGames`][cmd] round-trip (atomic in WPF
 * `MainWindow.reLoadGameInfo` — see backend module docs for
 * rationale). Splitting them into two stores would invite mid-load
 * races where the UI sees `services.value.length > 0` but
 * `ini.value` is still empty (or vice versa) — exactly the
 * inconsistent-intermediate-state bug WPF avoids.
 *
 * # Caching policy
 *
 * Per-session cache: {@link loadGames} short-circuits when
 * `loadState === 'loaded'` unless `force === true`. The store has no
 * region-keyed dictionary (unlike WPF's `GameList[region]`) because
 * the active session pins the region — re-entering the page after a
 * region switch goes through {@link clearGameData} (auth store
 * logout / session-expired) so the next `loadGames` runs fresh.
 *
 * # Cross-store wiring
 *
 * - `clearGameData` is composed into `main.ts`'s
 *   `installRouterGuards.clearAccountSession` callback **alongside**
 *   `account.clearSessionData()`, so the session-expired bridge
 *   wipes every session-scoped store in one shot. SRP: this file
 *   exposes the wipe; composition stays in `main.ts`.
 * - The store **never** imports `useAuthStore` — region passes in
 *   via the {@link imageUrl} / {@link selectGame} parameters from
 *   the calling component. Caller (`AccountList.vue`) already has
 *   `auth.session.region` in scope so the parameter is free, and
 *   keeping the store auth-unaware avoids the circular import that
 *   bit `auth.ts → invoke.ts → ...` patterns would suffer.
 *
 * [cmd]: ../types/bindings.ts (search for `listGames`)
 */

import { computed, ref } from 'vue'
import { defineStore } from 'pinia'

import { commands } from '../types/bindings'
import type { GameIniEntry, GameInfoBundle, GameService, LoginRegion } from '../types/bindings'
import { CommandInvocationError, safeInvoke, surfaceCommandError } from '../services/invoke'

/**
 * `<service_code>_<service_region>` pairs WPF treats as
 * "unconnected" games (no Beanfun-managed in-game wallet — the
 * service has its own per-account credential separate from the
 * Beanfun login).
 *
 * Mirrors `MainWindow.xaml.cs::selectedGameChanged` L646-655 — the
 * sole place WPF flips `MainWindow.UnconnectedGame` between `true`
 * and `false`. The list is hard-coded in WPF (no server-side
 * configuration), so we mirror it as a frozen literal here.
 *
 * Frontend consumers branch on this through
 * {@link useGameStore.isUnconnectedGame}: when `true`,
 * `AccountList.vue` swaps in the unconnected-game add-account /
 * change-password dialogs (P12.3 D6 / D7) instead of the
 * regular-game `AddServiceAccount.vue` and shows a
 * change-password row affordance the regular flow doesn't have.
 *
 * Adding a new unconnected game is a one-line change here —
 * grep-replace from WPF lands cleanly because the constant uses
 * the same `<code>_<region>` shape.
 */
export const UNCONNECTED_GAME_CODES: ReadonlySet<string> = new Set(['610153_TN', '610085_TC'])

/**
 * Build the `<service_code>_<service_region>` key WPF uses to index
 * into `INIData` and to test against {@link UNCONNECTED_GAME_CODES}.
 *
 * Pure helper — no store state read — so it's exported separately
 * for use in unit tests and in `AccountList.vue` mount-time
 * resolution (where the page already has `service_code` /
 * `service_region` from the session and just needs the joined
 * string to look up INI).
 */
export function gameCodeOf(serviceCode: string, serviceRegion: string): string {
  return `${serviceCode}_${serviceRegion}`
}

/**
 * Build the `<img src>` URL for a game banner image, mirroring
 * WPF `MainWindow.xaml.cs::GameService.loadImage` (L494-510)
 * branch:
 *
 * ```csharp
 * if (!url.StartsWith("http://", ...) && !url.StartsWith("https://", ...))
 *     url = $"{imageBaseUrl}{url}";
 * byte[] buffer = new WebClient().DownloadData(url);
 * ```
 *
 * `name` is whatever string came back as
 * `Service{XLarge,Large,Small}ImageName` from the upstream
 * `Services.ServiceList` JSON literal. Two historical shapes
 * coexist in the wild:
 *
 * - **Full URL** — `https://images.beanfun.com/GameZone/<id>.jpg`
 *   etc. As of the P12.4-followup-B-fix F3-redo audit (2026-04),
 *   every one of the 12 services in the live TW
 *   `Services.ServiceList` returns this shape.
 * - **Bare filename** — historical (`<service_code>_l.jpg` /
 *   `<timestamp>.jpg`). Not observed in production today; WPF
 *   carried the `imageBaseUrl` prefix branch as a defensive
 *   fallback that beanfun-next mirrors here.
 *
 * The branch order matters: passthrough first so a future
 * upstream that mixes the two shapes per row still works
 * without tripping the prefix wrap.
 *
 * # Why the WebView can fetch directly (no IPC proxy)
 *
 * Unlike WPF (which used `WebClient.DownloadData` from the C#
 * side then fed the bytes into a `BitmapImage` memory stream),
 * the Tauri WebView issues `<img>` requests directly. The new
 * `images.beanfun.com` host accepts cross-origin `<img>` fetches
 * with the `tauri://localhost` referer (probed during the
 * F3-redo investigation: `200 OK` with the real JPEG body, no
 * hotlink rejection). Pushing the load to the WebView keeps the
 * backend pure-data and avoids a `Vec<u8>` round-trip per tile.
 *
 * # F3-redo audit trail (P12.4-followup-B-fix)
 *
 * Earlier F3 attempts switched the base host between
 * `tw.images.beanfun.com` (the WPF original — since retired by
 * Beanfun, returns `ERR_CONNECTION_TIMED_OUT`) and
 * `tw.beanfun.com/uploaded_images/.../game_zone/`. Both were
 * wrong: every live `Service*ImageName` is already a full URL,
 * so the prefix wrap produced nonsense like
 * `https://tw.beanfun.com/uploaded_images/.../https://images.beanfun.com/...`,
 * which the lenient server returned as `200 + 0 byte`.
 * Replicating WPF's L494 branch is the actual fix.
 *
 * # Edge cases
 *
 * - `name === ''` returns the bare base URL — the resulting
 *   `<img>` would 404, but the caller (template `:src` binding)
 *   typically gates the render on `name.length > 0` already.
 *   Surfacing the empty path rather than `null` keeps the
 *   return type a plain `string` so templates don't have to
 *   handle two shapes.
 * - The `region` parameter is preserved on the API even though
 *   today's base is the same for TW and HK — keeps the call
 *   sites stable if Beanfun ever resplits hosts in the future.
 */
export function imageUrl(name: string, region: LoginRegion): string {
  if (name.startsWith('http://') || name.startsWith('https://')) {
    return name
  }
  /*
   * Region is currently homogeneous (`images.beanfun.com` serves
   * both TW and HK in 2026) but the parameter is preserved so a
   * future Beanfun-side host re-split lands as a one-line change
   * here without forcing every call site to drop / re-add the
   * argument.
   */
  void region
  const base = 'https://images.beanfun.com/GameZone/'
  return `${base}${name}`
}

/**
 * 4-state load machine. Mirrors the same pattern
 * `pages/ManageAccount.vue` (P12.2 D9) uses for its data load —
 * exposing the state lets templates render exactly one of the four
 * branches without juggling `loading || error || empty`-style
 * boolean compounds.
 *
 * - `idle` — initial, before the first `loadGames` call.
 * - `loading` — IPC in flight.
 * - `loaded` — `services` and `ini` populated (may still be empty
 *   arrays / objects if the server returned an empty catalogue,
 *   which is a legitimate "no games" state distinct from `error`).
 * - `error` — `loadError` is non-`null`. Caller renders the retry
 *   banner.
 */
export type GameLoadState = 'idle' | 'loading' | 'loaded' | 'error'

export const useGameStore = defineStore('game', () => {
  const ini = ref<Record<string, GameIniEntry>>({})
  const services = ref<GameService[]>([])
  const selectedGameCode = ref<string | null>(null)
  const loadState = ref<GameLoadState>('idle')
  const loadError = ref<string | null>(null)

  /**
   * Read the [`GameService`] for the current selection, or `null`
   * when nothing is selected / the selection is not in the
   * fetched catalogue.
   *
   * Returning `null` (rather than throwing) lets templates
   * `v-if="game.selectedGame"` directly. Mirrors WPF's
   * `MainWindow.SelectedGame` field — null until the first
   * `selectedGameChanged()` populates it from the catalogue
   * loop (`MainWindow.xaml.cs` L661-674).
   */
  const selectedGame = computed<GameService | null>(() => {
    if (selectedGameCode.value === null) return null
    const target = selectedGameCode.value
    return (
      services.value.find((s) => gameCodeOf(s.service_code, s.service_region) === target) ?? null
    )
  })

  /**
   * Read the [`GameIniEntry`] for the current selection, or `null`
   * when nothing is selected / the selection has no INI section.
   *
   * Mirrors WPF's `INIData[gameCode]` access pattern — WPF returns
   * an empty `KeyDataCollection` for missing sections (every field
   * read yields `""`), the Rust port returns `None` so the caller
   * can branch explicitly. The frontend mostly cares about
   * `selectedIni?.win_class_name` (auto-paste enable) and
   * `selectedIni?.exe.length > 0` (launchable gate).
   */
  const selectedIni = computed<GameIniEntry | null>(() => {
    if (selectedGameCode.value === null) return null
    return ini.value[selectedGameCode.value] ?? null
  })

  /**
   * `true` when the active selection is one of the
   * {@link UNCONNECTED_GAME_CODES}. Drives `AccountList.vue`'s
   * dialog routing (regular vs. unconnected `AddServiceAccount`)
   * and the change-password row affordance visibility.
   *
   * Mirrors WPF `MainWindow.UnconnectedGame` (L91, set inside
   * `selectedGameChanged` L646-655). Computed off
   * {@link selectedGameCode} so the boolean is always in sync with
   * the active selection — no separate `boolean` ref to keep
   * consistent (DRY).
   */
  const isUnconnectedGame = computed<boolean>(() => {
    if (selectedGameCode.value === null) return false
    return UNCONNECTED_GAME_CODES.has(selectedGameCode.value)
  })

  /**
   * Apply a successful [`commands.listGames`][cmd] response to the
   * store. Single-write authority — every caller (the public
   * action plus tests) goes through this helper so future field
   * additions to `GameInfoBundle` land in one place (DRY).
   *
   * [cmd]: ../types/bindings.ts
   */
  function applyBundle(bundle: GameInfoBundle): void {
    ini.value = { ...bundle.ini } as Record<string, GameIniEntry>
    services.value = bundle.services
    loadState.value = 'loaded'
    loadError.value = null
  }

  /**
   * Fetch the per-region game catalogue.
   *
   * # Caching
   *
   * Idempotent unless `force === true`. Once `loadState === 'loaded'`,
   * subsequent calls return the cached state without firing a
   * second IPC round-trip. The cache is wiped by
   * {@link clearGameData} (logout / session-expired bridge), so the
   * next post-login `loadGames` always runs fresh.
   *
   * # Concurrency
   *
   * If `loadState === 'loading'` (a previous call is still in
   * flight), a second call short-circuits to a no-op `Promise<void>`
   * — there's only ever one `commands.listGames()` request live at
   * once. WPF makes the same single-flight assumption implicitly
   * via UI thread serialisation; we make it explicit here.
   *
   * # Error handling
   *
   * Failures are surfaced two ways:
   *
   * 1. {@link loadError} carries the message string for
   *    `GameList.vue` / the AccountList game-info bar's inline
   *    "Retry" banner.
   * 2. {@link surfaceCommandError} fires the standard toast for
   *    the rest of the UI (matches every other store action).
   *
   * The action **does not throw** — callers (mount hooks, retry
   * buttons) treat the post-call `loadState === 'loaded'` /
   * `'error'` as the success signal. Throwing would force every
   * call site into `try { } catch { }`, which buys nothing on top
   * of the toast + inline banner combo.
   */
  async function loadGames(force = false): Promise<void> {
    if (!force && loadState.value === 'loaded') return
    if (loadState.value === 'loading') return

    loadState.value = 'loading'
    loadError.value = null

    const result = await safeInvoke(commands.listGames())
    if (result.ok) {
      applyBundle(result.data)
      return
    }
    loadError.value = result.error.message
    loadState.value = 'error'
    surfaceCommandError(result.error)
    /*
     * Do not throw — see "Error handling" in the docblock above. The
     * caller is expected to inspect `loadState` / `loadError` after
     * `await`. The `CommandInvocationError` import below is kept
     * because tests sometimes assert that the store *did not* leak
     * the error as a thrown exception, and grepping for the type
     * keeps that intent explicit.
     */
    void CommandInvocationError
  }

  /**
   * Set the active game by `(service_code, service_region)` pair.
   *
   * Mirrors `MainWindow.xaml.cs::GameList.SelectionChanged` (the
   * `Windows/GameList.xaml.cs` handler that runs when the user
   * picks a game from the list dialog) — WPF immediately writes
   * `App.MainWnd.service_code` / `service_region` and calls
   * `selectedGameChanged()` to re-render the AccountList shell. We
   * keep the store side minimal (just the joined gameCode) and
   * leave the post-selection refresh (account list reload, game
   * banner image swap) to the caller — `AccountList.vue` watches
   * `selectedGameCode` and orchestrates the rest.
   */
  function selectGame(serviceCode: string, serviceRegion: string): void {
    selectedGameCode.value = gameCodeOf(serviceCode, serviceRegion)
  }

  /**
   * Re-hydrate `selectedGameCode` + the matching `ini` entry from
   * the persisted Config.xml snapshot written by
   * `pages/AccountList.vue::selectActiveGame` (P12.4 followup-A
   * D5).
   *
   * # When this is called
   *
   * `pages/IdPassForm.vue` / `pages/QrForm.vue` GameStart
   * buttons (WPF `btn_StartGame_Click`, `id-pass_form.xaml.cs`
   * L297-300 + `qr_form.xaml.cs` L84-87) defer to
   * `useGameLauncher().runGame()`, which requires
   * `selectedGameCode` + `selectedIni` to be populated. On the
   * LoginPage the user hasn't authenticated, so:
   *
   * - `loadGames()` never ran — `services` and `ini` are both
   *   empty {} / [].
   * - `selectGame()` was never called — `selectedGameCode` is null.
   *
   * `restoreLastSelected()` patches just enough of the store to
   * make a launch attempt possible: copy the persisted
   * gameCode + INI entry into the in-memory state without
   * triggering the (auth-required) catalogue fetch. `services`
   * stays empty — the launcher only needs `selectedIni` (for
   * `exe` / `dir_value_name` / `dir_reg`), not `selectedGame`,
   * for the path-detect + spawn flow. The single
   * `selectedGame`-dependent branch (`download_url` fallback in
   * `resolveGamePath`) gracefully degrades to the same
   * `gamePathPickerPending` toast since `selectedGame` will be
   * `null` (the catalogue isn't loaded), which is acceptable
   * fallback semantics — the user can re-login + select game +
   * try again.
   *
   * # Why a Pinia action and not a composable
   *
   * The state being mutated (`selectedGameCode` + the `ini` map)
   * is store-owned. Putting the patch inside the store keeps the
   * mutation surface single-source — every other place that
   * touches these refs (`selectGame`, `applyBundle`,
   * `clearGameData`) already lives here, so a follow-up
   * developer reading the store has a complete picture.
   *
   * # Mirrors WPF instance state lifetime
   *
   * WPF `MainWindow.service_code` / `service_region` /
   * `game_exe` / `game_commandLine` / `game.dir_value_name` /
   * `game.dir_reg` survive logout (only reset on full process
   * restart) — `MainWindow.runGame()` running on the LoginPage
   * after logout works because those fields are still
   * populated. The SPA's `clearGameData()` reset on logout is
   * stricter than WPF; this restore action narrows the gap by
   * letting the launch-relevant subset survive across the
   * Config.xml round-trip.
   *
   * # Return value
   *
   * `true` when both keys resolve to non-empty / parseable
   * snapshots; `false` otherwise (no `loginGame`, no
   * `lastSelectedIni`, JSON parse failure, etc.). The launcher
   * caller surfaces the same `GameSelected` toast WPF shows
   * when `service_code` is empty.
   *
   * # Idempotent / safe to call from a populated state
   *
   * If `selectedGameCode` is already set (post-login session
   * with the catalogue loaded), this action is a no-op short
   * of the IPC round-trip — the persisted snapshot would
   * either match (no-op write) or be stale (we still prefer
   * the in-memory live data). Returns `true` in that case to
   * unblock the launcher fast-path.
   *
   * @param configStore — caller-injected `useConfigStore()` instance
   *                      so this action stays test-friendly without
   *                      depending on the global Pinia activation
   *                      order at module-load time.
   */
  function restoreLastSelected(configStore: { get: (key: string) => string | undefined }): boolean {
    if (selectedGameCode.value !== null && selectedIni.value !== null) {
      return true
    }

    const savedCode = configStore.get('loginGame') ?? ''
    const savedIniRaw = configStore.get('lastSelectedIni') ?? ''
    if (savedCode === '' || savedIniRaw === '') return false

    let parsedIni: GameIniEntry
    try {
      parsedIni = JSON.parse(savedIniRaw) as GameIniEntry
    } catch {
      /*
       * Corrupted snapshot (manual Config.xml edit / partial
       * write on previous crash). Soft-fail to false so the
       * caller surfaces the same `GameSelected` toast as the
       * "no snapshot" path — the user can re-login + select
       * game + the next `selectActiveGame` will overwrite the
       * bad row.
       */
      return false
    }

    /*
     * Patch the entry into the existing `ini` map (don't
     * replace the map — a concurrent `loadGames` finishing
     * after this restore would otherwise lose its bundle).
     * The new entry is the only one we care about because
     * `selectedIni` is keyed off `selectedGameCode`.
     */
    ini.value = { ...ini.value, [savedCode]: parsedIni }
    selectedGameCode.value = savedCode
    return true
  }

  /**
   * Wipe every piece of game-scoped state. Composed into
   * `main.ts::installRouterGuards.clearAccountSession` so the
   * session-expired bridge clears the catalogue alongside the
   * account store (P12.3 D4 — without this, a re-login briefly
   * flashes the previous user's game grid before the new
   * `loadGames()` runs).
   *
   * Resets `loadState` to `'idle'` (not `'loaded'`) so the next
   * `loadGames()` is treated as a fresh fetch by the caching
   * branch.
   *
   * # P12.4 followup-A D6 note
   *
   * `clearGameData` only wipes the in-memory store; the
   * persisted `loginGame` / `lastSelectedIni` Config.xml keys
   * are intentionally left intact so
   * {@link restoreLastSelected} can re-hydrate the launch
   * subset on the LoginPage after logout. Mirrors WPF
   * MainWindow instance state — survives logout, only reset on
   * full process restart.
   */
  function clearGameData(): void {
    ini.value = {}
    services.value = []
    selectedGameCode.value = null
    loadState.value = 'idle'
    loadError.value = null
  }

  return {
    ini,
    services,
    selectedGameCode,
    loadState,
    loadError,

    selectedGame,
    selectedIni,
    isUnconnectedGame,

    loadGames,
    selectGame,
    restoreLastSelected,
    clearGameData,
  }
})
