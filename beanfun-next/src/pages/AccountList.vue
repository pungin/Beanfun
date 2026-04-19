<script setup lang="ts">
/**
 * Account list — the post-login landing page (P12.2 D1).
 *
 * # WPF parity
 *
 * Combines the central area of WPF `MainWindow.xaml` (game info bar +
 * Gash balance + OTP panel) with `AccountList.xaml` (the
 * `lstViewAccount` ItemsControl). The WPF original split this across
 * two XAML files because `MainWindow` owns chrome (game switcher,
 * Logout button, member-center / customer-service links) while
 * `AccountList` owns just the per-account ItemTemplate; the SPA
 * collapses both into one route component because the `LoginPage`
 * shell pattern (D1: a parent shell with `<RouterView />` for sub-
 * forms) doesn't apply here — there are no `/accounts/*` sub-routes
 * yet, and the chrome belongs to the page, not a global shell (a
 * future `MainShell` for P12.3+ would re-host the chrome if other
 * top-level pages need to share it).
 *
 * # D1 scope (what's REAL)
 *
 * - Page chrome (header + sections) using design-system utilities
 *   from `src/styles/utilities.css` so DRY against `LoginPage.vue`
 * - Service-account list with **four** rendered states:
 *     1. `loading` — first paint while {@link useAccountStore.getServiceAccounts}
 *        is in-flight
 *     2. `error` — load failed; inline banner + Retry button
 *     3. `empty` — load succeeded with `accounts.length === 0`
 *     4. `ready` — populated rows with click-to-select highlight
 * - Account row click → set `account.selectedSid` (the OTP / Start
 *   Game flow downstream reads this from the store)
 * - Logout button → confirm dialog → `auth.logout()` →
 *   `account.clearSessionData()` → `router.push('/login')`
 *
 * # D1 scope (what's STUBBED)
 *
 * Each stub renders the affordance shown in the mockup
 * (`mockups/AccountList.html`) but its handler logs a `TODO P12.X`
 * marker and returns. No fake data, no toasts the user can't act
 * on. Concrete D-step ownership (matching the P12.2 plan in
 * `Todo.md`):
 *
 * | Affordance                                          | Owner D-step |
 * |-----------------------------------------------------|--------------|
 * | Game info bar (icon, name, status, change-game btn) | P12.3 (GameList integration) |
 * | Start Game button                                   | P12.3 (`commands.runGame`) |
 * | Gash balance value + refresh                        | P12.2 D-step (`account.getRemainPoint`) |
 * | Member Center / Customer Service links              | P12.4 (open WebBrowser) |
 * | Tools button (game-specific tools window)           | P12.3 (`MapleTools` / `KartTools` per gameCode + conditional visibility) |
 * | Auto-paste preference                               | **REAL since P12.2 D5** (`useConfigStore` `autoPaste` key, lazy-write + AutoPasteTip on first toggle) |
 * | OTP value + Get OTP + Copy                          | **REAL since P12.2 D5** (`account.getOtp` + `commands.autoPaste` + clipboard fallback) |
 * | Add Service Account button                          | **REAL since P12.2 D3** (`windows/AddServiceAccount.vue`) |
 * | Drag handle (reorder)                               | **REAL since P12.2 D7** (vuedraggable + `Config.xml` `AccountOrder_<gameCode>` persistence) |
 * | Per-row context menu (more_vert)                    | **REAL since P12.2 D4** (`Change Alias` + `Account Info` + `Check Email` items wired; other items land in their own D-steps) |
 *
 * Stubs render the mockup affordance verbatim so visual QA on D1
 * already shows the final layout — only the wiring is deferred.
 *
 * # Why logout orchestration lives here, not in the auth store
 *
 * `auth.logout()` is the *backend* round-trip + auth-state wipe.
 * The post-logout fan-out (clear non-auth session caches, navigate)
 * is page-triggered orchestration that needs scope of the router
 * and every Pinia store — same SRP rationale as the router-guard
 * `clearAccountSession` callback (see `router/index.ts` docblock).
 * If a second page eventually grows a Logout button (Settings page
 * in P12.4), the three-step "confirm + auth.logout + clear + nav"
 * sequence will get extracted to a `useLogoutFlow()` composable.
 * For D1 the page is the only caller, so inlining stays cheaper
 * than abstracting prematurely (YAGNI).
 *
 * # Mockup conflict resolution
 *
 * The Stitch mockup ships a 480px-wide mobile-pattern layout with
 * a fixed top bar (Beanfun Next + help/settings/close) and a fixed
 * bottom Play button. Both are intentionally **omitted** in this
 * port:
 *
 * - Top bar belongs to the future `TitleBar.vue` (P12.4) — every
 *   page would otherwise re-render it; the page-scoped header
 *   here is a per-page heading that complements (not duplicates)
 *   the global title bar.
 * - Bottom Play button is a phone-pattern affordance that
 *   duplicates the Start Game button inside the game info bar —
 *   on a desktop window the user always sees the in-flow Start
 *   Game CTA, so the bottom dock has no UX purpose.
 */

import { computed, onMounted, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useRouter } from 'vue-router'
import {
  ElButton,
  ElCheckbox,
  ElDropdown,
  ElDropdownItem,
  ElDropdownMenu,
  ElIcon,
  ElMessage,
  ElMessageBox,
} from 'element-plus'
import {
  DocumentCopy,
  EditPen,
  InfoFilled,
  Key,
  Message,
  MoreFilled,
  Operation,
  Plus,
  Refresh,
  Service,
  SwitchButton,
  User,
  VideoPlay,
} from '@element-plus/icons-vue'
import draggable from 'vuedraggable'

import { useAuthStore } from '../stores/auth'
import { useAccountStore } from '../stores/account'
import { useConfigStore } from '../stores/config'
import { useGameStore, gameCodeOf, imageUrl } from '../stores/game'
import { commands, type GameStartMode, type ServiceAccount } from '../types/bindings'
import {
  CommandInvocationError,
  safeInvoke,
  surfaceCommandError,
  wrapCommand,
} from '../services/invoke'
import AddServiceAccount from '../windows/AddServiceAccount.vue'
import ChangeServiceAccountDisplayName from '../windows/ChangeServiceAccountDisplayName.vue'
import CopyBox from '../windows/CopyBox.vue'
import GameList from '../windows/GameList.vue'
import ServiceAccountInfo from '../windows/ServiceAccountInfo.vue'
import UnconnectedGameAddAccount from '../windows/UnconnectedGame_AddAccount.vue'
import UnconnectedGameChangePassword from '../windows/UnconnectedGame_ChangePassword.vue'

defineOptions({ name: 'AccountList' })

const { t } = useI18n()
const router = useRouter()
const auth = useAuthStore()
const account = useAccountStore()
/*
 * Aliased to `configStore` to leave room for future template props
 * named `config` (mirrors the D4 `accountStore` rename rationale —
 * `vue/no-dupe-keys` would flag a clash) and to make the intent
 * unambiguous at every read site (this is the K-V config cache,
 * not a generic settings object).
 */
const configStore = useConfigStore()
/*
 * D8 — game store wire-in. Aliased to `game` (matching the
 * `account` / `auth` / `configStore` alias style used above).
 * Required for: per-game catalogue (game info bar name+image),
 * conditional Tools button visibility, isUnconnectedGame branch
 * for Add Account & Change Password, INI lookup for the Start
 * Game pipeline (`exe` → command-line template, `login_action_type`
 * → direct/OTP branch, `dir_value_name`/`dir_reg` → game-path
 * detection).
 */
const game = useGameStore()

/* --------------- service-account list state --------------- */

type LoadState = 'loading' | 'ready' | 'error'

const loadState = ref<LoadState>('loading')
/**
 * Backend-supplied error message kept alongside `loadState` so the
 * inline banner can surface the actual server reason without the
 * user having to re-trigger the auto-dismissed toast that
 * `wrapCommand` already emitted.
 */
const loadError = ref<string | null>(null)

async function loadList(): Promise<void> {
  loadState.value = 'loading'
  loadError.value = null
  try {
    await account.getServiceAccounts()
    /*
     * D7: re-apply the user's saved drag-and-drop order on top of
     * the server-sorted list. Mirrors the WPF call site
     * `BeanfunClient.Account.cs::GetAccounts` L137-139 which calls
     * `AccountList.Current.ApplyAccountOrder()` immediately after
     * the initial `OrderBy(ssn)`. No-op when the user has never
     * reordered for the current gameCode (the action's
     * undefined-csv guard handles it) or when no session is yet
     * hydrated (the load path can race with the route guard on
     * cold mount; deferring to the first refresh is safe — the
     * server-sorted order is the right default).
     */
    const savedKey = accountOrderConfigKey.value
    if (savedKey !== null) {
      account.applyServiceAccountOrderFromSavedCsv(configStore.get(savedKey))
    }
    loadState.value = 'ready'
  } catch (err) {
    /*
     * `account.getServiceAccounts` funnels through `wrapCommand`,
     * so any non-`auth.session_required` failure already produced
     * a toast + console log. Mirror the message into the inline
     * banner so the page state is self-explanatory after the
     * toast auto-dismisses, and so a Retry click has visible
     * context for what the user is retrying.
     */
    loadState.value = 'error'
    if (err instanceof CommandInvocationError) {
      loadError.value = err.cause.message
    } else {
      loadError.value = t('accountList.loadFailed')
    }
  }
}

onMounted(() => {
  /*
   * D8 — defer the initial `loadList()` until `setupGameOnMount`
   * has resolved which game to make active. Without the deferral,
   * a saved `loginGame` that differs from the post-login session
   * default would trigger TWO consecutive `getServiceAccounts`
   * round-trips (one for the default, one after `setActiveService`
   * swaps the session pair) — the user would see the spinner
   * twice and a brief flash of the wrong game's accounts. The
   * deferred path always lands on a single fetch:
   *
   * - saved loginGame valid       → setupGameOnMount → selectActiveGame → loadList
   * - saved loginGame invalid     → setupGameOnMount → loadList + open picker
   * - catalogue load fails        → setupGameOnMount → loadList (with session defaults)
   *
   * Mirrors WPF login + `MainWnd_Loaded` + `selectedGameChanged()`
   * single-`GetAccounts` cadence (`MainWindow.xaml.cs` L520-540 +
   * L661-674) — WPF doesn't double-fetch either.
   */
  void setupGameOnMount()
  /*
   * D11: lazy auto-fetch the Gash balance once on mount so the
   * displayed value matches WPF UX (login → AccountList paint →
   * `bfClient.remainPoint` already populated). The cache-aware
   * `force = false` overload short-circuits if a previous mount
   * (or a manual refresh from the same session) already fetched.
   * Errors are intentionally swallowed here — `wrapCommand`
   * already toasted + logged, and surfacing the failure to the
   * page-level error banner would conflate "balance fetch failed"
   * (recoverable, retry button visible) with "account list fetch
   * failed" (blocks the entire page).
   */
  void account.getRemainPoint().catch(() => {})
})

const serviceAccounts = computed(() => account.serviceAccounts)
const accountCount = computed(() => serviceAccounts.value.length)

function isSelected(a: ServiceAccount): boolean {
  return account.selectedSid === a.sid
}

function selectRow(a: ServiceAccount): void {
  /*
   * WPF `lstViewAccount_SelectionChanged` ignores disabled rows
   * (the row is rendered greyed-out and clicks fall through to
   * the parent ListView with no state mutation). Mirror that here
   * so users can't accidentally arm a banned account for the OTP
   * / Start Game flow.
   */
  if (!a.is_enable) return
  account.selectedSid = a.sid
}

/* --------------- logout --------------- */

async function handleLogout(): Promise<void> {
  /*
   * Mirror WPF `MainWindow.Logout_Click`: confirm → backend
   * logout → wipe auth + account state → nav back to the region
   * picker. The account-store wipe stays explicit here (instead
   * of being chained inside `auth.logout()`) for the SRP reason
   * called out in the router-guard `clearAccountSession` docblock
   * — the auth store doesn't know which non-auth Pinia stores
   * back session-scoped state.
   */
  try {
    await ElMessageBox.confirm(t('LogoutConfirm'), t('Logout'), {
      confirmButtonText: t('Logout'),
      cancelButtonText: t('Cancel'),
      type: 'warning',
    })
  } catch {
    return
  }

  try {
    await auth.logout()
  } catch {
    /*
     * `wrapCommand` already toasted the error. Continue with the
     * local clear + nav anyway — if the backend logout failed,
     * the cookie is in an unknown state, and forcing the SPA
     * back to the login page is strictly safer than leaving the
     * user on a half-authenticated AccountList screen.
     */
  }
  account.clearSessionData()
  await router.push('/login')
}

/* --------------- stubs (each owned by a future D-step) --------------- */

/**
 * Build a no-op handler that logs a stable `TODO` marker. Centralised
 * so QA can grep `[AccountList]` in the dev console to spot every
 * un-wired affordance, and so future contributors don't reinvent
 * three different stub conventions across this file.
 */
function makeStub(label: string): () => void {
  return () => {
    console.warn(`[AccountList] ${label} — handler pending real D-step.`)
  }
}

/*
 * P12.3 D8: `handleStartGame`, `handleChangeGame`, `handleAddAccount`,
 * and `handleChangePassword` are the **real** wire-ups for the game
 * switcher + start-game + add-account + change-password flows
 * (defined further down in the file). The Tools button's actual
 * click handler still goes through this stub — the per-game tools
 * windows (`MapleTools` for MapleStory codes, `KartTools` for
 * KartRider) are P12.4 scope; P12.3 D8e only ships the conditional
 * **visibility** of the button (see `showToolsButton` below).
 */
const handleTools = makeStub('Tools button (game-specific tools window)')
const handleMemberCenter = makeStub('Member Center link')
const handleCustomerService = makeStub('Customer Service link')

/* --------------- D8c/D8d/D8e — game info bar + Tools visibility --------------- */

/**
 * Display name shown on the game info bar. Falls back to the
 * placeholder text when no game is selected (initial paint, while
 * `setupGameOnMount` is hydrating the catalogue, or after the user
 * dismisses the picker without choosing). Mirrors WPF
 * `MainWindow.gameName.Content` (L662) which writes
 * `selectedGame.name` after every `selectedGameChanged()`.
 */
const gameNameDisplay = computed<string>(() => {
  return game.selectedGame?.name ?? t('accountList.gamePlaceholder')
})

/**
 * Region-aware banner image URL for the current selection.
 *
 * Returns an empty string when no game is selected so the template
 * `v-if="gameImageUrl"` gate hides the `<img>` cleanly without
 * emitting a 404 to the WebView console. WPF used the small image
 * variant (`small_image_name`) on the AccountList header bar (per
 * the D8 mockup parity reference); we keep that field choice — the
 * "large" / "xlarge" variants are reserved for the GameList grid /
 * LoginPage hero respectively.
 */
const gameImageUrl = computed<string>(() => {
  const selected = game.selectedGame
  if (!selected) return ''
  const region = auth.session?.region
  if (!region) return ''
  return imageUrl(selected.small_image_name, region)
})

/**
 * D8e — Tools button conditional visibility.
 *
 * WPF `MainWindow.xaml.cs` L1710-1713 toggles `btn_Tools.Visibility`
 * on every `selectedGameChanged()`:
 *
 * ```cs
 * accountList.btn_Tools.Visibility =
 *   (gameCode == "610074_T9" || gameCode == "610075_T9" || gameCode == "610096_TE")
 *     ? Visibility.Visible : Visibility.Collapsed;
 * ```
 *
 * The set is hard-coded in WPF (no server-side configuration), so
 * we mirror it as a frozen literal here. Adding a new tools-bearing
 * game is a one-line edit.
 *
 * # Why a Set (not Array.includes)
 *
 * O(1) membership check vs. O(n) for `[...].includes(...)` — the
 * size is small enough that the perf delta is irrelevant, but the
 * `Set` form better signals intent ("this is a membership test, not
 * an ordered collection") and matches the {@link UNCONNECTED_GAME_CODES}
 * convention from `stores/game.ts`.
 */
const TOOLS_GAME_CODES: ReadonlySet<string> = new Set(['610074_T9', '610075_T9', '610096_TE'])

const showToolsButton = computed<boolean>(() => {
  if (game.selectedGameCode === null) return false
  return TOOLS_GAME_CODES.has(game.selectedGameCode)
})

/* --------------- D8c — game switcher + active-service pipeline --------------- */

/**
 * `<GameList />` modal visibility. Driven by either:
 *
 * - The Change Game button on the game info bar (user-initiated).
 * - The mount-time auto-open path inside {@link setupGameOnMount}
 *   when no valid `loginGame` config entry resolves against the
 *   loaded catalogue.
 */
const gameListVisible = ref(false)

function handleChangeGame(): void {
  gameListVisible.value = true
}

/**
 * Canonical "switch the active game" pipeline. Mirrors WPF
 * `MainWindow.xaml.cs::selectedGameChanged()` (L661-680) end-to-end:
 *
 * | Step                                | WPF site                                   |
 * |-------------------------------------|--------------------------------------------|
 * | Update frontend `selectedGameCode`  | `MainWindow.service_code/region` writes    |
 * | Persist `loginGame` to Config.xml   | `ConfigAppSettings.SetValue("loginGame",…)` (L661) |
 * | Sync backend session pair           | (no equivalent — WPF mutates the field directly) |
 * | Clear stale account selection       | `redrawSAccountList` → `SelectedIndex = -1` |
 * | Re-fetch account list               | `bfClient.GetAccounts(code, region)` (L638) |
 *
 * # Why `setActiveService` is gated on `sameAsSession`
 *
 * On the cold-mount path (`setupGameOnMount` resolves saved
 * `loginGame` to the same pair already on the session), the
 * backend session is already correct and a `setActiveService`
 * round-trip would be a wasted IPC. Skipping it on the equality
 * branch is purely an optimisation — the swap is idempotent on the
 * backend side, so a stray call would not corrupt state.
 *
 * # Why `loadList()` runs unconditionally when `refresh === true`
 *
 * Even when the session pair is unchanged, the caller usually
 * wants to re-paint the account list (e.g. cold mount needs the
 * first paint regardless of swap). Callers that don't need the
 * refresh (none in the current code base, but reserved for future
 * background sync paths) can pass `refresh = false`.
 *
 * # Persisted Config.xml is best-effort
 *
 * `configStore.set` toasts on failure via `wrapCommand`; we don't
 * re-catch here because a Config.xml write failure is non-fatal —
 * the in-memory selection is still correct, and the next
 * successful write will reconcile. Mirrors WPF L661 `SetValue`
 * which silently swallows IO errors.
 */
async function selectActiveGame(
  serviceCode: string,
  serviceRegion: string,
  refresh: boolean,
): Promise<void> {
  game.selectGame(serviceCode, serviceRegion)
  /*
   * Persist before any async work so a subsequent crash / window
   * close still restores the user's last choice on next boot —
   * matches WPF's "set first, then act" ordering at L661.
   */
  await configStore.set('loginGame', gameCodeOf(serviceCode, serviceRegion))

  const session = auth.session
  const sameAsSession =
    session !== null &&
    session.service_code === serviceCode &&
    session.service_region === serviceRegion

  if (!sameAsSession) {
    try {
      await wrapCommand(commands.setActiveService(serviceCode, serviceRegion))
    } catch {
      /*
       * `wrapCommand` already toasted the structured error. Bail
       * without refreshing — running `loadList()` against the old
       * backend session would paint the previous game's accounts,
       * which is a worse UX than leaving the list empty.
       */
      return
    }
  }

  /*
   * Clear stale per-account selection so the OTP / Start Game
   * affordances don't carry over a sid that is unlikely to be
   * present in the new game's account list. Mirrors WPF
   * `redrawSAccountList`'s `SelectedIndex = -1` reset.
   */
  account.selectedSid = null

  if (refresh) {
    await loadList()
  }
}

/**
 * `<GameList @select>` handler. The dialog only emits this event
 * for *different* picks (mirrors WPF's `if (service_code != ... ||
 * service_region != ...)` early-exit), so this path always runs
 * the full `selectActiveGame` pipeline including the backend swap
 * and account-list refresh.
 */
function handleGameSelected(serviceCode: string, serviceRegion: string): void {
  void selectActiveGame(serviceCode, serviceRegion, true)
}

/**
 * Mount-time game selection bootstrap. Composed into `onMounted`
 * above; runs once per AccountList mount.
 *
 * # Sequence
 *
 * 1. Load the per-region game catalogue. Idempotent — the store
 *    short-circuits if a previous mount in the same session
 *    already populated it.
 * 2. If the catalogue load failed (`game.loadState === 'error'`),
 *    fall back to loading the account list with whatever the
 *    backend session was seeded with at login time. The
 *    `gameStore.loadGames` action already toasted the error, so
 *    the user sees a structured complaint without the page
 *    bricking.
 * 3. Resolve `loginGame` from Config.xml. WPF stores it as
 *    `<service_code>_<service_region>` (`MainWindow.xaml.cs`
 *    L520-540 / L661); we use {@link gameCodeOf} on the way in
 *    and `lastIndexOf('_')` on the way out so a future game whose
 *    code contains an underscore (none exist today) doesn't
 *    silently round-trip incorrectly.
 * 4. If `loginGame` resolves to a game in the loaded catalogue,
 *    delegate to {@link selectActiveGame} (which handles the
 *    backend swap + account refresh).
 * 5. Otherwise (no saved value, malformed value, or value not in
 *    the loaded catalogue), open the picker dialog AND kick off a
 *    default-session `loadList()` so the user sees the empty /
 *    default-game account list while the picker is open. Mirrors
 *    WPF's `selectedGameChanged()` falling through to the default
 *    item when no `loginGame` matches.
 */
async function setupGameOnMount(): Promise<void> {
  await game.loadGames()

  if (game.loadState === 'error') {
    void loadList()
    return
  }

  const saved = configStore.get('loginGame') ?? ''
  const session = auth.session

  if (saved && session) {
    const sep = saved.lastIndexOf('_')
    if (sep > 0) {
      const code = saved.substring(0, sep)
      const region = saved.substring(sep + 1)
      const found = game.services.find(
        (s) => s.service_code === code && s.service_region === region,
      )
      if (found) {
        await selectActiveGame(code, region, true)
        return
      }
    }
  }

  /*
   * Picker auto-open + default-session account paint. The two run
   * in parallel because they're independent — neither depends on
   * the other completing.
   */
  void loadList()
  gameListVisible.value = true
}

/* --------------- Gash balance refresh (D11) --------------- */

/**
 * # D11 — Gash balance value + refresh
 *
 * Mirrors WPF `Pages/AccountList.xaml.cs::m_UpdatePoint_Click`
 * (L137-140) which calls
 * `App.MainWnd.updateRemainPoint(App.MainWnd.bfClient.getRemainPoint())`
 * — a force-refresh per click (no caching) — and
 * `MainWindow.xaml.cs::updateRemainPoint` (L1716-1721) which formats
 * the displayed string as:
 *
 *   GashRemain.format(remainPoint + (TW || remainPoint == 0 ? "" : GashRemainInGame.format(floor(remainPoint / 2.5))))
 *
 * # Parity decisions
 *
 * - **Force-refresh on click**: WPF re-IPCs every time. We pass
 *   `force = true` to `account.getRemainPoint` so the cached value
 *   in the store is bypassed (the store cache exists for other
 *   unrelated lazy lookups; the manual refresh button must always
 *   round-trip).
 *
 * - **Auto-fetch on mount**: WPF login (`BeanfunClient.Login.cs`
 *   L177 / 829 / 879) populates `bfClient.remainPoint` as part of
 *   the post-login sequence, so the WPF user sees the value
 *   immediately on `AccountList` paint. The SPA login flow (P11)
 *   does not pre-fetch — to keep WPF parity at the user-facing
 *   layer (not the IPC layer), we lazy-fetch once on mount via
 *   `account.getRemainPoint()` (no `force`). Failures are silently
 *   swallowed at the call site here because `wrapCommand` already
 *   toasted + console-logged the structured cause; throwing would
 *   only break the unrelated `loadList()` initialisation, and the
 *   user can retry via the visible refresh button.
 *
 * - **HK in-game suffix**: WPF only appends `GashRemainInGame` when
 *   `LoginRegion != "TW" && remainPoint != 0` and the displayed
 *   in-game value is `Math.Floor(remainPoint / 2.5)`. We re-derive
 *   the same condition from `auth.session.region` (the SPA
 *   equivalent of `App.LoginRegion`) so the formatted string is
 *   byte-for-byte the same as the WPF header text.
 *
 * - **No success toast**: the refresh button updates the visible
 *   value in place — that *is* the user feedback. Adding a "點數
 *   已更新" toast would be SPA-only chrome the WPF user never saw.
 *   Failures still toast via `wrapCommand`.
 */
const refreshing = ref(false)

async function handleRefreshBalance(): Promise<void> {
  if (refreshing.value) return
  refreshing.value = true
  try {
    await account.getRemainPoint(true)
  } catch {
    /*
     * `wrapCommand` (inside `account.getRemainPoint`) already toasted +
     * logged the structured error. Mirror WPF: a failed re-fetch
     * leaves the previously-displayed value in place — no inline
     * banner, no exception bubbling up to the page-level error UI
     * (which is reserved for `loadList()` failures that prevent
     * the page from rendering at all).
     */
  } finally {
    refreshing.value = false
  }
}

/**
 * Formatted Gash-balance text shown next to the refresh button.
 * Re-derived whenever `account.remainPoint` or the active session
 * region changes — never stored, so a logout-clear of the store's
 * `remainPoint` (`account.clearSessionData`) collapses the display
 * back to the placeholder without any extra wiring here.
 *
 * Format mirrors WPF `MainWindow.updateRemainPoint` (L1716-1721)
 * exactly — see {@link handleRefreshBalance} docblock for the
 * decision table.
 */
const formattedRemainPoint = computed(() => {
  const value = account.remainPoint
  if (value === null) return t('accountList.gashBalancePlaceholder')
  const region = auth.session?.region
  const showInGame = region !== 'TW' && value !== 0
  const inGameSuffix = showInGame ? t('GashRemainInGame', [Math.floor(value / 2.5)]) : ''
  return t('GashRemain', [`${value}${inGameSuffix}`])
})

/* --------------- add service account (D3 + D8g unconnected branch) --------------- */

/**
 * `<AddServiceAccount />` modal visibility. Driven by the Plus
 * button at the bottom of the list **only** when the active game
 * is a *connected* game. Unconnected games (per
 * {@link UNCONNECTED_GAME_CODES}) take the {@link unconnectedAddVisible}
 * branch instead — see {@link handleAddAccount} for the dispatch.
 */
const addAccountVisible = ref(false)

/**
 * D8g — `<UnconnectedGameAddAccount />` modal visibility. Mirrors
 * the WPF `btnAddServiceAccount_Click` second branch
 * (`AccountList.xaml.cs` L117-135) which opens
 * `UnconnectedGame_AddAccount` when the active service is in
 * {@link UNCONNECTED_GAME_CODES}.
 */
const unconnectedAddVisible = ref(false)

/**
 * Add Service Account button handler. Branches between the two
 * dialogs on {@link useGameStore.isUnconnectedGame}, mirroring WPF
 * `btnAddServiceAccount_Click` (L117-135) verbatim.
 *
 * # Defensive guard
 *
 * If no game is selected yet (cold mount, picker still open),
 * surface `MsgSelectGame` and bail. WPF doesn't need this guard
 * because its UI binding disables the button until
 * `selectedGameChanged()` fires; the SPA mockup keeps the button
 * always-visible (the game-info-bar layout would jump if it
 * appeared/disappeared), so we add the runtime gate.
 */
function handleAddAccount(): void {
  if (!game.selectedGame) {
    ElMessage.warning(t('GameSelected'))
    return
  }
  if (game.isUnconnectedGame) {
    unconnectedAddVisible.value = true
  } else {
    addAccountVisible.value = true
  }
}

/**
 * Refresh the account list once an unconnected-game add succeeds.
 * Mirrors WPF `redrawSAccountList()` after the dialog closes (the
 * regular `AddServiceAccount.vue` path already wires a similar
 * refresh inside its own success handler — see that component's
 * docblock). Failures stay silent here because `loadList`'s own
 * error path already toasts via `wrapCommand`.
 */
function handleUnconnectedAccountCreated(): void {
  void loadList()
}

/* --------------- D8h — per-row change password (unconnected only) --------------- */

/**
 * `<UnconnectedGameChangePassword />` modal visibility + the
 * `accountIndex` prop the dialog forwards to the backend.
 *
 * Mirrors WPF `m_ChangePassword_Click` (`AccountList.xaml.cs`
 * L227-235) which opens `UnconnectedGame_ChangePassword(this,
 * list_Account.SelectedIndex)`. The SPA captures the row at the
 * menu-trigger time so a downstream selection change can't
 * misroute the change-password POST.
 *
 * # Why one set of refs (not per-row)
 *
 * Same rationale as `changeAliasTarget` / `accountInfoTarget`
 * above — the dialog is modal and the user can only change one
 * password at a time.
 */
const changePasswordVisible = ref(false)
/**
 * 0-based row index inside `account.serviceAccounts` that the
 * dialog should pass to the backend `unconnectedGameChangePassword`
 * IPC. Mirrors WPF's `list_Account.SelectedIndex` argument. The
 * default `-1` is a sentinel that should never reach the backend
 * (the menu item is `v-if`-gated below); kept as a safe init so
 * TypeScript can prove the prop is always a `number`.
 */
const changePasswordAccountIndex = ref(-1)

function handleChangePassword(targetAccount: ServiceAccount): void {
  const idx = account.serviceAccounts.findIndex((a) => a.sid === targetAccount.sid)
  if (idx < 0) {
    /*
     * Race-condition safeguard: the row was removed from the list
     * between menu-open and item-click (e.g. a concurrent refresh
     * dropped it). Surface the same `MsgSelectAccount` toast WPF
     * uses when `SelectedIndex < 0`.
     */
    ElMessage.warning(t('MsgSelectAccount'))
    return
  }
  changePasswordAccountIndex.value = idx
  changePasswordVisible.value = true
}

/*
 * Refresh on `verify-code-sent` mirrors `redrawSAccountList()`
 * (the WPF parent runs after the dialog `Close()`s). Strictly
 * speaking the password-reset email step doesn't change the
 * account list rows, but a refresh re-syncs server-side mutable
 * fields (`slastusedtime` etc.) the user might rely on. Cheap,
 * idempotent, and matches WPF's "refresh after every modal close"
 * convention.
 */
function handleChangePasswordSent(): void {
  void loadList()
}

/* --------------- D8f — Start Game pipeline --------------- */

/**
 * Parsed `login_action_type` for the active game's INI entry,
 * defaulted per WPF `MainWindow.xaml.cs` L547 (empty INI → 8).
 * Drives the {@link startGameDirect} branch and the OTP-completed
 * "OTP+launch" chain inside {@link handleGetOtp}.
 */
const loginActionType = computed<number>(() => {
  const raw = game.selectedIni?.login_action_type ?? ''
  if (raw === '') return 8
  const parsed = Number.parseInt(raw, 10)
  return Number.isNaN(parsed) ? 8 : parsed
})

/**
 * `tradLogin` (傳統登入) preference. Stored in Config.xml under
 * key `tradLogin`, default `"true"` per WPF `Settings.xaml.cs`
 * L68. Case-insensitive parse mirrors the {@link autoPaste}
 * historical-config defence above.
 */
const tradLogin = computed<boolean>(() => {
  return configStore.getOr('tradLogin', 'true').toLowerCase() === 'true'
})

/**
 * `true` when Start Game should bypass the OTP fetch and launch
 * the game binary directly with empty credentials. Mirrors WPF
 * `Pages/AccountList.xaml.cs::Button_Click` L57-63:
 *
 * ```cs
 * if ((tradLogin && login_action_type == 1) || login_action_type == 0)
 *     runGame();              // direct, no creds
 * else
 *     btnGetOtp_Click();      // OTP first
 * ```
 *
 * The other path (`!tradLogin && login_action_type == 1`) takes
 * the OTP route and chains into `runGame(account, otp)` from the
 * OTP-completed handler — see the {@link handleGetOtp} branch
 * mirroring `MainWindow.xaml.cs` L2152-2155.
 */
const startGameDirect = computed<boolean>(() => {
  if (!game.selectedIni) return false
  return (tradLogin.value && loginActionType.value === 1) || loginActionType.value === 0
})

/**
 * Whether the OTP+launch chain is the active OTP completion
 * branch. Mirrors `MainWindow.xaml.cs` L2152
 * (`!tradLogin && login_action_type == 1`). Used by
 * {@link handleGetOtp} to chain into {@link runGame} instead of
 * the auto-paste / clipboard branch.
 */
const otpLaunchChain = computed<boolean>(() => {
  if (!game.selectedIni) return false
  return !tradLogin.value && loginActionType.value === 1
})

/**
 * Start Game button enable rule. Mirrors WPF UI bindings:
 *
 * - Game must be selected (no game = no INI = nothing to launch).
 * - Direct-launch branch: account selection is **not** required
 *   (the game's own launcher prompts the user for credentials).
 * - OTP-required branch: account selection is mandatory because
 *   `bfClient.GetOTP(account, ...)` needs a target.
 */
const startGameDisabled = computed<boolean>(() => {
  if (!game.selectedGame) return true
  if (startGameDirect.value) return false
  return account.selectedServiceAccount === null
})

/**
 * Map the `startGameMode` config value (string-encoded integer
 * `"0"` / `"1"` / `"2"` per WPF `MainWindow.xaml.cs` L1837 +
 * `enum GameStartMode` L32-37) to the backend [`GameStartMode`]
 * tagged union.
 *
 * The backend enum re-serialises as PascalCase string literals
 * (`"Auto"` / `"Normal"` / `"LocaleRemulator"`) over IPC — mapping
 * here lets the frontend read the legacy integer config without
 * the backend having to accept either shape. Defaults to `Auto`
 * on missing / unparseable input, mirroring WPF's `int.Parse`
 * fallback path (default `"0"` is `Auto`).
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
 * The check is purely advisory — we surface `MsgGamePathHaveWChar`
 * but continue with the launch (matching WPF's `break` after the
 * MessageBox).
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
 *      `accountList.gamePathPickerPending` toast so the user knows
 *      the redirect target is missing.
 *    - No → open the game's `download_url` (`Process.Start` with
 *      `UseShellExecute = true` — IPC equivalent is
 *      [`commands.openUrl`]).
 *    Either branch returns `null` to signal "abort the launch".
 * 3. Re-read after the prompt (WPF L1747-1751 paranoid double-check
 *    in case the user picked a path mid-flow). Replicated for
 *    parity even though the SPA's prompt path doesn't currently
 *    mutate the config inline.
 *
 * Returns the resolved game path on success, `null` when the
 * launch should abort (user chose "No" / pending-Settings toast
 * surfaced / re-read still empty).
 */
async function resolveGamePath(): Promise<string | null> {
  const selected = game.selectedGame
  const ini = game.selectedIni
  if (!selected || !ini || game.selectedGameCode === null) {
    ElMessage.warning(t('GameSelected'))
    return null
  }

  const detectResult = await safeInvoke(
    commands.detectGamePath(game.selectedGameCode, ini.dir_value_name, ini.dir_reg),
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
     * `ElMessageBox.confirm` rejects with `'cancel'` on Cancel and
     * `'close'` on the X / Esc. Treat `'cancel'` as the "No" branch
     * so the user can use the cancel button as the WPF `No` button
     * (matches mockup parity); `'close'` short-circuits the launch.
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
   * Beanfun catalogue doesn't always populate it). Fall through
   * to the same pending-Settings toast in that case so the user
   * has a consistent fallback message instead of a silent no-op.
   */
  if (selected.download_url === '') {
    ElMessage.info(t('accountList.gamePathPickerPending'))
    return null
  }
  await safeInvoke(commands.openUrl(selected.download_url))
  return null
}

/**
 * Already-running-process check + optional kill prompt. Mirrors
 * WPF L1765-1833:
 * 1. Enumerate processes whose `executable_path` matches the
 *    target `gamePath` (backend `list_game_processes`
 *    encapsulates the WPF process-name regex + WMI filter).
 * 2. If any match, prompt `MsgGameAlreadyRun` Yes/No.
 *    - Yes → `kill_game_processes(pids)` and continue.
 *    - No  → continue without killing (WPF launches anyway,
 *      treating the prompt as advisory).
 *
 * Returns `true` to proceed with the launch, `false` to abort
 * (user dismissed the prompt with Esc / X).
 */
async function checkAndKillRunningGameProcesses(gamePath: string): Promise<boolean> {
  const listResult = await safeInvoke(commands.listGameProcesses(gamePath))
  if (!listResult.ok) {
    /*
     * Process enumeration failed (rare — usually a WMI permission
     * issue). The standard wrapCommand-style toast would be too
     * loud here; log and continue so the user can still launch.
     */
    console.warn('[AccountList] listGameProcesses failed:', listResult.error)
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

/**
 * Main launch entry point. Mirrors WPF `runGame(account, password)`
 * (`MainWindow.xaml.cs` L1724-1900) end-to-end with the WPF
 * sub-routines factored into the helpers above for SRP.
 *
 * Empty `account` / `password` string args (the default) mean
 * "no credentials" — `commands.launchGame` treats that as
 * `command_line` substitution being disabled, matching WPF L1867-1879
 * (`account != "" && password != "" && game_commandLine != ""`).
 *
 * # Why no try/catch around `commands.launchGame`
 *
 * The IPC funnels failures through `wrapCommand` already (toast +
 * console). WPF L1895-1899 wraps the launch in a try/catch that
 * surfaces `MsgLocalePluginRunError` for LR-mode failures; the
 * backend `launch_game` already maps LR-resource / spawn errors to
 * structured `CommandError` codes that the frontend toasts via
 * `wrapCommand`. Re-catching here would double-toast.
 */
async function runGame(accountId = '', password = ''): Promise<void> {
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
     * `break`s out of the scan loop and continues the launch. We
     * do the same with a non-blocking warning toast.
     */
    ElMessage.warning(t('MsgGamePathHaveWChar'))
  }

  const proceed = await checkAndKillRunningGameProcesses(gamePath)
  if (!proceed) return

  const mode = resolveStartMode()

  await wrapCommand(commands.launchGame(gamePath, mode, ini.exe, accountId, password))
}

/**
 * Start Game button click handler. Mirrors WPF
 * `Pages/AccountList.xaml.cs::Button_Click` (L55-71):
 *
 * - {@link startGameDirect} branch → `runGame()` with empty
 *   credentials.
 * - Otherwise → defer to {@link handleGetOtp}, which itself
 *   forks on {@link otpLaunchChain} after the OTP arrives:
 *   * `true`  → `runGame(account, otp)` (the OTP+launch chain).
 *   * `false` → existing auto-paste / clipboard flow.
 *
 * # Why no in-flight UI lock
 *
 * Same SRP rationale as the existing {@link handleGetOtp}
 * docblock — the SPA's IPC-level concurrency model means a
 * stuck `launchGame` call cannot deadlock other UI affordances.
 * Adding a global busy guard here would just reproduce the WPF
 * foot-gun where a slow launch blocks Logout.
 */
async function handleStartGame(): Promise<void> {
  if (!game.selectedGame) {
    ElMessage.warning(t('GameSelected'))
    return
  }
  if (startGameDirect.value) {
    await runGame()
    return
  }
  await handleGetOtp()
}

/* --------------- per-row context menu (D4) --------------- */

/**
 * Per-row context menu state. Driven by the row's `more_vert`
 * trigger via `<el-dropdown trigger="click">`, which is the SPA
 * equivalent of WPF's right-click `ContextMenu` on
 * `lstViewAccount.ItemTemplate` (`AccountList.xaml` L228+).
 *
 * The dropdown only owns the popover affordance; the actual
 * action handlers (Change Alias, Account Info, Change Email, Copy
 * ID, Official Site) live as direct page methods so each future
 * D-step can wire its own item without touching the dropdown
 * markup itself.
 *
 * # WPF deviation: the dropdown trigger
 *
 * WPF used a right-click `ContextMenu` directly on the row, with no
 * visible affordance. The SPA uses a left-click on a `more_vert`
 * icon button — this matches both `mockups/AccountList.html`
 * (which shows the icon at the row's right edge) and the broader
 * desktop-SPA convention that hidden right-click menus are an
 * accessibility hazard.
 *
 * # Why the menu items are split across D-steps
 *
 * The mockup's full menu is:
 *   1. Copy ID                — P12.2 D-step (clipboard wire)
 *   2. Change Alias           — **REAL since D4**
 *   3. Account Info           — P12.2 D6 (`windows/ServiceAccountInfo.vue`)
 *   4. Change Email           — P12.2 D-step (separate change-email dialog)
 *   5. Official Site          — P12.4 (WebBrowser open URL)
 *
 * D4 ships the Change Alias item; D6 adds Account Info. Each
 * subsequent D-step adds another `<el-dropdown-item>` here — no
 * need for a registry / dispatch table when the menu is this
 * small (YAGNI).
 */
const changeAliasVisible = ref(false)
const changeAliasTarget = ref<ServiceAccount | null>(null)

function handleChangeAlias(a: ServiceAccount): void {
  /*
   * Snapshot the row at trigger time so a downstream `selectRow`
   * (or list refresh that reorders) can't swap the dialog's
   * target out from under the user. The snapshot is cleared via
   * the `watch` below when the dialog closes so we don't leak
   * stale references between sessions.
   */
  changeAliasTarget.value = a
  changeAliasVisible.value = true
}

/*
 * Forget the target on close (cancel / submit success / Esc).
 * Lifting this into a `watch` instead of an explicit
 * `@update:visible` handler avoids piggy-backing on Vue's
 * internal v-model listener-merge behaviour and keeps the
 * dialog's update emit single-purpose (toggle the visibility
 * ref). The dialog's own `@closed` hook resets its form state
 * internally — this watcher is only responsible for releasing
 * the parent-owned target reference.
 */
watch(changeAliasVisible, (next) => {
  if (!next) changeAliasTarget.value = null
})

/*
 * Same row-snapshot + target-release pattern as Change Alias
 * above, for the D6 read-only Account Info dialog. Mirrors WPF
 * `m_AccInfo_Click` (L212-219) which captured `list_Account.SelectedItem`
 * once and threw on null — the SPA snapshot here is the row passed
 * into the menu callback (already non-null by construction), so
 * the WPF null-guard collapses to a no-op.
 */
const accountInfoVisible = ref(false)
const accountInfoTarget = ref<ServiceAccount | null>(null)

function handleAccountInfo(a: ServiceAccount): void {
  accountInfoTarget.value = a
  accountInfoVisible.value = true
}

watch(accountInfoVisible, (next) => {
  if (!next) accountInfoTarget.value = null
})

/* --------------- Get Email (D10.5) --------------- */

/**
 * Per-row "Check verification email" action — mirrors WPF
 * `m_GetEmail_Click` (`AccountList.xaml.cs` L204-209):
 *
 * ```cs
 * new CopyBox(
 *     TryFindResource("AuthEmail") as string,
 *     App.MainWnd.bfClient.getEmail()
 * ).ShowDialog();
 * ```
 *
 * # WPF parity choices
 *
 * - **Lives on the row context menu**, not on a page-level toolbar.
 *   WPF wired `m_GetEmail` as a `MenuItem` inside the row's
 *   `ContextMenu` (`AccountList.xaml` L253-257); the SPA mirrors
 *   that placement. The `getEmail` IPC itself does **not** take an
 *   account argument — it returns the verification email for the
 *   logged-in account from the shared session cookie — so the row
 *   parameter is unused at the call site, but the menu placement
 *   stays per-row to match WPF UX.
 * - **CopyBox dialog** is a generic `(title, value) + Copy` dialog
 *   (D10.1) — same component WPF used. Title is the WPF locale key
 *   `AuthEmail` ("驗證信箱地址 / Auth Email"), body shows the email
 *   address with a Copy button.
 * - **Error path**: WPF wraps the `getEmail` call in a try/catch
 *   that surfaces a generic error dialog. We funnel through
 *   `wrapCommand` so the standard `errors.{code}` translator +
 *   session-expired hook fire — same operational behaviour as every
 *   other IPC call in this file. On error the dialog is **not**
 *   opened (matches WPF — if the IPC throws, no CopyBox is shown).
 *
 * # Why a single set of refs (not per-row)
 *
 * The dialog is modal and the user can only inspect one email at a
 * time. A single `copyBoxVisible` / `copyBoxTitle` / `copyBoxValue`
 * trio is sufficient — no need for a `Map<sid, ...>` of dialog
 * states. Mirrors the same pattern as `changeAliasTarget`.
 */
const copyBoxVisible = ref(false)
const copyBoxTitle = ref('')
const copyBoxValue = ref('')

/*
 * No `account` param: WPF `bfClient.getEmail()` (and our matching
 * `commands.getEmail()` IPC) returns the verification email for the
 * **logged-in account**, sourced from the shared session cookie —
 * the per-row menu placement is purely UX (matches WPF), not a
 * per-row data dependency.
 */
async function handleGetEmail(): Promise<void> {
  try {
    const email = await wrapCommand(commands.getEmail())
    copyBoxTitle.value = t('AuthEmail')
    copyBoxValue.value = email
    copyBoxVisible.value = true
  } catch {
    /*
     * `wrapCommand` already toasted the error and logged the
     * structured cause. Match WPF's "no dialog if the IPC failed"
     * behaviour by simply not flipping `copyBoxVisible`.
     */
  }
}

/* --------------- OTP / auto-paste flow (D5) --------------- */

/**
 * # D5 — Get OTP / Copy OTP / Auto-paste preference
 *
 * Mirrors the WPF orchestration spread across:
 *
 * - `Beanfun/Pages/AccountList.xaml.cs`
 *   - `btnGetOtp_Click` (L82-101) — guard-rail + UI lock + spawn worker
 *   - `t_Password_PreviewMouseLeftButtonDown` (L103-115) — silent copy
 *   - `autoPaste_CheckedChanged` (L73-80) — first-time tip + persist
 * - `Beanfun/MainWindow.xaml.cs`
 *   - `getOtpWorker_DoWork` (L2092-2128) — single OTP IPC (`bfClient.GetOTP`)
 *   - `getOtpWorker_RunWorkerCompleted` (L2131-2240) — three branches:
 *     game-launch / clipboard-only / auto-paste; the auto-paste
 *     branch is also fallback-to-clipboard if the launcher window
 *     is missing (L2169-2174)
 *
 * Backend already exposes the building blocks
 * (`commands.getOtp(account)` from P11; `commands.autoPaste(req)`
 * from P10.3 D5d, including the `process.window_not_found` error
 * code we branch on for clipboard fallback). D5 is a thin frontend
 * wiring on top — no new IPC.
 *
 * # In-flight UI lock — narrower than WPF on purpose
 *
 * WPF disables seven controls while the worker runs (list / Get OTP
 * / Logout / Change Game / game name / Start Game / Add Service
 * Account / context menu) because BackgroundWorker can't be cancelled
 * mid-flight without leaving partial state behind. The SPA only
 * disables the Get OTP button itself: the IPC is async + cancellable
 * at the runtime level, every other affordance (logout, row
 * selection, dialog opens) is idempotent against an in-flight
 * `getOtp`, and a global lock would create the WPF foot-gun where a
 * stuck request blocks Logout. Same scope rationale as D3/D4.
 *
 * # Why the autoPaste branch swallows `process.window_not_found`
 *
 * `commands.autoPaste` returns a structured error code when the
 * launcher window isn't present yet (the user hasn't started the
 * game, or it's still on the splash screen). WPF L2169-2174 treats
 * "window missing" as a graceful fallback — copy to clipboard +
 * show the success-and-copy toast — because the user is going to
 * launch the game manually anyway and just needs the OTP in their
 * paste buffer. We branch via `safeInvoke` so the
 * `wrapCommand` toast pipeline doesn't fire for what is, from the
 * user's POV, a successful flow (just down a different branch).
 * Other autoPaste errors still get the standard
 * `surfaceCommandError` toast — they're real failures (e.g.
 * `process.non_ascii` for a corrupted SID, `process.platform_unsupported`
 * on a non-Windows dev build).
 *
 * # Why `specialClick` is computed on the frontend
 *
 * The backend service deliberately accepts a `bool` rather than the
 * `(service_code, service_region)` pair so it stays free of
 * MapleStory-specific business rules (see the D5d module docs). The
 * single matching test — `service_code === '610074' && service_region === 'T9'`
 * (WPF L2195) — is what lights up the SEA/TW pre-login dismiss
 * sequence; everything else gets the basic clear-and-type path.
 */

/**
 * The OTP string returned by the most recent `getOtp` call. Cleared
 * when:
 *
 * 1. The user changes selection (`watch(selectedSid)` below) — the
 *    OTP is row-bound and would mislead if it stayed visible while
 *    a different account is highlighted.
 * 2. Logout (`account.clearSessionData()` already covers this; no
 *    additional wiring needed here).
 * 3. The next `handleGetOtp` invocation — pre-clear before kicking
 *    off the request, mirroring WPF L91 (`t_Password.Text = ""`).
 */
const otpValue = ref('')

/**
 * In-flight guard for the OTP fetch + auto-paste sequence. Drives
 * (a) the Get OTP button's `:disabled` rule (re-entrancy guard) and
 * (b) the button label flip from `GetOtp` → `GettingOtp`, mirroring
 * WPF L90.
 */
const gettingOtp = ref(false)

/**
 * Initial value for the auto-paste preference. Read once at setup
 * via `useConfigStore.getOr('autoPaste', 'false')` — `'false'`
 * matches WPF's default (`AccountList.xaml.cs` L25 / L75
 * `GetValue("autoPaste", "false")`). The string-vs-boolean mismatch
 * is intentional: WPF stores the `Boolean.ToString()` representation
 * (`"True"` / `"False"`, or `"true"` / `"false"` after normalisation
 * by .NET's invariant culture); we round-trip through `String(bool)`
 * (`"true"` / `"false"`) on writes so old WPF-written Config.xml
 * values keep parsing.
 *
 * The case-insensitive equality check (`.toLowerCase() === 'true'`)
 * is the safety net for any historical Config.xml that captured
 * `"True"` from WPF's pre-normalisation era — strict `=== 'true'`
 * would silently treat those rows as "default false" and surprise
 * the upgrading user.
 */
const autoPaste = ref(configStore.getOr('autoPaste', 'false').toLowerCase() === 'true')

/**
 * Reset the OTP value when the user picks a different row. Without
 * this, the OTP for the previously-selected account would visually
 * stay attached to whichever row is currently highlighted — a
 * mis-binding bug that's easy to introduce because OTP refresh is
 * gesture-driven, not selection-driven.
 *
 * Logout is handled separately by `account.clearSessionData()` —
 * `selectedSid` becomes null along with everything else.
 */
watch(
  () => account.selectedSid,
  () => {
    otpValue.value = ''
  },
)

/**
 * Persist the auto-paste preference and, on the user's first-ever
 * toggle, surface the AutoPasteTip explainer (mirrors WPF L77-78:
 * `if (GetValue("autoPaste", "") == "") MessageBox.Show(...)`). The
 * sentinel test compares `configStore.get('autoPaste') === undefined`
 * to "key has never been written"; we then unconditionally write
 * the new value so future toggles skip the tip.
 *
 * The `change` handler is wired manually (rather than `v-model`)
 * because the tip + persistence side effects need to run on the
 * user's gesture, not on every reactive re-write of the local ref
 * (which would also fire on the initial setup hydration above).
 */
async function handleAutoPasteToggle(next: boolean | string | number): Promise<void> {
  /*
   * `<el-checkbox>` types its `change` payload as
   * `boolean | string | number` (the same union it accepts via
   * `value` for non-binary checkboxes). We always bind a binary
   * checkbox so the runtime value is a `boolean`, but coercing
   * defensively keeps us safe from any future Element Plus refactor
   * that tightens the type.
   */
  const nextBool = Boolean(next)
  autoPaste.value = nextBool

  /*
   * `=== undefined` (not `=== '' || === undefined`) because our
   * cache normalises Config.xml's missing-key sentinel to
   * `undefined` (see `useConfigStore`'s docblock on the absent-key
   * vs empty-string distinction). WPF used the empty-string sentinel
   * because .NET `ConfigurationManager.AppSettings[key]` returns
   * `null` ↔ `""` interchangeably.
   */
  const isFirstToggle = configStore.get('autoPaste') === undefined
  if (isFirstToggle) {
    /*
     * `info` (not `success` / `warning`) so the toast colour reads
     * as informational. `duration: 8000` + `showClose: true` gives
     * the user enough time to read the two-paragraph explainer; the
     * default `3000ms` is too short for a multi-line message.
     */
    ElMessage.info({
      message: t('accountList.autoPasteTip'),
      duration: 8000,
      showClose: true,
    })
  }

  /*
   * `wrapCommand` (inside `configStore.set`) toasts on failure, so
   * we don't catch — leave the local `autoPaste` ref ahead of the
   * persisted value if the disk write fails so the user can retry
   * with a fresh toggle (Config.xml writes are atomic; partial-write
   * recovery isn't a concern).
   */
  await configStore.set('autoPaste', String(nextBool))
}

/**
 * Copy `text` to the system clipboard, optionally surfacing the
 * WPF `GetOtpSuccessAndCopy` / `CopyFailed` toasts.
 *
 * Two callers, two policies:
 *
 * - `handleGetOtp` clipboard branch → `withSuccessToast = true`
 *   (mirrors WPF L2171-2172 `MessageBox.Show("已複製")`).
 * - `handleCopyOtp` (manual copy button) → `withSuccessToast = false`
 *   (mirrors WPF L110-114 silent `Clipboard.SetText` + `catch {}`).
 *
 * `navigator.clipboard.writeText` rejects on user-permission denial
 * inside iframes / cross-origin contexts; in our Tauri webview the
 * trust boundary is the OS user session so denial is unexpected, but
 * we still toast so the user knows the OTP wasn't copied (otherwise
 * they'd silently try to paste from an empty buffer).
 */
async function clipboardWriteOtp(text: string, withSuccessToast: boolean): Promise<boolean> {
  try {
    await navigator.clipboard.writeText(text)
    if (withSuccessToast) {
      ElMessage.success(t('GetOtpSuccessAndCopy'))
    }
    return true
  } catch {
    if (withSuccessToast) {
      ElMessage.error(t('CopyFailed'))
    }
    return false
  }
}

/**
 * Get OTP for the currently-selected service account. Mirrors WPF
 * `getOtpWorker_DoWork` + `getOtpWorker_RunWorkerCompleted`, minus
 * the dead `tradLogin && login_action_type == 1` branch (covered by
 * P12.3 Start Game flow) and minus the WPF race-condition that
 * re-reads `list_Account.SelectedIndex` on the worker-completed
 * thread (we snapshot `target` at trigger time — see Q6 in the
 * P12.2 D5 decision table).
 *
 * Branch table (matches the D5 Q4 decision):
 *
 * | Outcome                              | Toast                           | Side effect          |
 * | ------------------------------------ | ------------------------------- | -------------------- |
 * | No row selected                      | `MsgSelectAccount` warning      | none                 |
 * | `getOtp` throws                      | `wrapCommand` (detail)          | otpValue stays empty |
 * | `getOtp` ok + autoPaste off          | `GetOtpSuccessAndCopy`          | clipboard copy       |
 * | `getOtp` ok + autoPaste on + ok      | none (silent, mirrors WPF)      | game receives keys   |
 * | `getOtp` ok + autoPaste on + miss    | `GetOtpSuccessAndCopy`          | clipboard copy       |
 * | `getOtp` ok + autoPaste on + other   | `surfaceCommandError` (detail)  | otpValue stays set   |
 */
async function handleGetOtp(): Promise<void> {
  if (gettingOtp.value) return

  const target = account.selectedServiceAccount
  if (!target) {
    ElMessage.warning(t('MsgSelectAccount'))
    return
  }

  /*
   * Pre-clear so the previous OTP doesn't visually persist while the
   * fetch is in flight — same UX guarantee as WPF L91. Snapshot
   * `target` into a `const` so any in-flight selection change can't
   * misroute the auto-paste payload.
   */
  otpValue.value = ''
  gettingOtp.value = true

  let otp: string
  try {
    otp = await account.getOtp(target)
  } catch {
    /*
     * `account.getOtp` funnels through `wrapCommand`, which already
     * console-logged + toasted the cause. WPF wraps a caption
     * (`GetOtpFailed`) around the detail in `errexit`, but a
     * matching SPA caption-toast on top of the detail-toast would
     * just double-fire — the cause messages from the backend are
     * already self-explanatory ("未登入" / "網路連線異常" / etc).
     */
    gettingOtp.value = false
    return
  }

  otpValue.value = otp

  /*
   * D8f — OTP+launch chain. WPF `MainWindow.xaml.cs` L2152-2155
   * (`getOtpWorker_RunWorkerCompleted` first branch):
   *
   *   if (!tradLogin && login_action_type == 1)
   *       runGame(account, otp);
   *
   * This is the path used by Start Game when {@link startGameDirect}
   * is `false` (so we routed through `handleGetOtp` → `runGame`).
   * The auto-paste / clipboard branches below are skipped — the
   * launcher binary itself receives the credentials via
   * `command_line` substitution, and re-running auto-paste on top
   * would type the OTP into the login dialog twice.
   */
  if (otpLaunchChain.value) {
    gettingOtp.value = false
    await runGame(target.sid, otp)
    return
  }

  if (!autoPaste.value) {
    await clipboardWriteOtp(otp, true)
    gettingOtp.value = false
    return
  }

  /*
   * `auth.session` is guaranteed non-null here because the route
   * guard wouldn't have let us mount AccountList otherwise, but we
   * still derive `specialClick` defensively in case a future code
   * path mutates session mid-flight (the worst-case fallback is
   * `false` → no SEA pre-click sequence, which is the safer default
   * for non-MapleStory-TW games).
   */
  const session = auth.session
  const specialClick = session?.service_code === '610074' && session?.service_region === 'T9'

  /*
   * D8 — `className` flows from the per-game INI (WPF
   * `accountList.win_class_name`, captured by
   * `selectedGameChanged()` L562). Falls back to `'MapleStoryClass'`
   * only when the INI hasn't loaded yet (cold mount race) so the
   * paste call still has the historical default that worked across
   * the MapleStory family before P12.3's INI wiring landed.
   *
   * `safeInvoke` (not `wrapCommand`) so we can branch on
   * `process.window_not_found` without firing the wrapCommand toast
   * — that case is a graceful fallback to clipboard, not a user-
   * facing error. All other autoPaste errors get manually surfaced
   * via `surfaceCommandError` so the toast pipeline stays consistent
   * (console log + i18n translate + ElMessage.error).
   */
  const className = game.selectedIni?.win_class_name || 'MapleStoryClass'
  const result = await safeInvoke(
    commands.autoPaste({
      className,
      account: target.sid,
      password: otp,
      specialClick,
    }),
  )

  if (result.ok) {
    /*
     * Silent on success — the user sees the OTP land in the game
     * window, and the SPA toast would just visually clutter that
     * confirmation moment. Mirrors WPF L2235-2237 (PostString +
     * Enter, no MsgBox).
     */
  } else if (result.error.code === 'process.window_not_found') {
    /*
     * Game window missing — fall back to clipboard so the user can
     * paste manually after launching the game (mirrors WPF
     * L2169-2174). The success toast here is the WPF parity message;
     * we deliberately don't tell the user "auto-paste failed because
     * window missing" because that framing is a UX regression — WPF
     * presents the fallback as success and so should we.
     */
    await clipboardWriteOtp(otp, true)
  } else {
    surfaceCommandError(result.error)
  }

  gettingOtp.value = false
}

/**
 * Manual copy button — mirrors WPF `t_Password_PreviewMouseLeftButtonDown`
 * (`AccountList.xaml.cs` L103-115). Silent on both success and
 * failure (the WPF original wraps `Clipboard.SetText` in a bare
 * `try / catch {}` with no user-visible feedback either way) — see
 * the `clipboardWriteOtp` docblock for why the two callers diverge.
 */
function handleCopyOtp(): void {
  if (!otpValue.value) return
  void clipboardWriteOtp(otpValue.value, false)
}

/* --------------- drag-and-drop reorder (D7) --------------- */

/**
 * # D7 — Per-game drag-and-drop ordering
 *
 * Mirrors WPF `Beanfun/Pages/AccountList.xaml.cs` "Drag and Drop
 * Reorder" region (L257-451) + `SaveAccountOrder` (L477-487) +
 * `ApplyAccountOrder` (L489-531) + the `BeanfunClient.Account.cs`
 * L137-139 call site that runs ApplyAccountOrder immediately
 * after the server-side `OrderBy(ssn)`.
 *
 * # Why vuedraggable (and not native HTML5 drag-drop)
 *
 * `vuedraggable@4.1.0` is already in `package.json` (Vue 3 peer)
 * but had no consumers — using it here turns a dead dependency
 * into a load-bearing one and gives us:
 *
 * - The `handle` selector that mirrors WPF's `_isHandlePressed`
 *   gate (`MouseLeftButtonDown` only on the grip, never the row)
 *   for free, without the manual `ev.target` walking we'd need
 *   for HTML5 dnd.
 * - Sortable.js' built-in animation, ghost element, and same-list
 *   reorder semantics — re-implementing those over native dnd is
 *   ~200 lines of fiddly DOM measurement code.
 * - In-place mutation of the bound array via `:list`, which keeps
 *   Pinia reactivity end-to-end without needing a v-model
 *   computed setter just to satisfy the bind contract.
 *
 * D7.3 sanity check (Vue 3.5 + Vite 6 + Vitest 4) confirmed the
 * UMD bundle imports and mounts cleanly under our toolchain.
 *
 * # Why we still call `setServiceAccountOrder` after the @end event
 *
 * `<draggable :list>` mutates `account.serviceAccounts` in place,
 * so by the time `@end` fires the visible order is already
 * correct. The follow-up `account.setServiceAccountOrder` call
 * is **idempotent** in that case but earns its keep as the single
 * canonical reorder funnel: the spy in the page-level test (D7.5
 * case 2) asserts the store action receives the expected sid
 * order, which gives us a stable seam against future refactors
 * (e.g. adding analytics, deduplication, or a derived state
 * mirror — all would belong inside the action, not next to every
 * call site).
 *
 * # Why persist failures are silent (Q8)
 *
 * Mirrors WPF L482-487 `ConfigAppSettings.SetValue` which
 * swallows IO errors silently. A toast on every transient
 * Config.xml write failure during a fluent drag-drop interaction
 * would be jarring; the next refresh / drag will reconcile, and
 * the operation is non-destructive (only ordering is at stake,
 * not account data). We bypass `configStore.set` (which goes
 * through `wrapCommand` → toast on failure) by using `safeInvoke`
 * directly + manually updating the cache on success — see
 * `persistAccountOrder` below.
 *
 * # Why gameCode lives at the page (not the store)
 *
 * `gameCode = service_code + "_" + service_region` (WPF L482) is
 * a session-scoped derived value. The account store deliberately
 * doesn't import `useAuthStore` (SRP: it has no flow-continuation
 * special cases — see the file's top docblock); the page already
 * owns both stores (D5 specialClick is computed off the same two
 * fields), so deriving here keeps cross-store coupling at one
 * layer instead of bleeding into the persistent state layer.
 */

/**
 * Config.xml key for the current session's saved drag order. WPF
 * persists each gameCode under its own key (`AccountOrder_610074_T9`
 * etc., L486) so switching games picks up the per-game ordering
 * without manual reload — same here.
 *
 * Returns `null` when no session is active (e.g. the page renders
 * its loading state before the route guard hydrates). The two
 * load-time / drag-time call sites both no-op on `null`, so the
 * SPA gracefully tolerates the brief gap between mount and
 * session hydration without throwing.
 */
const accountOrderConfigKey = computed<string | null>(() => {
  const session = auth.session
  if (!session) return null
  return `AccountOrder_${session.service_code}_${session.service_region}`
})

/**
 * Write the new account order CSV to Config.xml without surfacing
 * a toast on failure (Q8 — mirror WPF silent `SetValue`). Bypasses
 * `configStore.set` (which calls `wrapCommand` → toast on failure)
 * and instead uses `safeInvoke` directly + manually updates the
 * cache on success so the in-memory snapshot stays in sync with
 * disk for the next read.
 *
 * On failure: console-log for dev visibility but otherwise stay
 * silent — the local `serviceAccounts` order is already correct,
 * the next refresh / next drag will reconcile, and the user
 * shouldn't get an error popup mid-fluent-drag.
 */
async function persistAccountOrder(key: string, csv: string): Promise<void> {
  const result = await safeInvoke(commands.setConfig(key, csv))
  if (result.ok) {
    configStore.entries[key] = csv
  } else {
    console.warn(`[AccountList] Failed to persist ${key}:`, result.error)
  }
}

/**
 * Vuedraggable `@end` handler. Vuedraggable's `:list` binding has
 * already mutated `account.serviceAccounts` in place by the time
 * this fires (Sortable.js' `onEnd` runs after the splice), so the
 * visible UI is already correct.
 *
 * Two follow-up writes:
 *
 * 1. {@link useAccountStore.setServiceAccountOrder} — canonicalises
 *    the order through the store action; idempotent here but
 *    keeps the spy seam for D7.5 case 2 and means any future
 *    invariant added to the action (e.g. dedup, analytics) lands
 *    at every reorder call site automatically.
 * 2. {@link persistAccountOrder} — write-through to Config.xml
 *    under the per-game key. Skipped when no session is active;
 *    silent on failure (mirrors WPF).
 */
function handleDragEnd(): void {
  const key = accountOrderConfigKey.value
  if (!key) return

  const orderedSids = account.serviceAccounts.map((a) => a.sid)
  account.setServiceAccountOrder(orderedSids)
  void persistAccountOrder(key, orderedSids.join(','))
}
</script>

<template>
  <main class="account-list bf-mica-bg">
    <div class="account-list__container">
      <header class="account-list__header">
        <h1 class="account-list__title bf-text-gradient">{{ t('accountList.title') }}</h1>
        <p class="account-list__subline">{{ t('accountList.subtitle') }}</p>
      </header>

      <!-- Game info bar (D8d) — real game name + image + change-game button. -->
      <section class="account-list__game bf-glass-panel">
        <div class="account-list__game-row">
          <div class="account-list__game-meta">
            <div class="account-list__game-icon" aria-hidden="true">
              <!--
                D8d: prefer the per-game banner image when the catalogue
                has hydrated; fall back to the generic VideoPlay glyph
                so the layout doesn't collapse during the brief setup
                window before `setupGameOnMount` resolves. The icon's
                container size is fixed regardless so the row height
                stays stable across the swap.
              -->
              <img
                v-if="gameImageUrl"
                :src="gameImageUrl"
                :alt="gameNameDisplay"
                class="account-list__game-icon-img"
                data-test="account-list-game-image"
              />
              <el-icon v-else :size="24"><VideoPlay /></el-icon>
            </div>
            <button
              type="button"
              class="account-list__game-info"
              :title="t('accountList.changeGame')"
              data-test="account-list-change-game"
              @click="handleChangeGame"
            >
              <span class="account-list__game-name" data-test="account-list-game-name">
                {{ gameNameDisplay }}
              </span>
              <span class="account-list__game-status">
                <span class="account-list__game-status-dot" />
                {{ t('accountList.statusOnline') }}
              </span>
            </button>
          </div>
          <div class="account-list__game-actions">
            <!--
              D8e: Tools button is only rendered for the three game
              codes WPF whitelisted (`610074_T9` / `610075_T9` /
              `610096_TE`). Hidden via `v-if` rather than `display:
              none` so QA can't see a hover affordance for a button
              that isn't reachable, and so the surrounding flex row
              tightens up cleanly when the button is absent.
            -->
            <button
              v-if="showToolsButton"
              type="button"
              class="bf-btn-ghost-icon account-list__icon-btn"
              :title="t('accountList.toolsButton')"
              data-test="account-list-tools"
              @click="handleTools"
            >
              <el-icon><Operation /></el-icon>
            </button>
            <button
              type="button"
              class="bf-btn-ghost-icon account-list__icon-btn account-list__icon-btn--danger"
              :title="t('Logout')"
              data-test="account-list-logout"
              @click="handleLogout"
            >
              <el-icon><SwitchButton /></el-icon>
            </button>
          </div>
        </div>
        <button
          type="button"
          class="bf-btn-gradient account-list__start-btn"
          :disabled="startGameDisabled"
          data-test="account-list-start"
          @click="handleStartGame"
        >
          <el-icon><VideoPlay /></el-icon>
          <span>{{ t('GameStart') }}</span>
        </button>
      </section>

      <!-- Quick actions row: balance + member center + support (all stubs) -->
      <section class="account-list__quick">
        <div class="account-list__balance bf-glass-card bf-ghost-border">
          <div class="account-list__balance-text">
            <span class="account-list__balance-label">
              {{ t('accountList.gashBalance') }}
            </span>
            <span class="account-list__balance-value" data-test="account-list-balance-value">
              {{ formattedRemainPoint }}
            </span>
          </div>
          <button
            type="button"
            class="account-list__balance-refresh"
            :class="{ 'account-list__balance-refresh--spinning': refreshing }"
            :title="t('accountList.refreshBalance')"
            :disabled="refreshing"
            data-test="account-list-refresh-balance"
            @click="handleRefreshBalance"
          >
            <el-icon><Refresh /></el-icon>
          </button>
        </div>
        <button
          type="button"
          class="account-list__quick-link bf-glass-card bf-ghost-border"
          data-test="account-list-member-center"
          @click="handleMemberCenter"
        >
          <el-icon><User /></el-icon>
          <span>{{ t('accountList.memberCenter') }}</span>
        </button>
        <button
          type="button"
          class="account-list__quick-link bf-glass-card bf-ghost-border"
          data-test="account-list-customer-service"
          @click="handleCustomerService"
        >
          <el-icon><Service /></el-icon>
          <span>{{ t('accountList.customerService') }}</span>
        </button>
      </section>

      <!-- Service accounts list — 4 rendered states (REAL) -->
      <section class="account-list__list bf-glass-panel">
        <header class="account-list__list-header">
          <h2 class="account-list__list-title">
            {{ t('accountList.serviceAccountsHeading') }}
          </h2>
          <span class="account-list__list-count" data-test="account-list-count">
            {{ t('accountList.accountCount', { count: accountCount }) }}
          </span>
        </header>

        <div class="account-list__list-body bf-custom-scrollbar">
          <p
            v-if="loadState === 'loading'"
            class="account-list__list-state"
            data-test="account-list-loading"
          >
            {{ t('accountList.loading') }}
          </p>

          <div
            v-else-if="loadState === 'error'"
            class="account-list__list-state account-list__list-state--error"
            data-test="account-list-error"
          >
            <p>{{ loadError ?? t('accountList.loadFailed') }}</p>
            <el-button
              type="primary"
              plain
              size="small"
              data-test="account-list-retry"
              @click="loadList"
            >
              {{ t('accountList.retry') }}
            </el-button>
          </div>

          <p
            v-else-if="serviceAccounts.length === 0"
            class="account-list__list-state"
            data-test="account-list-empty"
          >
            {{ t('accountList.empty') }}
          </p>

          <!--
            D7: vuedraggable wraps the row list. `:list` binds the
            actual mutable Pinia state array (not the read-only
            `serviceAccounts` computed) so Sortable.js' in-place
            splice flows through Pinia reactivity. The `handle`
            selector mirrors WPF's `_isHandlePressed` gate — only
            mouse-down on the grip starts a drag; any other click
            on the row falls through to `selectRow`. `ghost-class`
            styles the placeholder slot during drag (defined at the
            bottom of <style scoped>).
          -->
          <draggable
            v-else
            :list="account.serviceAccounts"
            tag="ul"
            item-key="sid"
            handle=".account-list__row-grip"
            :animation="150"
            ghost-class="account-list__row--ghost"
            class="account-list__rows"
            data-test="account-list-rows"
            @end="handleDragEnd"
          >
            <template #item="{ element: a, index: idx }">
              <li
                class="account-list__row"
                :class="{
                  'account-list__row--selected': isSelected(a),
                  'account-list__row--banned': !a.is_enable,
                }"
                :data-test="`account-row-${a.sid}`"
                @click="selectRow(a)"
              >
                <span
                  class="account-list__row-grip"
                  :title="t('accountList.dragHandle')"
                  aria-hidden="true"
                  >⋮⋮</span
                >
                <span class="account-list__row-num">{{ idx + 1 }}</span>
                <div class="account-list__row-info">
                  <p class="account-list__row-name">{{ a.sname }}</p>
                  <p class="account-list__row-sub">
                    <template v-if="a.is_enable">ID: {{ a.sid }}</template>
                    <template v-else>{{ t('accountList.statusBanned') }}</template>
                  </p>
                </div>
                <el-dropdown
                  trigger="click"
                  placement="bottom-end"
                  :hide-on-click="true"
                  popper-class="account-list__row-menu-popper"
                  @click.stop
                >
                  <button
                    type="button"
                    class="account-list__row-more"
                    :title="t('accountList.moreActions')"
                    :data-test="`account-row-more-${a.sid}`"
                    @click.stop
                  >
                    <el-icon><MoreFilled /></el-icon>
                  </button>
                  <template #dropdown>
                    <el-dropdown-menu>
                      <el-dropdown-item
                        :data-test="`account-row-change-alias-${a.sid}`"
                        @click="handleChangeAlias(a)"
                      >
                        <el-icon><EditPen /></el-icon>
                        <span>{{ t('ChangeAccountName') }}</span>
                      </el-dropdown-item>
                      <el-dropdown-item
                        :data-test="`account-row-info-${a.sid}`"
                        @click="handleAccountInfo(a)"
                      >
                        <el-icon><InfoFilled /></el-icon>
                        <span>{{ t('GameAccountInfo') }}</span>
                      </el-dropdown-item>
                      <el-dropdown-item
                        :data-test="`account-row-get-email-${a.sid}`"
                        @click="handleGetEmail"
                      >
                        <el-icon><Message /></el-icon>
                        <span>{{ t('CheckEmail') }}</span>
                      </el-dropdown-item>
                      <!--
                        D8h: Change Password menu item only appears for
                        unconnected games (mirrors WPF
                        `m_ChangePassword.Visibility` toggled by
                        `selectedGameChanged()` on the same predicate).
                        Connected games delegate password changes to
                        the Beanfun member centre web flow, which is
                        opened from the page-level chrome — no
                        per-row affordance is needed there.
                      -->
                      <el-dropdown-item
                        v-if="game.isUnconnectedGame"
                        :data-test="`account-row-change-password-${a.sid}`"
                        @click="handleChangePassword(a)"
                      >
                        <el-icon><Key /></el-icon>
                        <span>{{ t('ChangePassword') }}</span>
                      </el-dropdown-item>
                    </el-dropdown-menu>
                  </template>
                </el-dropdown>
              </li>
            </template>
          </draggable>
        </div>

        <footer class="account-list__list-footer">
          <button
            type="button"
            class="account-list__add-btn"
            data-test="account-list-add"
            @click="handleAddAccount"
          >
            <el-icon><Plus /></el-icon>
            <span>{{ t('AddServiceAccount') }}</span>
          </button>
        </footer>
      </section>

      <!-- Add Service Account modal (D3) — mounted unconditionally so its
           transitions can play; visibility is driven by `addAccountVisible`. -->
      <AddServiceAccount v-model:visible="addAccountVisible" />

      <!-- Change Service Account display-name modal (D4) — same mount-always
           pattern as D3. `changeAliasTarget` is cleared by the watcher above
           so stale account refs don't leak between sessions. -->
      <ChangeServiceAccountDisplayName
        v-model:visible="changeAliasVisible"
        :account="changeAliasTarget"
      />

      <!-- Service Account read-only info modal (D6) — same mount-always
           pattern as D3 / D4. `accountInfoTarget` is cleared by the
           watcher above so stale account refs don't leak between sessions. -->
      <ServiceAccountInfo v-model:visible="accountInfoVisible" :account="accountInfoTarget" />

      <!-- Generic copy-box dialog (D10.1) — currently driven by Get Email
           (D10.5); future per-row context-menu items that need a similar
           "show + copy" affordance can reuse the same single mount by
           writing into `copyBoxTitle` / `copyBoxValue` before flipping
           `copyBoxVisible`. -->
      <CopyBox v-model:visible="copyBoxVisible" :title="copyBoxTitle" :value="copyBoxValue" />

      <!-- D8c: Game picker dialog. Driven by either the Change Game
           button on the game info bar or the mount-time auto-open
           inside `setupGameOnMount` (when no valid `loginGame`
           resolves against the loaded catalogue). The dialog
           handles its own close on `@select`; we just listen for
           the discriminator pair and run the canonical
           `selectActiveGame` pipeline.

           `v-if="auth.session"` gate is purely a TS narrowing
           helper — the route guard already prevents AccountList
           from mounting without a hydrated session, so the
           condition will always be `true` at user-visible mount
           time. Removing it would force `region` to a `'TW'`
           fallback (see `gameImageUrl` for the same defensive
           pattern); the gate is the cleaner option here because
           the dialog can't usefully open without a region anyway. -->
      <GameList
        v-if="auth.session"
        v-model:visible="gameListVisible"
        :region="auth.session.region"
        @select="handleGameSelected"
      />

      <!-- D8g: Unconnected-game Add Account dialog. Mounted alongside
           the regular `<AddServiceAccount />` above; `handleAddAccount`
           dispatches between the two on `game.isUnconnectedGame`.
           The `created` event refreshes the row list so the new
           account appears immediately. -->
      <UnconnectedGameAddAccount
        v-model:visible="unconnectedAddVisible"
        @created="handleUnconnectedAccountCreated"
      />

      <!-- D8h: Unconnected-game Change Password dialog. Driven by the
           per-row Change Password menu item (which is itself
           `v-if`-gated on `game.isUnconnectedGame`). The `accountIndex`
           prop is the row's 0-based index in `account.serviceAccounts`,
           captured at menu-trigger time so a downstream selection /
           reorder can't misroute the change-password POST. -->
      <UnconnectedGameChangePassword
        v-model:visible="changePasswordVisible"
        :account-index="changePasswordAccountIndex"
        @verify-code-sent="handleChangePasswordSent"
      />

      <!-- OTP section (D5: REAL Get OTP / clipboard / auto-paste flow) -->
      <section class="account-list__otp bf-glass-panel">
        <header class="account-list__otp-header">
          <h2 class="account-list__otp-title">{{ t('accountList.otpHeading') }}</h2>
          <!--
            D5: bind via :model-value + @change (not v-model) so the
            persistence + first-time-AutoPasteTip side effects only run
            on the user's gesture, not on the setup-time hydration of
            the local ref from configStore.
          -->
          <el-checkbox
            :model-value="autoPaste"
            size="small"
            data-test="account-list-auto-paste"
            @change="handleAutoPasteToggle"
          >
            {{ t('accountList.autoPaste') }}
          </el-checkbox>
        </header>
        <div class="account-list__otp-row">
          <div class="account-list__otp-input">
            <input
              type="text"
              readonly
              :value="otpValue"
              :placeholder="t('accountList.otpPlaceholder')"
              class="account-list__otp-field"
              data-test="account-list-otp-field"
            />
            <button
              type="button"
              class="account-list__otp-copy"
              :title="t('accountList.copyOtp')"
              :disabled="!otpValue"
              data-test="account-list-otp-copy"
              @click="handleCopyOtp"
            >
              <el-icon><DocumentCopy /></el-icon>
            </button>
          </div>
          <button
            type="button"
            class="bf-btn-gradient account-list__otp-get"
            :disabled="gettingOtp"
            data-test="account-list-otp-get"
            @click="handleGetOtp"
          >
            {{ gettingOtp ? t('GettingOtp') : t('GetOtp') }}
          </button>
        </div>
      </section>
    </div>
  </main>
</template>

<style scoped>
.account-list {
  box-sizing: border-box;
  min-height: 100vh;
  padding: 2.5rem 1.5rem;
  display: flex;
  justify-content: center;
}

.account-list__container {
  width: 100%;
  max-width: 560px;
  display: flex;
  flex-direction: column;
  gap: 1rem;
}

/* --------------- header --------------- */

.account-list__header {
  text-align: left;
  margin-bottom: 0.25rem;
}

.account-list__title {
  margin: 0;
  font-size: 1.625rem;
  font-weight: 800;
  letter-spacing: -0.01em;
}

.account-list__subline {
  margin: 0.25rem 0 0;
  font-size: 0.8125rem;
  color: var(--bf-on-surface-variant);
}

/* --------------- game info bar --------------- */

.account-list__game {
  padding: 1rem;
  display: flex;
  flex-direction: column;
  gap: 0.875rem;
}

.account-list__game-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 1rem;
}

.account-list__game-meta {
  display: flex;
  align-items: center;
  gap: 0.75rem;
  min-width: 0;
}

.account-list__game-icon {
  width: 44px;
  height: 44px;
  border-radius: var(--bf-radius-button);
  background: linear-gradient(
    135deg,
    color-mix(in srgb, var(--bf-primary-container) 30%, transparent),
    color-mix(in srgb, var(--bf-primary) 25%, transparent)
  );
  color: var(--bf-on-primary);
  display: grid;
  place-items: center;
  flex-shrink: 0;
  box-shadow: var(--bf-shadow-card);
  overflow: hidden;
}

.account-list__game-icon-img {
  width: 100%;
  height: 100%;
  object-fit: cover;
  display: block;
}

.account-list__game-info {
  appearance: none;
  background: transparent;
  border: 0;
  padding: 0;
  text-align: left;
  cursor: pointer;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 0.125rem;
  color: var(--bf-on-surface);
  transition: color var(--bf-motion-fast);
}

.account-list__game-info:hover {
  color: var(--bf-primary);
}

.account-list__game-name {
  font-size: 1rem;
  font-weight: 700;
  line-height: 1.2;
}

.account-list__game-status {
  font-size: 0.75rem;
  color: var(--bf-on-surface-variant);
  display: inline-flex;
  align-items: center;
  gap: 0.375rem;
}

.account-list__game-status-dot {
  width: 0.5rem;
  height: 0.5rem;
  border-radius: var(--bf-radius-pill);
  background: var(--bf-success);
  display: inline-block;
}

.account-list__game-actions {
  display: flex;
  gap: 0.375rem;
  flex-shrink: 0;
}

.account-list__icon-btn {
  width: 32px;
  height: 32px;
  display: grid;
  place-items: center;
}

.account-list__icon-btn--danger {
  color: var(--bf-danger);
}

.account-list__start-btn {
  width: 100%;
  padding: 0.75rem 1rem;
  font-weight: 700;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 0.5rem;
}

/* --------------- quick actions --------------- */

.account-list__quick {
  display: flex;
  gap: 0.5rem;
}

.account-list__balance {
  flex: 1;
  padding: 0.625rem 0.75rem;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.5rem;
  min-width: 0;
}

.account-list__balance-text {
  display: flex;
  flex-direction: column;
  min-width: 0;
}

.account-list__balance-label {
  font-size: 0.625rem;
  text-transform: uppercase;
  letter-spacing: 0.06em;
  color: var(--bf-on-surface-variant);
  font-weight: 600;
}

.account-list__balance-value {
  font-size: 0.9375rem;
  font-weight: 700;
  color: var(--bf-on-surface);
}

.account-list__balance-refresh {
  appearance: none;
  background: transparent;
  border: 0;
  cursor: pointer;
  color: var(--bf-on-surface-variant);
  padding: 0.25rem;
  border-radius: var(--bf-radius-input);
  transition: color var(--bf-motion-fast);
}

.account-list__balance-refresh:hover {
  color: var(--bf-primary);
}

.account-list__balance-refresh:disabled {
  cursor: progress;
  opacity: 0.65;
}

.account-list__balance-refresh--spinning .el-icon {
  animation: account-list-spin 0.9s linear infinite;
}

@keyframes account-list-spin {
  from {
    transform: rotate(0deg);
  }
  to {
    transform: rotate(360deg);
  }
}

.account-list__quick-link {
  width: 76px;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 0.25rem;
  padding: 0.5rem;
  cursor: pointer;
  font-size: 0.6875rem;
  color: var(--bf-on-surface-variant);
  transition: color var(--bf-motion-fast);
}

.account-list__quick-link:hover {
  color: var(--bf-primary);
}

/* --------------- list section --------------- */

.account-list__list {
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.account-list__list-header {
  padding: 0.875rem 1rem;
  display: flex;
  align-items: center;
  justify-content: space-between;
  border-bottom: 1px solid rgba(255, 255, 255, 0.4);
}

.account-list__list-title {
  margin: 0;
  font-size: 0.875rem;
  font-weight: 700;
  color: var(--bf-on-surface);
}

.account-list__list-count {
  font-size: 0.75rem;
  font-weight: 600;
  color: var(--bf-on-surface-variant);
  background: var(--bf-surface-container);
  padding: 0.1875rem 0.5rem;
  border-radius: var(--bf-radius-input);
}

.account-list__list-body {
  flex: 1;
  overflow-y: auto;
  max-height: 300px;
  padding: 0.5rem;
  display: flex;
  flex-direction: column;
  gap: 0.25rem;
  min-height: 80px;
}

.account-list__list-state {
  margin: auto;
  padding: 1.5rem 1rem;
  text-align: center;
  font-size: 0.8125rem;
  color: var(--bf-on-surface-variant);
}

.account-list__list-state--error {
  color: var(--bf-danger);
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 0.625rem;
}

.account-list__rows {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 0.25rem;
}

.account-list__row {
  display: flex;
  align-items: center;
  gap: 0.625rem;
  padding: 0.625rem 0.75rem;
  border-radius: var(--bf-radius-card);
  cursor: pointer;
  position: relative;
  transition:
    background-color var(--bf-motion-fast),
    box-shadow var(--bf-motion-fast);
}

.account-list__row:hover {
  background: var(--bf-surface-container-low);
}

.account-list__row--selected {
  background: var(--bf-surface-container-low);
  box-shadow:
    inset 3px 0 0 var(--bf-primary),
    0 2px 8px color-mix(in srgb, var(--bf-primary) 8%, transparent);
}

.account-list__row--banned {
  opacity: 0.6;
  cursor: not-allowed;
}

.account-list__row-grip {
  color: var(--bf-outline-variant);
  font-size: 0.875rem;
  user-select: none;
  cursor: grab;
  letter-spacing: -0.15em;
}

.account-list__row-grip:active {
  cursor: grabbing;
}

/*
 * D7 mockup deviation: the mockup shows `cursor: not-allowed` on
 * banned rows, but Q9 (pre-flight decision) keeps banned rows
 * draggable to mirror WPF behaviour. The grip cursor stays `grab`
 * even on banned rows so the drag affordance is visually
 * consistent — the row body itself remains `not-allowed` via
 * `.account-list__row--banned` above.
 */

/*
 * Vuedraggable ghost-class — applied to the placeholder element
 * Sortable.js inserts at the projected drop location during drag.
 * Subdued styling so the ghost reads as "destination" rather
 * than competing with the live row being dragged.
 */
.account-list__row--ghost {
  opacity: 0.4;
  background: color-mix(in srgb, var(--bf-primary) 12%, var(--bf-surface-variant));
  border: 1px dashed color-mix(in srgb, var(--bf-primary) 50%, transparent);
}

.account-list__row-num {
  width: 28px;
  height: 28px;
  border-radius: var(--bf-radius-pill);
  background: var(--bf-surface-variant);
  color: var(--bf-on-surface);
  font-weight: 700;
  font-size: 0.8125rem;
  display: grid;
  place-items: center;
  flex-shrink: 0;
}

.account-list__row--selected .account-list__row-num {
  background: linear-gradient(135deg, var(--bf-primary-container), var(--bf-primary));
  color: var(--bf-on-primary);
  box-shadow: var(--bf-shadow-card);
}

.account-list__row-info {
  flex: 1;
  min-width: 0;
}

.account-list__row-name {
  margin: 0;
  font-size: 0.875rem;
  font-weight: 600;
  color: var(--bf-on-surface);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.account-list__row--selected .account-list__row-name {
  color: var(--bf-primary);
}

.account-list__row--banned .account-list__row-name {
  text-decoration: line-through;
  font-style: italic;
  color: var(--bf-on-surface-variant);
}

.account-list__row-sub {
  margin: 0.0625rem 0 0;
  font-size: 0.6875rem;
  color: var(--bf-on-surface-variant);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.account-list__row--banned .account-list__row-sub {
  color: var(--bf-danger);
}

.account-list__row-more {
  appearance: none;
  background: transparent;
  border: 0;
  cursor: pointer;
  padding: 0.25rem;
  border-radius: var(--bf-radius-input);
  color: var(--bf-on-surface-variant);
  opacity: 0;
  transition:
    opacity var(--bf-motion-fast),
    background var(--bf-motion-fast);
}

.account-list__row:hover .account-list__row-more,
.account-list__row--selected .account-list__row-more {
  opacity: 1;
}

.account-list__row-more:hover {
  background: var(--bf-surface-variant);
}

.account-list__list-footer {
  padding: 0.625rem 0.75rem;
  border-top: 1px solid rgba(255, 255, 255, 0.4);
  background: rgba(255, 255, 255, 0.3);
}

.account-list__add-btn {
  width: 100%;
  padding: 0.5rem 0.75rem;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 0.375rem;
  font-size: 0.8125rem;
  font-weight: 600;
  color: var(--bf-primary);
  background: transparent;
  border: 1px dashed color-mix(in srgb, var(--bf-primary) 35%, transparent);
  border-radius: var(--bf-radius-button);
  cursor: pointer;
  transition:
    border-color var(--bf-motion-fast),
    background var(--bf-motion-fast);
}

.account-list__add-btn:hover {
  border-color: var(--bf-primary);
  background: color-mix(in srgb, var(--bf-primary) 6%, transparent);
}

/* --------------- OTP section --------------- */

.account-list__otp {
  padding: 1rem;
  display: flex;
  flex-direction: column;
  gap: 0.625rem;
}

.account-list__otp-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.account-list__otp-title {
  margin: 0;
  font-size: 0.875rem;
  font-weight: 700;
  color: var(--bf-on-surface);
}

.account-list__otp-row {
  display: flex;
  gap: 0.5rem;
  align-items: stretch;
}

.account-list__otp-input {
  flex: 1;
  position: relative;
  display: flex;
  align-items: center;
}

.account-list__otp-field {
  width: 100%;
  background: rgba(226, 226, 226, 0.3);
  border: 0;
  border-bottom: 2px solid var(--bf-outline-variant);
  font-family: 'JetBrains Mono', 'Consolas', ui-monospace, monospace;
  font-size: 1.125rem;
  letter-spacing: 0.2em;
  text-align: center;
  padding: 0.5rem 2rem 0.5rem 0.5rem;
  border-radius: var(--bf-radius-input) var(--bf-radius-input) 0 0;
  color: var(--bf-on-surface);
  cursor: default;
  transition: border-color var(--bf-motion-fast);
}

.account-list__otp-field:focus {
  outline: none;
  border-bottom-color: var(--bf-primary);
}

.account-list__otp-copy {
  position: absolute;
  right: 0.5rem;
  appearance: none;
  background: transparent;
  border: 0;
  cursor: pointer;
  color: var(--bf-on-surface-variant);
  padding: 0.25rem;
  transition: color var(--bf-motion-fast);
}

.account-list__otp-copy:hover:not(:disabled) {
  color: var(--bf-primary);
}

.account-list__otp-copy:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}

.account-list__otp-get {
  min-width: 100px;
  padding: 0 1rem;
  font-size: 0.875rem;
  font-weight: 700;
  display: inline-flex;
  align-items: center;
  justify-content: center;
}
</style>
