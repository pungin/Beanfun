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
 * | Drag handle (reorder)                               | **REAL since P12.2 D7** (SortableJS + `Config.xml` `AccountOrder_<gameCode>` persistence) |
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

import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useRouter } from 'vue-router'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
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
  Iphone,
  Key,
  Message,
  MoreFilled,
  Operation,
  Plus,
  Refresh,
  Service,
  Setting,
  SwitchButton,
  User,
  VideoPlay,
  Wallet,
} from '@element-plus/icons-vue'
import Sortable from 'sortablejs'

import { useAuthStore } from '../stores/auth'
import { useAccountStore } from '../stores/account'
import { useConfigStore } from '../stores/config'
import { useGameStore, gameCodeOf, imageUrl } from '../stores/game'
import { useUiStore } from '../stores/ui'
import { commands, type LoginRegion, type ServiceAccount } from '../types/bindings'
import {
  CommandInvocationError,
  safeInvoke,
  surfaceCommandError,
  wrapCommand,
} from '../services/invoke'
import { useInAppBrowser } from '../composables/useInAppBrowser'
import AddServiceAccount from '../windows/AddServiceAccount.vue'
import ChangeServiceAccountDisplayName from '../windows/ChangeServiceAccountDisplayName.vue'
import CopyBox from '../windows/CopyBox.vue'
import GameList from '../windows/GameList.vue'
import ServiceAccountInfo from '../windows/ServiceAccountInfo.vue'
import ToolsDialogStack from '../windows/ToolsDialogStack.vue'
import UnconnectedGameAddAccount from '../windows/UnconnectedGame_AddAccount.vue'
import UnconnectedGameChangePassword from '../windows/UnconnectedGame_ChangePassword.vue'
import { TOOLS_GAME_CODES } from '../constants/tools'
import {
  CLASSIC_ELIGIBLE_GAME_CODES,
  CLASSIC_LAUNCHED_EVENT,
  CLASSIC_FAILED_EVENT,
  CLASSIC_TIMEOUT_EVENT,
} from '../constants/classic'
import TitleBar from '../components/TitleBar.vue'
import { useGameLauncher } from '../composables/useGameLauncher'

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
const ui = useUiStore()

/*
 * One-shot flag: auto-start game on the first successful loadList
 * after mount. Cleared after first use so retry / game-switch
 * don't re-trigger.
 */
let pendingAutoStart = true
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
/*
 * P12.4 followup-A D4 — game-launch chain extracted into the
 * `useGameLauncher` composable so `pages/IdPassForm.vue` and
 * `pages/QrForm.vue` can share the same `runGame(account, password)`
 * pipeline (WPF `MainWindow.runGame` is called from both LoginPage
 * forms via `btn_StartGame_Click` and from AccountList via
 * `Button_Click`). The composable keeps every WPF parity behaviour
 * (resolveGamePath / pathHasWideChar / process check / mode resolve
 * / final spawn) — see its docblock for the WPF source line table.
 */
const launcher = useGameLauncher()

/* --------------- service-account list state --------------- */

type LoadState = 'loading' | 'ready' | 'error'

/*
 * #263: Initialize loadState based on whether data is already present
 * from a login-form prefetch. This eliminates the "blank loading state"
 * gap when navigating from login forms that already called getServiceAccounts.
 * Mirrors WPF behaviour where `BeanfunClient.Login` populates `accountList`
 * before navigation, so `redrawSAccountList` renders immediately.
 */
const loadState = ref<LoadState>(account.serviceAccounts.length > 0 ? 'ready' : 'loading')
/**
 * Backend-supplied error message kept alongside `loadState` so the
 * inline banner can surface the actual server reason without the
 * user having to re-trigger the auto-dismissed toast that
 * `wrapCommand` already emitted.
 */
const loadError = ref<string | null>(null)

async function loadList(): Promise<void> {
  /*
   * #263: Only show loading state if there's no data yet. If the store
   * already has accounts (from login-form prefetch), keep showing the
   * existing data while the refresh runs in the background.
   */
  if (account.serviceAccounts.length === 0) {
    loadState.value = 'loading'
  }
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

    // Auto-select the first enabled account after load — mirrors WPF
    // which defaults SelectedIndex to 0 on `redrawSAccountList`.
    if (!account.selectedSid && account.serviceAccounts.length > 0) {
      const first = account.serviceAccounts.find((a) => a.is_enable)
      if (first) account.selectedSid = first.sid
    }

    loadState.value = 'ready'

    // Trigger window resize after account list renders — fitWindow
    // needs to re-measure now that the DOM has new rows.
    void nextTick(() => window.dispatchEvent(new Event('resize')))

    // Auto-start game after first load if the setting is enabled.
    // Mirrors WPF `loginWorker_RunWorkerCompleted` L1421-1429.
    // Only fires once per mount (not on retry / game switch).
    if (pendingAutoStart) {
      pendingAutoStart = false
      if (ui.autoStartGame && account.serviceAccounts.length > 0) {
        void handleStartGame()
      }
    }
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

/*
 * #263: Maximum number of account rows visible before scrolling kicks in.
 * Keep in sync with the max-height calculation in .account-list__list-body CSS.
 */
const MAX_VISIBLE_ROWS = 5

/*
 * #263: Show a floating scroll indicator when there are more accounts
 * than can fit in the visible area. The indicator uses an SVG chevron
 * that fades in/out based on scroll position, helping users discover
 * that the list is scrollable.
 */
const showScrollIndicator = computed(() => accountCount.value > MAX_VISIBLE_ROWS)

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

/**
 * Double-click on a row → copy the row's service-account ID (`sid`)
 * to the clipboard. Mirrors WPF `lstViewAccount_MouseDoubleClick`
 * (`AccountList.xaml.cs`), whose only side effect was
 * `Clipboard.SetText(selected.sid)` — paired with the WPF tooltip
 * resource `DoubleClickCopy` ("雙擊複製帳號 / Double-click to copy
 * account") which is still shipped via the `DoubleClickCopy` i18n
 * key today. Banned rows fall through silently (same guard as
 * {@link selectRow}) so the SPA mirrors WPF's "disabled rows ignore
 * input" behaviour.
 *
 * Selection is intentionally **not** armed here: WPF treats the
 * double-click as a one-shot copy, not a row-arm; mutating
 * `selectedSid` would also flip the OTP / Start Game UI on top of
 * the copy, which the WPF user never saw. Single-click + Enter is
 * the ergonomic path for OTP fetch (issue #239 fixes the missing
 * keyboard route too).
 *
 * The SPA bypasses the {@link clipboardWriteOtp} helper because that
 * helper's success / failure toasts use the OTP-specific
 * `GetOtpSuccessAndCopy` string. A generic `CopyFinished` /
 * `CopyFailed` toast pair matches the WPF tooltip wording and keeps
 * the OTP helper single-purpose.
 */
async function handleRowDblClick(a: ServiceAccount): Promise<void> {
  if (!a.is_enable) return
  try {
    await navigator.clipboard.writeText(a.sid)
    ElMessage.success(t('CopyFinished'))
  } catch {
    /*
     * `navigator.clipboard.writeText` only rejects on permission /
     * trust-boundary denial inside our Tauri webview — surface it
     * so the user knows the buffer wasn't updated (otherwise
     * they'd silently paste stale content into the launcher).
     */
    ElMessage.error(t('CopyFailed'))
  }
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
  game.clearGameData()
  await router.push('/login')
}

/* --------------- real handler wire-ups --------------- */

/*
 * P12.3 D8: `handleStartGame`, `handleChangeGame`, `handleAddAccount`,
 * and `handleChangePassword` are the **real** wire-ups for the game
 * switcher + start-game + add-account + change-password flows
 * (defined further down in the file). `handleTools` is REAL since
 * P12.5 D7 — see {@link handleTools} below for the wiring.
 *
 * `handleMemberCenter` / `handleCustomerService` are REAL since
 * P12.4-followup-B-fix F8/F9 — see the
 * {@link handleMemberCenter} / {@link handleCustomerService}
 * docblocks below for the WPF parity wiring. F8/F9 also retired
 * the previous `makeStub` helper that only those two handlers
 * used; the file no longer contains any placeholder handlers.
 */

/* --- P12.4-followup-B-fix F8/F9 — Member Center / Customer Service --- */

/**
 * Per-region landing URL for the Customer Service portal. Mirrors
 * WPF `Pages/AccountList.xaml.cs::btn_Customerservice_Click`
 * L190-202 byte-for-byte:
 *
 * - **TW** → `https://tw.beanfun.com/customerservice/www/main.aspx`
 * - **HK** → `https://bfweb.hk.beanfun.com/newfaq/service_newBF.aspx`
 *
 * Frozen at module scope so the test surface can grep the
 * literal URL strings without re-mounting; both targets are
 * inside the `*.beanfun.com` allowlist (see
 * `src-tauri/src/commands/web_browser.rs::is_allowed_host`) so
 * the in-app webview path always wins — no system-browser
 * fallback expected in production.
 *
 * # Why frontend-built (vs. backend command like Member Center)
 *
 * No `web_token` interpolation is needed (WPF passes a static URL
 * with no session secret in either region), so we keep the URL
 * shape co-located with the click handler and dispatch through
 * the shared `useInAppBrowser` composable. This preserves SRP:
 * the new `openMemberCenterBrowser` backend command exists
 * specifically because of the secret-confinement constraint;
 * Customer Service has no equivalent reason to grow a dedicated
 * IPC command just to interpolate a region into a static string.
 */
const CUSTOMER_SERVICE_URLS: Readonly<Record<LoginRegion, string>> = Object.freeze({
  TW: 'https://tw.beanfun.com/customerservice/www/main.aspx',
  HK: 'https://bfweb.hk.beanfun.com/newfaq/service_newBF.aspx',
})

/**
 * Shared in-app-browser dispatch. Single instance per component
 * setup matches the composable's stateless contract — see
 * `composables/useInAppBrowser.ts`.
 */
const inAppBrowser = useInAppBrowser()

/**
 * Open the Beanfun **Member Center** in a dedicated in-app webview
 * window. Mirrors WPF `Pages/AccountList.xaml.cs::BF_btnMember_Click`.
 *
 * The URL is built **backend-side** (see
 * `commands::web_browser::open_member_center_browser`) because it
 * embeds the session's `web_token` — a server-side secret that
 * must never cross IPC (`commands::dto`'s sentinel test enforces
 * this invariant). The frontend therefore invokes the dedicated
 * command with no arguments instead of going through
 * `useInAppBrowser` (which expects a frontend-supplied URL).
 *
 * Errors:
 *
 * - `auth.session_required` — backend reports no active session.
 *   Defence-in-depth: the AccountList page is behind the auth
 *   route guard, so this branch should be unreachable unless the
 *   guard itself regresses.
 * - `ui.window_create_failed` — Tauri builder failure (rare).
 *
 * Both surface a generic toast via the shared error message;
 * the standard `wrapCommand` toast pipeline is bypassed because
 * the caller is a fire-and-forget click handler that returns
 * `Promise<void>` and would swallow a thrown
 * `CommandInvocationError`.
 */
async function handleMemberCenter(): Promise<void> {
  const result = await safeInvoke(commands.openMemberCenterBrowser())
  if (!result.ok) {
    ElMessage.error(result.error.message || t('inAppBrowser.openFailed'))
  }
}

/**
 * Open the Beanfun **Customer Service** portal in a dedicated
 * in-app webview window. Mirrors WPF
 * `Pages/AccountList.xaml.cs::btn_Customerservice_Click`
 * L190-202.
 *
 * URL is selected from {@link CUSTOMER_SERVICE_URLS} by the active
 * session's `region`. If no session is active (defence in depth —
 * the route guard makes this unreachable), surfaces the generic
 * `inAppBrowser.openFailed` toast and skips the IPC round-trip.
 *
 * Dispatched through {@link inAppBrowser}.open which handles the
 * cookie-seeded in-app window + the system-browser fallback. The
 * fallback path is theoretically unreachable here because both
 * URLs are inside the backend allowlist; left wired so a future
 * accidental URL drift surfaces as a system-browser open rather
 * than a silent failure.
 */
async function handleCustomerService(): Promise<void> {
  const region = auth.session?.region
  if (!region) {
    ElMessage.error(t('inAppBrowser.openFailed'))
    return
  }
  await inAppBrowser.open(CUSTOMER_SERVICE_URLS[region])
}

/* --- Gash Recharge / App Gash Recharge (WPF bfb_Gash_Click + btn_Deposite_Click parity) --- */

/**
 * Open the Beanfun **Gash recharge** page. Mirrors WPF
 * `Pages/AccountList.xaml.cs::bfb_Gash_Click`.
 *
 * Same pattern as {@link handleMemberCenter} — the URL embeds
 * `web_token`, so a dedicated backend command keeps the secret
 * confined to Rust.
 */
async function handleGashRecharge(): Promise<void> {
  const result = await safeInvoke(commands.openGashRechargeBrowser())
  if (!result.ok) {
    ElMessage.error(result.error.message || t('inAppBrowser.openFailed'))
  }
}

/**
 * Open the **beanfun! App Gash deposit** page. Mirrors WPF
 * `Pages/AccountList.xaml.cs::btn_Deposite_Click`.
 *
 * No `web_token` needed — the URL is a static `m.beanfun.com`
 * endpoint. Dispatched through {@link inAppBrowser} (same pattern
 * as {@link handleCustomerService}).
 */
async function handleAppGashRecharge(): Promise<void> {
  await inAppBrowser.open('https://m.beanfun.com/Deposite')
}

/* --------------- P12.5 D7 — Tools button real handler --------------- */

/**
 * Imperative handle to the {@link ToolsDialogStack} mounted at the
 * bottom of the template. The wrapper component owns all four
 * Tools-related dialog mounts (MapleTools / KartTools /
 * EquipCalculator / CoreCalculator; the in-app browser is no
 * longer a dialog — followup-B B7 routes it through
 * `useInAppBrowser` → native `WebviewWindow`) and exposes a
 * single `openForGame(gameCode)` method we call from
 * {@link handleTools}.
 *
 * # Why a `ref` (and not e.g. emit-up + parent-owned visibility)
 *
 * The dialog stack is opened by a one-shot click on the Tools
 * toolbar button — it has no companion `v-model` state on the
 * AccountList page that needs to round-trip the dialog's open
 * state back. Holding the wrapper imperatively keeps the parent
 * page's reactive surface focused on AccountList-specific state
 * (selected service account, OTP, etc.) and avoids polluting
 * this 2.7 kLoC SFC with five extra `ref<boolean>`s + their
 * matching props plumbing. See `windows/ToolsDialogStack.vue`
 * top docblock for the full design discussion.
 */
const toolsDialogRef = ref<InstanceType<typeof ToolsDialogStack> | null>(null)

/**
 * Tools toolbar button click handler. Mirrors WPF
 * `AccountList.xaml.cs::btn_Tools_Click` (L237-250) — dispatches
 * to MapleTools or KartTools based on the active gameCode.
 *
 * # Why guard on `selectedGameCode === null`
 *
 * The template gates the button with `v-if="showToolsButton"`
 * which is itself `false` whenever `selectedGameCode === null`
 * (see {@link showToolsButton}), so in practice the click can't
 * fire without a selected game. The defensive null-check is
 * cheap insurance against a future refactor that detaches the
 * gate from the click handler — the WPF switch falls through to
 * a no-op for any non-tools code, and the SPA's null branch
 * matches that "do nothing" semantics.
 *
 * # Why `void`-prefix on the call
 *
 * `openForGame` is async (it awaits `commands.detectGamePath` to
 * resolve the MapleStory game path before opening MapleTools).
 * We don't need the click handler to wait for the resolution —
 * the dialog opens after the await completes; surfacing a
 * Promise back to the click handler would require either
 * `async` propagation (unnecessary surface area) or an
 * error-boundary toast for a backend lookup that has its own
 * graceful empty-string fallback. Fire-and-forget matches the
 * WPF synchronous `new MapleTools().Show()` shape closely
 * enough for parity.
 */
function handleTools(): void {
  const code = game.selectedGameCode
  if (code === null) return
  void toolsDialogRef.value?.openForGame(code)
}

/* --------------- P12.4 D6 — Settings / About header navigation --------------- */

/**
 * Push the SPA into the Settings page. Mirrors WPF
 * `MainWindow::btn_Setting_Click` (L1071-1078) which navigates the
 * top-level frame to `Settings.xaml`. The route is `requiresAuth:
 * false` (see `router/index.ts`), so the same handler also works
 * if the AccountList is reached via a future direct deep-link from
 * an unauthenticated state — but in steady state this button is
 * only mounted on the post-login AccountList.
 *
 * No `await` on `router.push` because the caller is a click
 * handler that doesn't need to know when the navigation resolves;
 * any rejection would be a vue-router internal error and is
 * already logged by the global error handler in `main.ts`.
 */
function handleOpenSettings(): void {
  void router.push('/settings')
}

/**
 * Push the SPA into the About page. Mirrors WPF
 * `MainWindow::btn_About_Click` (L1057-1064). Same `void` /
 * fire-and-forget rationale as {@link handleOpenSettings}.
 */
function handleOpenAbout(): void {
  void router.push('/about')
}

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
 * The set is hard-coded in WPF (no server-side configuration). We
 * import {@link TOOLS_GAME_CODES} from `src/constants/tools.ts` —
 * P12.5 D7 promoted the literal to a shared module so
 * `pages/Settings.vue` can consume the same gate (WPF L621 /
 * L630-633 applies the identical predicate to the Settings-page
 * Tools button). See the constants module docblock for the
 * "visibility set vs routing partition" rationale.
 */
const showToolsButton = computed<boolean>(() => {
  if (game.selectedGameCode === null) return false
  return TOOLS_GAME_CODES.has(game.selectedGameCode)
})

/* --------------- MapleStory Classic (懷舊服) --------------- */

/**
 * Show the Classic launch button only when (a) the selected game is
 * MapleStory and (b) this session can actually drive the galaxy SSO —
 * an HK session (account/password) or a TW GamaPass session. A TW
 * account/password or QR session has no portal-side identity, so the
 * classic portal would just dead-end at its login page.
 */
const showClassicButton = computed<boolean>(() => {
  const code = game.selectedGameCode
  if (code === null || !CLASSIC_ELIGIBLE_GAME_CODES.has(code)) return false
  return auth.session?.region === 'HK' || auth.viaGamepass
})

/** Re-entry guard: one classic launch in flight at a time. */
const classicLaunching = ref(false)

/**
 * Kick off the hidden galaxy SSO → NGM launch
 * (`commands.openClassicLogin`). Outcome arrives asynchronously via
 * the `classic-*` Tauri events registered in `onMounted`; the guard is
 * released there (or on invoke error).
 */
async function handleStartClassic(): Promise<void> {
  if (classicLaunching.value) return
  classicLaunching.value = true
  ElMessage.info(t('classic.launching'))
  const result = await safeInvoke(commands.openClassicLogin())
  if (!result.ok) classicLaunching.value = false
}

/** `listen()` handles for the classic launch events, torn down on unmount. */
const classicUnlistenFns: UnlistenFn[] = []

async function registerClassicListeners(): Promise<void> {
  // try/catch: `listen` needs the Tauri IPC bridge; in jsdom specs that
  // don't stub `@tauri-apps/api/event` (or any IPC-less environment) it
  // rejects — the page must still mount, just without the toasts.
  try {
    const launched = await listen(CLASSIC_LAUNCHED_EVENT, () => {
      classicLaunching.value = false
      ElMessage.success(t('classic.launched'))
    })
    const failed = await listen(CLASSIC_FAILED_EVENT, () => {
      classicLaunching.value = false
      ElMessage.warning(t('classic.launchFailed'))
    })
    const timedOut = await listen(CLASSIC_TIMEOUT_EVENT, () => {
      classicLaunching.value = false
      ElMessage.warning(t('classic.launchTimeout'))
    })
    classicUnlistenFns.push(launched, failed, timedOut)
  } catch (e) {
    console.warn('[account-list] classic event listeners unavailable', e)
  }
}

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
  /*
   * P12.4 followup-A D5 — also persist the active game's INI
   * snapshot so `pages/IdPassForm.vue` / `pages/QrForm.vue`
   * GameStart buttons (WPF `btn_StartGame_Click`) can launch the
   * same game from the LoginPage even after `clearGameData` wiped
   * the in-memory game store on logout / session-expired.
   *
   * Mirrors WPF MainWindow instance state lifetime — the
   * `game_exe` / `game_commandLine` / `game.dir_value_name` /
   * `game.dir_reg` / `win_class_name` fields survive a logout
   * because they live on the `MainWindow` instance, not the
   * session. They only reset on full process restart.
   *
   * Stored as JSON because `useConfigStore` is string-only K-V;
   * `game.restoreLastSelected()` (D6) is the symmetric read path
   * used by `useGameLauncher` consumers running outside an
   * authenticated session. Existing `loginGame` config key (WPF
   * `Config.xml::loginGame` — `setupGameOnMount` source of
   * truth, L641) is reused for the gameCode itself; only the
   * INI body needs the new key.
   *
   * Soft-fails if `selectedIni` is somehow null (cold mount
   * race window). The launcher's `restoreLastSelected` will
   * then return `false` and the LoginForm GameStart button will
   * surface `GameSelected` toast — same fallback as WPF when
   * `service_code` is empty.
   */
  const activeIni = game.selectedIni
  if (activeIni !== null) {
    await configStore.set('lastSelectedIni', JSON.stringify(activeIni))
  }

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
    auth.updateSessionService(serviceCode, serviceRegion)
  }

  /*
   * Clear stale per-account selection so the OTP / Start Game
   * affordances don't carry over a sid that is unlikely to be
   * present in the new game's account list. Mirrors WPF
   * `redrawSAccountList`'s `SelectedIndex = -1` reset.
   */
  account.selectedSid = null
  // Issue #300: the new game has a different account set — drop the
  // per-account OTP cache so a stale token can't surface against a
  // same-named sid in another game.
  otpCache.value.clear()

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
  /*
   * Fast path — "navigate back from Settings / About" UX fix.
   *
   * `AccountList` is a regular routed component, so visiting
   * `/settings` or `/about` unmounts it and visiting back
   * remounts it. Without this skip, every return would re-run
   * the full bootstrap (`game.loadGames` + `selectActiveGame` +
   * `loadList`) and the user would see the spinner + an empty
   * list flash before the data refilled. Users with custom
   * drag-and-drop sort orders read that flash as "my order was
   * reset" because the persistent CSV (`AccountOrder_<code>_<region>`)
   * is only re-applied AFTER the HTTP response lands.
   *
   * Skipping is safe only when the cached account list still
   * belongs to the same game the UI says is active. A routed
   * remount can preserve `game.selectedGameCode` from Config.xml /
   * previous selection even while the backend session is still
   * pinned to the login default; in that mismatch shape we must
   * fall through to `selectActiveGame` so `setActiveService`
   * runs before the next account-list fetch.
   *
   * Every code path that *should* cause a re-fetch either clears
   * `serviceAccounts` first or leaves selectedGame/session
   * intentionally mismatched:
   *
   * - logout / session expired → `clearAccountSession` resets
   *   the store (router guard + `registerSessionExpiredHandler`).
   * - change game → `setActiveService` clears the store before
   *   the new fetch.
   * - first cold mount after login → store is empty by
   *   construction, so the predicate is false and the full
   *   bootstrap runs.
   *
   * `loadState` would otherwise sit at its initial `'loading'`
   * sentinel forever (no `loadList` to flip it to `'ready'`),
   * so we flip it inline before returning.
   */
  const session = auth.session
  const selectedGameCode = game.selectedGameCode
  const sessionGameCode =
    session !== null ? gameCodeOf(session.service_code, session.service_region) : null
  const cachedAccountsMatchSession =
    selectedGameCode === null || selectedGameCode === sessionGameCode

  if (account.serviceAccounts.length > 0 && session !== null && cachedAccountsMatchSession) {
    loadState.value = 'ready'
    return
  }

  await game.loadGames()

  if (game.loadState === 'error') {
    void loadList()
    return
  }

  const saved = configStore.get('loginGame') ?? ''
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
 * Extract the numeric limit from the server notice text
 * (e.g. "此遊戲最多允許新增帳號數:2" → 2). Returns `null` when
 * no number is found or the notice is not the `other` variant.
 */
const accountLimit = computed<number | null>(() => {
  const notice = account.amountLimitNotice
  if (notice.kind !== 'other') return null
  const match = notice.data.match(/\d+/)
  return match ? parseInt(match[0], 10) : null
})

/**
 * Whether the add-account button should be disabled.
 * - `auth_re_login_required` → always disabled (need verification)
 * - `other` with parseable limit → disabled only when current
 *   account count >= the limit
 * - `none` / unparseable → enabled (let server reject if needed)
 */
const isAddAccountDisabled = computed(() => {
  const notice = account.amountLimitNotice
  if (notice.kind === 'auth_re_login_required') return true
  if (accountLimit.value !== null) {
    return account.serviceAccounts.length >= accountLimit.value
  }
  return false
})

/**
 * Human-readable limit notice text for the UI.
 * - `auth_re_login_required` → localised `AuthReLogin` string
 * - `other` → server's verbatim text (e.g. "此遊戲最多允許新增帳號數:2")
 * - `none` → null (nothing to show)
 */
const limitNoticeText = computed<string | null>(() => {
  const notice = account.amountLimitNotice
  if (notice.kind === 'auth_re_login_required') return t('AuthReLogin')
  if (notice.kind === 'other') return notice.data
  return null
})

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
  if (launchingGame.value) return true
  if (!game.selectedGame) return true
  if (startGameDirect.value) return false
  return account.selectedServiceAccount === null
})

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
const launchingGame = ref(false)

async function handleStartGame(): Promise<void> {
  if (launchingGame.value) return
  if (!game.selectedGame) {
    ElMessage.warning(t('GameSelected'))
    return
  }
  launchingGame.value = true
  try {
    if (startGameDirect.value) {
      await launcher.runGame()
      return
    }
    await handleGetOtp()
  } finally {
    launchingGame.value = false
  }
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
 * Per-sub-account OTP cache (issue #300 feature request).
 *
 * Keyed by service-account `sid` → the last OTP fetched for that
 * account this session. Lets the user toggle between sub-accounts
 * without losing an OTP they already generated: switching away and
 * back restores the cached value instead of blanking the field.
 *
 * Lifetime is intentionally session/list-scoped, not time-scoped:
 *
 * - Populated in {@link handleGetOtp} on every successful fetch.
 * - Read by the `watch(selectedSid)` below to restore on row change.
 * - Cleared when the active game changes ({@link selectActiveGame},
 *   the new game has a different account set) and on logout (the
 *   whole component unmounts).
 *
 * We deliberately do NOT expire entries here — Beanfun OTPs are
 * short-lived, but the user explicitly asked for the value to persist
 * across toggles, and the canonical "is it still valid?" answer is a
 * fresh Get OTP press. Showing the last value is strictly better than
 * blanking it, and never auto-launches off a cached token (Start Game
 * always routes through a fresh {@link handleGetOtp}).
 */
const otpCache = ref(new Map<string, string>())

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
 * Re-bind the OTP value when the user picks a different row.
 *
 * The OTP is row-bound and must never visually stay attached to a
 * different highlighted account (a mis-binding bug that's easy to
 * introduce because OTP refresh is gesture-driven, not selection-
 * driven). Issue #300: rather than unconditionally blanking, restore
 * the cached OTP for the newly-selected account when one exists, so
 * toggling between sub-accounts keeps each account's last token
 * visible. Accounts with no cached OTP (or the `null` selection)
 * fall back to an empty field.
 *
 * Logout is handled separately by `account.clearSessionData()` —
 * `selectedSid` becomes null along with everything else.
 */
watch(
  () => account.selectedSid,
  (sid) => {
    otpValue.value = (sid && otpCache.value.get(sid)) || ''
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
  // Issue #300: remember this account's OTP so switching sub-accounts
  // and back restores it instead of blanking the field.
  otpCache.value.set(target.sid, otp)

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
    await launcher.runGame(target.sid, otp)
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

/**
 * Double-click the OTP field → copy to clipboard (issue #300
 * regression). Up to 5.9.2 double-clicking the generated OTP
 * auto-copied it; the rewrite dropped that, leaving the dedicated
 * copy icon as the only path. Restore the gesture here.
 *
 * Unlike the silent copy-icon ({@link handleCopyOtp}), the
 * double-click surfaces the `GetOtpSuccessAndCopy` toast: a
 * double-click has no persistent affordance (no button highlight),
 * so explicit feedback is what tells the user the copy landed.
 */
function handleOtpDblClick(): void {
  if (!otpValue.value) return
  void clipboardWriteOtp(otpValue.value, true)
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
 * # Why SortableJS (and not native HTML5 drag-drop)
 *
 * SortableJS gives us:
 *
 * - The `handle` selector that mirrors WPF's `_isHandlePressed`
 *   gate (`MouseLeftButtonDown` only on the grip, never the row)
 *   for free, without the manual `ev.target` walking we'd need
 *   for HTML5 dnd.
 * - Sortable.js' built-in animation, ghost element, and same-list
 *   reorder semantics — re-implementing those over native dnd is
 *   ~200 lines of fiddly DOM measurement code.
 *
 * # Why the @end handler manually splices the array
 *
 * SortableJS manipulates the DOM during drag — the
 * backing data must be updated manually via `oldIndex` /
 * `newIndex` from the event. The handler splices the Pinia
 * array in place, then funnels through `setServiceAccountOrder`
 * as the canonical reorder action (keeps the spy seam for tests
 * and any future invariants like analytics or dedup).
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
 * SortableJS `@end` handler. Receives `oldIndex` / `newIndex`
 * from the event, splices the Pinia array in place, then persists.
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
function handleDragEnd(event: Sortable.SortableEvent): void {
  const key = accountOrderConfigKey.value
  if (!key) return

  const { oldIndex, newIndex, item, from } = event
  if (oldIndex == null || newIndex == null || oldIndex === newIndex) return

  /*
   * SortableJS physically moved the DOM element during drag. Revert that
   * manipulation so Vue's reactivity system is the sole owner of the DOM —
   * otherwise the reactive reorder below triggers a Vue re-render that
   * conflicts with the already-moved element, producing wrong positions.
   *
   * Best-effort: in the real WebView `item` can already be detached (Vue
   * re-rendered the row — a hover/refresh tick mid-drag — replacing the
   * node SortableJS is holding), so `removeChild` throws `NotFoundError`.
   * That MUST NOT abort the handler: the reactive reorder + Config.xml
   * persist below are the source of truth, and skipping them was silently
   * dropping the user's order on disk (it only survived in-session because
   * SortableJS had already moved the DOM). Swallow the revert failure;
   * Vue reconciles the DOM from the reordered array on the next paint.
   */
  try {
    from.removeChild(item)
    from.insertBefore(item, from.children[oldIndex] ?? null)
  } catch {
    /* node already reconciled by Vue — reactive reorder below wins */
  }

  const list = [...account.serviceAccounts]
  const [moved] = list.splice(oldIndex, 1)
  list.splice(newIndex, 0, moved)
  const orderedSids = list.map((a) => a.sid)

  account.setServiceAccountOrder(orderedSids)
  void persistAccountOrder(key, orderedSids.join(','))
}

/* SortableJS instance lifecycle */

const rowsRef = ref<HTMLElement | null>(null)
let sortableInstance: Sortable | null = null

/*
 * #263: Ref for the list body container to track scroll position
 * for the floating scroll indicator.
 */
const listBodyRef = ref<HTMLElement | null>(null)
const listScrolledToBottom = ref(false)

function initSortable(): void {
  destroySortable()
  if (!rowsRef.value) return
  sortableInstance = Sortable.create(rowsRef.value, {
    handle: '.account-list__row-grip',
    animation: 150,
    ghostClass: 'account-list__row--ghost',
    forceFallback: true,
    onEnd: handleDragEnd,
  })
}

function destroySortable(): void {
  if (sortableInstance) {
    sortableInstance.destroy()
    sortableInstance = null
  }
}

/*
 * #263: Track scroll position to toggle the scroll indicator visibility.
 * The indicator fades out when the user scrolls near the bottom.
 */
function handleListScroll(event: Event): void {
  const el = event.target as HTMLElement
  const threshold = 20 // pixels from bottom to consider "at bottom"
  listScrolledToBottom.value = el.scrollHeight - el.scrollTop - el.clientHeight < threshold
}

watch(rowsRef, (el) => {
  if (el) initSortable()
  else destroySortable()
})

onBeforeUnmount(destroySortable)

/* --------------- Enter hotkey (WPF / Beanfun B6 parity) --------------- */

/**
 * Mirrors the legacy Beanfun (B6) client: once a service account row is
 * highlighted, pressing `Enter` kicks off the same Get-OTP flow as
 * clicking the button.
 *
 * # Why a `window`-level listener (vs a row-level `@keydown`)
 *
 * Clicking a row in our layout sets `account.selectedSid` but does not
 * move DOM focus onto the `<li>` (rows are intentionally not tab stops
 * — adding `tabindex` would change tab order and leak focus rings onto
 * the whole list). Without focus on the row, a row-scoped `@keydown`
 * never fires. A window-level listener sees the Enter keystroke
 * regardless of which non-form element currently holds focus, which is
 * the same behaviour WPF `lstViewAccount.KeyDown` had on Key.Enter.
 *
 * # Guards
 *
 * 1. `event.key === 'Enter'` — only Enter, and not on synthetic repeats
 *    (holding Enter shouldn't spam-fire the OTP IPC), and not mid-IME
 *    composition (CJK users would otherwise trigger an OTP every time
 *    they commit a candidate).
 * 2. Focus inside a form control (`input` / `textarea` / `[contenteditable]`)
 *    → let the control handle its own Enter (e.g. submitting a dialog
 *    form). Our `readonly` OTP `<input>` passes this filter so Enter
 *    while it happens to hold focus after a "Copy OTP" click still
 *    routes through.
 * 3. Focus on a `<button>` — the browser will already fire a `click`
 *    for Enter on a focused button, and our handler would double-fire
 *    (once as keydown → `handleGetOtp`, once as the button's own click
 *    handler). Skip so the button-scoped behaviour wins.
 * 4. Any Element Plus overlay (`.el-overlay` — covers `ElDialog` +
 *    `ElMessageBox`, both of which render outside our page subtree via
 *    `append-to-body`) has an element focused inside → the modal owns
 *    Enter (confirm button, form submit, etc). Without this guard the
 *    "Change Alias" dialog's OK-on-Enter would also fire GetOtp on the
 *    page behind it.
 * 5. `gettingOtp.value === true` — an OTP fetch is already in flight;
 *    `handleGetOtp` would no-op anyway, but short-circuiting here
 *    skips the (harmless) `e.preventDefault()` so assistive tech
 *    isn't told we swallowed the key when we actually didn't.
 * 6. `!account.selectedSid` — nothing to fetch for. Staying silent
 *    (no toast) matches B6: pressing Enter before picking an account
 *    simply did nothing. We intentionally don't route through
 *    `handleGetOtp` in this case because its built-in "no selection"
 *    toast was written for a button click — a keyboard press that
 *    accidentally lands on an empty list shouldn't spam the user
 *    with a warning.
 */
function handleGlobalEnter(event: KeyboardEvent): void {
  if (event.key !== 'Enter') return
  if (event.isComposing || event.repeat) return

  /*
   * `event.target` can be the `Window` itself when the key is
   * dispatched without a focused element (typical on first paint
   * before the user has clicked anything). `closest` only exists on
   * `Element`, so narrow first — a non-Element target means "focus
   * is nowhere", which is exactly the case we want to forward.
   */
  const target = event.target
  if (target instanceof HTMLElement) {
    const tag = target.tagName
    if (tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT') return
    if (target.isContentEditable) return
    if (tag === 'BUTTON' || target.closest('button')) return
  }

  const active = document.activeElement as HTMLElement | null
  if (active?.closest('.el-overlay')) return

  if (gettingOtp.value) return
  if (!account.selectedSid) return

  event.preventDefault()
  void handleGetOtp()
}

onMounted(() => {
  window.addEventListener('keydown', handleGlobalEnter)
  void registerClassicListeners()
  // One-shot: the login form armed an immediate Classic launch
  // (「登入後啟動經典版」). Consume before firing so a later manual
  // navigation back here can never re-trigger it.
  if (ui.pendingClassicLaunch) {
    ui.pendingClassicLaunch = false
    void handleStartClassic()
  }
})

onBeforeUnmount(() => {
  window.removeEventListener('keydown', handleGlobalEnter)
  for (const unlisten of classicUnlistenFns) {
    try {
      unlisten()
    } catch (e) {
      console.error('[account-list] classic unlisten threw', e)
    }
  }
  classicUnlistenFns.length = 0
})
</script>

<template>
  <main class="account-list bf-glass-window" data-window-root>
    <TitleBar>
      <button
        type="button"
        class="account-list__titlebar-btn"
        :title="t('Settings')"
        data-test="account-list-settings"
        @click="handleOpenSettings"
      >
        <el-icon aria-hidden="true"><Setting /></el-icon>
      </button>
      <button
        type="button"
        class="account-list__titlebar-btn"
        :title="t('settings.aboutLink')"
        data-test="account-list-about"
        @click="handleOpenAbout"
      >
        <el-icon aria-hidden="true"><InfoFilled /></el-icon>
      </button>
    </TitleBar>
    <div class="account-list__scroll">
      <div class="account-list__container" data-window-content>
        <!-- Game bar: icon + name + start button in one compact row -->
        <section class="account-list__game bf-glass-panel">
          <div class="account-list__game-icon" aria-hidden="true">
            <img
              v-if="gameImageUrl"
              :src="gameImageUrl"
              :alt="gameNameDisplay"
              class="account-list__game-icon-img"
              data-test="account-list-game-image"
            />
            <el-icon v-else :size="20"><VideoPlay /></el-icon>
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
          <div class="account-list__game-actions">
            <button
              type="button"
              class="bf-btn-gradient account-list__start-btn"
              :disabled="startGameDisabled"
              data-test="account-list-start"
              @click="handleStartGame"
            >
              <el-icon :size="14"><VideoPlay /></el-icon>
              <span>{{ t('GameStart') }}</span>
            </button>
            <button
              v-if="showClassicButton"
              type="button"
              class="bf-btn-gradient account-list__start-btn account-list__classic-btn"
              :disabled="classicLaunching"
              :title="t('classic.buttonTitle')"
              data-test="account-list-classic"
              @click="handleStartClassic"
            >
              <el-icon :size="14"><VideoPlay /></el-icon>
              <span>{{ t('classic.button') }}</span>
            </button>
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
        </section>

        <!-- Quick actions: balance left, action buttons right -->
        <section class="account-list__quick bf-glass-panel">
          <div class="account-list__balance">
            <span class="account-list__balance-label">
              {{ t('accountList.gashBalance') }}
            </span>
            <span class="account-list__balance-value" data-test="account-list-balance-value">
              {{ formattedRemainPoint }}
            </span>
            <button
              type="button"
              class="account-list__balance-refresh"
              :class="{ 'account-list__balance-refresh--spinning': refreshing }"
              :title="t('accountList.refreshBalance')"
              :disabled="refreshing"
              data-test="account-list-refresh-balance"
              @click="handleRefreshBalance"
            >
              <el-icon :size="14"><Refresh /></el-icon>
            </button>
          </div>
          <div class="account-list__quick-actions">
            <button
              type="button"
              class="account-list__quick-btn"
              data-test="account-list-gash-recharge"
              @click="handleGashRecharge"
            >
              <el-icon :size="14"><Wallet /></el-icon>
              <span>{{ t('GashRecharge') }}</span>
            </button>
            <button
              v-if="auth.session?.region === 'TW'"
              type="button"
              class="account-list__quick-btn"
              data-test="account-list-app-gash-recharge"
              @click="handleAppGashRecharge"
            >
              <el-icon :size="14"><Iphone /></el-icon>
              <span>{{ t('AppGashRecharge') }}</span>
            </button>
            <button
              type="button"
              class="account-list__quick-btn"
              data-test="account-list-member-center"
              @click="handleMemberCenter"
            >
              <el-icon :size="14"><User /></el-icon>
              <span>{{ t('accountList.memberCenter') }}</span>
            </button>
            <button
              type="button"
              class="account-list__quick-btn"
              data-test="account-list-customer-service"
              @click="handleCustomerService"
            >
              <el-icon :size="14"><Service /></el-icon>
              <span>{{ t('accountList.customerService') }}</span>
            </button>
          </div>
        </section>

        <!-- Service accounts list — 4 rendered states (REAL) -->
        <section class="account-list__list bf-glass-panel">
          <header class="account-list__list-header">
            <h2 class="account-list__list-title">
              {{ t('accountList.serviceAccountsHeading') }}
            </h2>
            <div class="account-list__list-header-right">
              <span
                v-if="limitNoticeText"
                class="account-list__limit-badge"
                data-test="account-list-limit-notice"
              >
                {{ limitNoticeText }}
              </span>
              <button
                type="button"
                class="account-list__add-btn-inline"
                :disabled="isAddAccountDisabled"
                data-test="account-list-add"
                @click="handleAddAccount"
              >
                <el-icon :size="12"><Plus /></el-icon>
              </button>
              <span class="account-list__list-count" data-test="account-list-count">
                {{ t('accountList.accountCount', { count: accountCount }) }}
              </span>
            </div>
          </header>

          <!--
            #295: Wrapper provides the positioning context for the
            scroll-hint overlay. The hint must be a sibling of
            list-body (not a child) so that position:absolute anchors
            to the wrapper's visible bottom edge rather than to the
            scroll content's bottom, which would shift on scroll.
          -->
          <div class="account-list__list-scroll-wrapper">
            <div
              ref="listBodyRef"
              class="account-list__list-body bf-custom-scrollbar"
              @scroll="handleListScroll"
            >
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
              D7: SortableJS (via `rowsRef` + `watch`) makes the row
              list drag-sortable. The `handle` option mirrors WPF's
              `_isHandlePressed` gate — only mouse-down on the grip
              starts a drag; any other click on the row falls through
              to `selectRow`. `ghostClass` styles the placeholder
              slot during drag (defined at bottom of <style scoped>).
            -->
              <ul v-else ref="rowsRef" class="account-list__rows" data-test="account-list-rows">
                <li
                  v-for="(a, idx) in serviceAccounts"
                  :key="a.sid"
                  class="account-list__row"
                  :class="{
                    'account-list__row--selected': isSelected(a),
                    'account-list__row--banned': !a.is_enable,
                  }"
                  :data-test="`account-row-${a.sid}`"
                  @click="selectRow(a)"
                  @dblclick="handleRowDblClick(a)"
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
                    <p v-if="!a.is_enable" class="account-list__row-sub">
                      {{ t('accountList.statusBanned') }}
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
              </ul>
            </div>

            <!--
              #295: Scroll indicator is a sibling of list-body (not a child)
              so position:absolute stays anchored to the wrapper's visible
              bottom edge regardless of scroll position.
            -->
            <Transition name="account-list__scroll-hint-fade">
              <div
                v-if="showScrollIndicator && !listScrolledToBottom"
                class="account-list__scroll-hint"
                aria-hidden="true"
              >
                <svg
                  width="24"
                  height="24"
                  viewBox="0 0 24 24"
                  fill="none"
                  xmlns="http://www.w3.org/2000/svg"
                >
                  <path
                    d="M7 10L12 15L17 10"
                    stroke="currentColor"
                    stroke-width="2"
                    stroke-linecap="round"
                    stroke-linejoin="round"
                  />
                </svg>
              </div>
            </Transition>
          </div>
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

        <!-- P12.5 D7: Tools dialog stack — single mount that hosts
           MapleTools / KartTools / EquipCalculator /
           CoreCalculator (in-app browser routed via composable;
           see followup-B B7). Opened imperatively by `handleTools` via
           the `toolsDialogRef` handle (see the `handleTools`
           docblock for why imperative + why a single-component
           wrapper). The wrapper mounts unconditionally so its
           internal dialog mounts can play their open transitions
           on first open; per-dialog visibility is owned inside
           the wrapper. -->
        <ToolsDialogStack ref="toolsDialogRef" />

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
                :title="t('accountList.copyOtp')"
                data-test="account-list-otp-field"
                @dblclick="handleOtpDblClick"
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
    </div>
  </main>
</template>

<style scoped>
.account-list {
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.account-list__titlebar-btn {
  appearance: none;
  background: transparent;
  border: none;
  width: 28px;
  height: 28px;
  display: grid;
  place-items: center;
  border-radius: 6px;
  cursor: pointer;
  color: var(--bf-on-surface-variant, #54443a);
  transition: background 150ms ease;
  padding: 0;
}
.account-list__titlebar-btn .el-icon {
  font-size: 18px;
}
.account-list__titlebar-btn:hover {
  background: rgba(0, 0, 0, 0.06);
}

/*
 * Follow-up to #236: the page used to let the *outer* `__scroll`
 * container overflow when the combined height of the game bar +
 * quick actions + account list + OTP section exceeded the window.
 * With many service accounts on a small-to-medium display that
 * pushed the OTP section off-screen, forcing the user to scroll
 * the wheel before they could hit "Get OTP" — diverging from the
 * legacy Beanfun (B6) single-page layout.
 *
 * The fix turns the scroll container into a non-scrolling flex
 * column and flips the *inner* account list (`__list` →
 * `__list-body` below) into the only scroller. Everything else
 * (header, game bar, quick actions, OTP footer) retains its
 * natural height so the OTP row is always anchored at the bottom
 * of the window, and only the list itself scrolls when the account
 * count outgrows the available space.
 *
 * `min-height: 0` on every intermediate flex level is required by
 * the flexbox spec so that `flex: 1` children can actually shrink
 * below their content height (without it the inner scrollbar never
 * appears and the list clips).
 */
.account-list__scroll {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding: 0.75rem;
  display: flex;
  flex-direction: column;
}

.account-list__container {
  flex: 1;
  width: 100%;
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
}

/* --------------- header --------------- */

.account-list__header {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  text-align: left;
  margin-bottom: 0.25rem;
}

.account-list__header-text {
  flex: 1;
  min-width: 0;
}

.account-list__title {
  margin: 0;
  /* rule in this section is nudged +~0.0625rem so the page reads  */
  /* more comfortably on high-DPI displays without overshooting    */
  /* the rest of the chrome's proportions.                           */
  font-size: 1.75rem;
  font-weight: 800;
  letter-spacing: -0.01em;
}

.account-list__subline {
  margin: 0.25rem 0 0;
  font-size: 0.875rem;
  color: var(--bf-on-surface-variant);
}

/* --------------- game info bar --------------- */

.account-list__game {
  padding: 0.5rem 0.75rem;
  display: flex;
  align-items: center;
  gap: 0.5rem;
}

.account-list__game-icon {
  width: 36px;
  height: 36px;
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
  flex: 1;
  overflow: hidden;
  display: flex;
  flex-direction: column;
  gap: 0.0625rem;
  color: var(--bf-on-surface);
  transition: color var(--bf-motion-fast);
}

.account-list__game-info:hover {
  color: var(--bf-primary);
}

.account-list__game-name {
  max-width: 100%;
  font-size: 1.0625rem;
  font-weight: 700;
  line-height: 1.2;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.account-list__game-status {
  font-size: 0.8125rem;
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
  padding: 0.375rem 0.75rem;
  font-weight: 700;
  font-size: 0.8125rem;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 0.25rem;
  min-width: 0;
  white-space: nowrap;
}

.account-list__game-actions {
  display: flex;
  align-items: center;
  gap: 0.25rem;
  flex-shrink: 0;
}

/* --------------- quick actions --------------- */

.account-list__quick {
  padding: 0.375rem 0.75rem;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.5rem;
  flex-wrap: wrap;
}

.account-list__balance {
  display: flex;
  align-items: center;
  gap: 0.375rem;
  flex: 1 1 15rem;
  min-width: 0;
}

.account-list__balance-label {
  font-size: 0.8125rem;
  color: var(--bf-on-surface-variant);
  font-weight: 600;
  white-space: nowrap;
}

.account-list__balance-value {
  font-size: 0.9375rem;
  font-weight: 700;
  color: var(--bf-on-surface);
  white-space: nowrap;
}

.account-list__balance-refresh {
  appearance: none;
  background: transparent;
  border: 0;
  cursor: pointer;
  color: var(--bf-on-surface-variant);
  padding: 0.125rem;
  border-radius: var(--bf-radius-input);
  transition: color var(--bf-motion-fast);
  flex-shrink: 0;
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

.account-list__quick-actions {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, auto));
  gap: 0.25rem;
  flex-shrink: 0;
  margin-left: auto;
}

.account-list__quick-btn {
  appearance: none;
  background: transparent;
  border: 0;
  cursor: pointer;
  display: inline-flex;
  align-items: center;
  gap: 0.125rem;
  padding: 0.25rem 0.375rem;
  font-size: 0.6875rem;
  color: var(--bf-on-surface-variant);
  border-radius: var(--bf-radius-input);
  transition:
    color var(--bf-motion-fast),
    background var(--bf-motion-fast);
  white-space: nowrap;
  min-width: 0;
}

.account-list__quick-btn:hover {
  color: var(--bf-primary);
  background: rgba(0, 0, 0, 0.04);
}

/* --------------- list section --------------- */

.account-list__list {
  display: flex;
  flex-direction: column;
}

.account-list__list-header {
  padding: 0.5rem 0.75rem;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.375rem;
  border-bottom: 1px solid rgba(255, 255, 255, 0.4);
}

.account-list__list-header-right {
  display: flex;
  align-items: center;
  gap: 0.375rem;
}

.account-list__list-title {
  margin: 0;
  font-size: 0.8125rem;
  font-weight: 700;
  color: var(--bf-on-surface);
  white-space: nowrap;
}

.account-list__list-count {
  font-size: 0.6875rem;
  font-weight: 600;
  color: var(--bf-on-surface-variant);
  background: var(--bf-surface-container);
  padding: 0.125rem 0.375rem;
  border-radius: var(--bf-radius-input);
}

.account-list__add-btn-inline {
  appearance: none;
  background: transparent;
  border: 1px dashed var(--bf-outline-variant);
  cursor: pointer;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 22px;
  height: 22px;
  color: var(--bf-primary);
  border-radius: 50%;
  transition:
    border-color var(--bf-motion-fast),
    background var(--bf-motion-fast);
  flex-shrink: 0;
}

.account-list__add-btn-inline:hover:not(:disabled) {
  border-color: var(--bf-primary);
  background: color-mix(in srgb, var(--bf-primary) 10%, transparent);
}

.account-list__add-btn-inline:disabled {
  opacity: 0.45;
  cursor: not-allowed;
}

.account-list__limit-badge {
  font-size: 0.625rem;
  color: var(--bf-primary);
  white-space: nowrap;
}

/*
 * #295: Wrapper provides the positioning context for the scroll-hint
 * overlay so it stays anchored to the visible bottom edge of the list
 * rather than the scroll content's bottom.
 */
.account-list__list-scroll-wrapper {
  position: relative;
}

.account-list__list-body {
  overflow-y: auto;
  padding: 0.5rem;
  display: flex;
  flex-direction: column;
  gap: 0.25rem;
  /*
   * #263: cap the visible rows. Keep --max-visible-rows in sync with
   * MAX_VISIBLE_ROWS constant in <script>. Each row is ~36px height +
   * 0.25rem gap between rows + 1rem total padding (0.5rem top + bottom).
   */
  --max-visible-rows: 5;
  max-height: calc(var(--max-visible-rows) * 36px + (var(--max-visible-rows) - 1) * 0.25rem + 1rem);
}

/*
 * #263: Floating scroll hint indicator. Appears at the bottom of the
 * account list when there are more accounts than can fit, prompting
 * the user that the list is scrollable. Fades out when scrolled near bottom.
 */
.account-list__scroll-hint {
  position: absolute;
  bottom: 0;
  left: 50%;
  transform: translateX(-50%);
  display: flex;
  align-items: center;
  justify-content: center;
  width: 32px;
  height: 24px;
  background: linear-gradient(to top, var(--bf-surface-container-low), transparent);
  color: var(--bf-on-surface-variant);
  pointer-events: none;
  z-index: 1;
}

.account-list__scroll-hint svg {
  animation: account-list__scroll-hint-bounce 1.5s ease-in-out infinite;
}

@keyframes account-list__scroll-hint-bounce {
  0%,
  100% {
    transform: translateY(0);
  }
  50% {
    transform: translateY(3px);
  }
}

/*
 * #263: Transition for the scroll hint fade in/out.
 */
.account-list__scroll-hint-fade-enter-active,
.account-list__scroll-hint-fade-leave-active {
  transition: opacity 0.2s ease;
}

.account-list__scroll-hint-fade-enter-from,
.account-list__scroll-hint-fade-leave-to {
  opacity: 0;
}

.account-list__list-state {
  margin: auto;
  padding: 1.5rem 1rem;
  text-align: center;
  font-size: 0.875rem;
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
  padding: 0.4375rem 0.75rem;
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
  font-size: 0.9375rem;
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
 * SortableJS ghost-class — applied to the placeholder element
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
  font-size: 0.875rem;
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
  font-size: 0.75rem;
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
  font-size: 0.875rem;
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

.account-list__add-btn:hover:not(:disabled) {
  border-color: var(--bf-primary);
  background: color-mix(in srgb, var(--bf-primary) 6%, transparent);
}

.account-list__add-btn--disabled {
  opacity: 0.45;
  cursor: not-allowed;
}

.account-list__limit-notice {
  margin: 0 0 0.5rem;
  padding: 0.375rem 0.75rem;
  font-size: 0.8125rem;
  color: var(--el-color-warning, #e6a23c);
  text-align: center;
}

/* --------------- OTP section --------------- */

.account-list__otp {
  padding: 0.5rem 0.75rem;
  display: flex;
  flex-direction: column;
  gap: 0.375rem;
}

.account-list__otp-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.account-list__otp-title {
  margin: 0;
  font-size: 0.8125rem;
  font-weight: 700;
  color: var(--bf-on-surface);
}

.account-list__otp-row {
  display: flex;
  gap: 0.375rem;
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
  /*
   * Issue #272 — was a literal `rgba(226, 226, 226, 0.3)` (a
   * light-grey wash that read fine on the warm light-mode
   * glass panel). In dark mode the same grey overlay landed
   * on top of the `#252529` panel and produced a low-contrast
   * smudge with the placeholder text inside it. Route the
   * background through `color-mix` against the token's
   * on-surface colour: 8% in light mode reproduces the same
   * faint grey lift the original literal achieved, and in
   * dark mode the same 8% becomes a subtle white tint (on
   * `#e6e1e5`) that gives the field a visible chrome edge
   * without blowing out brightness.
   */
  background: color-mix(in srgb, var(--bf-on-surface) 8%, transparent);
  border: 0;
  border-bottom: 2px solid var(--bf-outline-variant);
  font-family: 'JetBrains Mono', 'Consolas', ui-monospace, monospace;
  font-size: 1rem;
  letter-spacing: 0.15em;
  text-align: center;
  padding: 0.375rem 2rem 0.375rem 0.5rem;
  border-radius: var(--bf-radius-input) var(--bf-radius-input) 0 0;
  color: var(--bf-on-surface);
  cursor: default;
  transition: border-color var(--bf-motion-fast);
}

/*
 * Issue #272 — the OTP "尚未取得" placeholder used the browser
 * default (`color` * ~54% in WebView2), which produced
 * unreadable dark-grey-on-dark-grey in dark mode. Pin the
 * placeholder to `--bf-on-surface-variant` (`#54443a` light /
 * `#c9c5ca` dark) at 70% opacity so it still reads as
 * secondary text while staying legible in both themes.
 */
.account-list__otp-field::placeholder {
  color: var(--bf-on-surface-variant);
  opacity: 0.7;
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
  min-width: 80px;
  padding: 0 0.75rem;
  font-size: 0.8125rem;
  font-weight: 700;
  display: inline-flex;
  align-items: center;
  justify-content: center;
}
</style>
