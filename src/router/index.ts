/**
 * Vue Router configuration — hash-mode SPA mirroring the WPF
 * Page+Window layout via per-route components.
 *
 * # Why hash mode?
 *
 * Tauri loads the bundled `index.html` via the `tauri://` protocol;
 * `createWebHistory` would require a custom resolver that rewrites
 * deep-link URLs back to `/index.html`. Hash routing sidesteps the
 * problem entirely and is the de-facto standard for Tauri SPAs (the
 * official template uses it too). This trade-off does not affect
 * frontend UX — the user never sees the URL bar.
 *
 * # Route hierarchy
 *
 * ```
 * /                                redirect → /login
 * /login                           LoginPage (shell with <RouterView />)
 *   "" (default child)             LoginRegionSelection      (D2 ✓)
 *   /login/id-pass                 IdPassForm                (D3 ✓)
 *   /login/qr                      QrForm                    (D4 ✓)
 *   /login/gamepass                GamepassForm              (D5 CP2 ✓)
 *   /login/totp                    LoginTotp                 (D6 ✓)
 *   /login/wait                    LoginWait                 (D7 ✓)
 *   /login/verify                  VerifyPage                (D8 ✓)
 * /accounts                        AccountList               (P12.2 D1 ✓, requiresAuth)
 * /manage-account                  ManageAccount             (P12.2 D9 ✓, public since #314)
 * /:pathMatch(.*)*                 redirect → /              (D10 catch-all)
 * ```
 *
 * Each subsequent D-step appends one entry under `loginChildren` so
 * the diff per D-step stays focused on the form being added, not the
 * router scaffolding.
 *
 * # Why the picker lives on the parent's empty path (not `/login/region`)
 *
 * vue-router warns when a *named* parent has an *unnamed* empty-path
 * child — the parent name doesn't render the empty child. Naming the
 * empty child instead lets `name: 'login-region'` resolve to the
 * picker and avoids the warning. Keeping the picker on the parent
 * path also matches the WPF default boot UX (no extra `/region`
 * segment that the user would never type by hand).
 *
 * # 404 handling
 *
 * Catch-all `/:pathMatch(.*)*` redirects to `/`, which in turn
 * redirects to `/login`. Returning a real 404 page is a P13 concern.
 *
 * # Auth guards (D10)
 *
 * Two router-level concerns wired by {@link installRouterGuards}:
 *
 * 1. **`beforeEach` requiresAuth guard** — routes opting in via
 *    `meta: { requiresAuth: true }` get redirected to `/login` if
 *    the auth store reports no session. The original target is
 *    stashed under `?redirect=<fullPath>` so a future post-login
 *    deep-link replay (P12.2 D2+ once `/accounts/...` exists) can
 *    return the user to where they intended. P12.1 ships zero
 *    protected routes — every login child is explicitly public —
 *    but the guard infrastructure lands now so P12.2 D-steps just
 *    set the meta flag and inherit the behavior.
 *
 * 2. **session-expired bridge** — wires
 *    `services/invoke.ts::registerSessionExpiredHandler` so any
 *    backend command failing with `auth.session_required` clears
 *    the local Pinia auth state via `auth.clearSession()`,
 *    *also* clears the account store's session-scoped cache via
 *    `clearAccountSession()` (P12.2 D1 fix — without this the
 *    next login would briefly flash the previous session's
 *    service-account list while the new fetch ran), and forces a
 *    navigation back to `/login?sessionExpired=1`. The query
 *    flag exists so a future banner / toast on `LoginPage` can
 *    surface "your session expired, please log in again" UX
 *    without the user having to read the toast (P12.X concern;
 *    flag is reserved here so the contract is stable).
 */

import type { RouteRecordRaw, Router } from 'vue-router'
import { createRouter, createWebHashHistory } from 'vue-router'

import { nextTick } from 'vue'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { getCurrentWebview } from '@tauri-apps/api/webview'
import { LogicalSize } from '@tauri-apps/api/dpi'

import { registerSessionExpiredHandler } from '../services/invoke'
import { isWindowFitSuspended } from '../services/windowFit'

/**
 * Resize the Tauri window to the given **logical** (DPI-independent)
 * dimensions.
 *
 * Sizing with {@link LogicalSize} (not `PhysicalSize`) is what keeps the
 * window proportional on every display: Tauri multiplies logical px by
 * the window's OS scale factor internally, so `420` logical px is `420`
 * device px at 100% scaling, `525` at 125%, `630` at 150% — the same
 * visual size everywhere. The old `PhysicalSize` path baked device
 * pixels and only compensated on ≥4K monitors, so at 125–150% Windows
 * scaling the window came out physically too small and the whole UI was
 * squeezed.
 *
 * Exported so individual pages can call it on mount when their content
 * height differs from the route meta default.
 */
export function resizeWindow(width: number, height: number): void {
  void getCurrentWindow().setSize(new LogicalSize(Math.ceil(width), Math.ceil(height)))
}

import LoginPage from '../pages/LoginPage.vue'
import LoginRegionSelection from '../pages/LoginRegionSelection.vue'
import IdPassForm from '../pages/IdPassForm.vue'
import QrForm from '../pages/QrForm.vue'
import GamepassForm from '../pages/GamepassForm.vue'
import RecaptchaForm from '../pages/RecaptchaForm.vue'
import LoginTotp from '../pages/LoginTotp.vue'
import LoginWait from '../pages/LoginWait.vue'
import VerifyPage from '../pages/VerifyPage.vue'
import AccountList from '../pages/AccountList.vue'
import ManageAccount from '../pages/ManageAccount.vue'
import SettingsPage from '../pages/Settings.vue'
import AboutPage from '../pages/About.vue'

/**
 * Where authenticated users land after a successful login.
 *
 * Backed by the `AccountList` route registered in {@link routes}
 * since P12.2 D1. Earlier P12.1 D3-D8 login-success call sites
 * (`IdPassForm.vue` / `LoginTotp.vue` / `QrForm.vue` /
 * `GamepassForm.vue`) all `router.push('/accounts')`; before P12.2
 * D1 these fell through the catch-all back to `/login`, which was
 * a documented benign no-op. Now they reach the real page.
 *
 * Centralising the path string here (rather than scattering it
 * across the four call sites) is what made the P12.1 → P12.2
 * hand-off a one-line change instead of a grep-and-replace.
 */
export const LOGGED_IN_LANDING_PATH = '/accounts'

/**
 * Stable route-name constants. UI code referencing routes by name
 * (rather than path strings) survives path renames during P12 churn.
 *
 * The parent `/login` shell deliberately has no `name` — see the
 * "empty path child" rationale in the header docblock.
 */
export const ROUTE_NAMES = {
  LoginRegion: 'login-region',
  LoginIdPass: 'login-id-pass',
  LoginQr: 'login-qr',
  LoginGamepass: 'login-gamepass',
  LoginRecaptcha: 'login-recaptcha',
  LoginTotp: 'login-totp',
  LoginWait: 'login-wait',
  LoginVerify: 'login-verify',
  Accounts: 'accounts',
  ManageAccount: 'manage-account',
  Settings: 'settings',
  About: 'about',
} as const

/**
 * Login child routes — appended one-per-D-step as each form lands.
 *
 * The empty-string child path is the vue-router idiom for "render
 * this when the parent path is hit with no further segments"; we put
 * the region picker there so first-launch users see it immediately
 * after navigating to `/login` (matches WPF default boot UX before
 * the D10 router guard short-circuits to the last login form).
 */
const loginChildren: RouteRecordRaw[] = [
  {
    path: '',
    name: ROUTE_NAMES.LoginRegion,
    component: LoginRegionSelection,
    meta: {
      titleKey: 'titleBar.regionSelection',
      titleIcon: 'public',
      windowWidth: 420,
      windowHeight: 480,
    },
  },
  {
    path: 'id-pass',
    name: ROUTE_NAMES.LoginIdPass,
    component: IdPassForm,
    meta: { titleKey: 'titleBar.login', titleIcon: 'login', windowWidth: 420, windowHeight: 520 },
  },
  {
    path: 'qr',
    name: ROUTE_NAMES.LoginQr,
    component: QrForm,
    meta: {
      titleKey: 'titleBar.login',
      titleIcon: 'qr_code_2',
      windowWidth: 420,
      windowHeight: 600,
    },
  },
  {
    path: 'gamepass',
    name: ROUTE_NAMES.LoginGamepass,
    component: GamepassForm,
    meta: {
      titleKey: 'titleBar.login',
      titleIcon: 'verified_user',
      windowWidth: 420,
      windowHeight: 460,
    },
  },
  {
    path: 'recaptcha',
    name: ROUTE_NAMES.LoginRecaptcha,
    component: RecaptchaForm,
    meta: {
      titleKey: 'titleBar.login',
      titleIcon: 'verified_user',
      windowWidth: 420,
      windowHeight: 460,
    },
  },
  {
    path: 'totp',
    name: ROUTE_NAMES.LoginTotp,
    component: LoginTotp,
    meta: {
      titleKey: 'titleBar.totp',
      titleIcon: 'encrypted',
      windowWidth: 420,
      windowHeight: 380,
    },
  },
  {
    path: 'wait',
    name: ROUTE_NAMES.LoginWait,
    component: LoginWait,
    meta: {
      titleKey: 'titleBar.loginWait',
      titleIcon: 'login',
      windowWidth: 420,
      windowHeight: 360,
    },
  },
  {
    path: 'verify',
    name: ROUTE_NAMES.LoginVerify,
    component: VerifyPage,
    meta: {
      titleKey: 'titleBar.verify',
      titleIcon: 'shield_lock',
      windowWidth: 420,
      windowHeight: 480,
    },
  },
]

export const routes: RouteRecordRaw[] = [
  {
    path: '/',
    redirect: '/login',
  },
  {
    path: '/login',
    component: LoginPage,
    children: loginChildren,
  },
  {
    path: LOGGED_IN_LANDING_PATH,
    name: ROUTE_NAMES.Accounts,
    component: AccountList,
    /*
     * P12.2 D1: first protected route that exercises the D10 guard
     * infrastructure. Unauthenticated visits are redirected to
     * `/login?redirect=/accounts` so a future post-login replay
     * (P12.2 D-step) can land the user back on the page they
     * originally targeted.
     */
    meta: {
      requiresAuth: true,
      titleKey: 'titleBar.accounts',
      titleIcon: 'sports_esports',
      windowWidth: 480,
      windowHeight: 800,
    },
  },
  {
    path: '/settings',
    name: ROUTE_NAMES.Settings,
    component: SettingsPage,
    meta: {
      titleKey: 'titleBar.settings',
      titleIcon: 'settings',
      windowWidth: 660,
      windowHeight: 680,
    },
    /*
     * P12.4 D6: Settings page is reachable from both the
     * post-login `AccountList` top-bar icon and (per WPF parity
     * `Settings.xaml.cs::Button_Click` L85-94) the pre-login
     * funnel — WPF's `return_page == loginPage` branch lets
     * unauthenticated users open Settings to change language /
     * theme before logging in. We therefore intentionally leave
     * `requiresAuth` undefined (= public). The Game section
     * inside the page guards itself on `game.selectedGame`, so
     * an unauthenticated visit just shows the App section + the
     * empty-state banner instead of crashing on a missing
     * selection.
     */
  },
  {
    path: '/about',
    name: ROUTE_NAMES.About,
    component: AboutPage,
    meta: { titleKey: 'titleBar.about', titleIcon: 'info', windowWidth: 420, windowHeight: 680 },
    /*
     * P12.4 D7: same public-route rationale as `/settings` —
     * WPF allowed the About page from both pre-login and
     * post-login surfaces. The page only reads `commands.version`
     * and `commands.checkUpdate`, neither of which require an
     * authenticated session.
     */
  },
  {
    path: '/manage-account',
    name: ROUTE_NAMES.ManageAccount,
    component: ManageAccount,
    /*
     * P12.2 D9: stored-credential CRUD page (Users.dat add / edit /
     * delete + plaintext JSON import / export). Entry point is the
     * Settings page button (P12.4), which is itself public — WPF's
     * `Settings.xaml.cs::ManageAcc_Click` (L266-269) navigates
     * unconditionally, with no login gate, and the page only touches
     * the local DPAPI-encrypted Users.dat via `commands.loadAccounts`
     * / `saveAccount` / `removeAccount` (no session-scoped backend
     * call anywhere on the page). D9 originally shipped
     * `requiresAuth: true` on the "session-scoped UX" theory; issue
     * #314 showed that guess breaks the WPF pre-login flow (Settings
     * → 管理帳號 bounced back to `/login`), so the route is public
     * now, matching `/settings` and `/about`.
     */
    meta: {
      titleKey: 'titleBar.manageAccount',
      titleIcon: 'manage_accounts',
      windowWidth: 880,
      windowHeight: 640,
    },
  },
  {
    path: '/:pathMatch(.*)*',
    redirect: '/',
  },
]

/**
 * Build the application router instance. Exposed as a factory rather
 * than a module-level singleton so:
 *
 * 1. Vitest can mount a fresh router per test (no leaked navigation
 *    history between cases),
 * 2. SSR / multi-window scenarios can each get their own router
 *    without sharing history state.
 *
 * `main.ts` calls `createAppRouter()` exactly once and follows up
 * with {@link installRouterGuards} to wire the auth-related
 * cross-cutting hooks.
 */
export function createAppRouter() {
  return createRouter({
    history: createWebHashHistory(),
    routes,
  })
}

/**
 * Per-route flags interpreted by the router-level cross-cutting
 * hooks installed by {@link installRouterGuards}.
 */
declare module 'vue-router' {
  interface RouteMeta {
    /**
     * When `true`, an unauthenticated visit to this route is
     * intercepted by the {@link installRouterGuards} `beforeEach`
     * hook and redirected to `/login?redirect=<original.fullPath>`.
     *
     * Defaults to `undefined` (treated as `false`). Login children
     * intentionally leave this undefined — they're the public
     * entry surface; flipping a login child to `requiresAuth: true`
     * would deadlock the boot flow (no session → redirect to
     * `/login` → redirect to `/login` → ...).
     */
    requiresAuth?: boolean
    /**
     * i18n key for the custom title bar text. When unset, the
     * title bar falls back to `t('AppName')`.
     */
    titleKey?: string
    /**
     * Material Symbols icon name for the custom title bar.
     * When unset, falls back to `'coffee'`.
     */
    titleIcon?: string
    /**
     * Desired window width in logical pixels for this route.
     * The router afterEach hook calls `appWindow.setSize()` when
     * the value differs from the current route.
     */
    windowWidth?: number
    /**
     * Desired window height in logical pixels for this route.
     */
    windowHeight?: number
  }
}

/**
 * Dependencies the {@link installRouterGuards} hook needs to read
 * and mutate auth state without importing the Pinia store directly.
 *
 * Taking these as a function-shaped contract instead of an
 * `AuthStore` reference keeps the router decoupled from Pinia's
 * setup-store internals (handy for vitest cases that exercise the
 * guard with a plain `() => false` stub) and avoids a circular
 * import (router → store → invoke → ... ).
 */
export interface RouterGuardDeps {
  /**
   * Returns `true` iff a usable session is in scope. Called once
   * per navigation by the `requiresAuth` guard.
   */
  isAuthenticated: () => boolean
  /**
   * Wipes the local auth state without invoking any backend command
   * (the session is gone server-side already, so a `commands.logout()`
   * round-trip would just fail and add latency). The session-expired
   * bridge calls this before redirecting.
   */
  clearSession: () => void
  /**
   * Optional companion wipe for *non-auth* session-scoped state
   * (P12.2 D1: the `account` store's cached service-account list,
   * email, remain-point, contract, and OTP selection). Fired by the
   * session-expired bridge alongside {@link clearSession} so a
   * subsequent re-login doesn't briefly flash the previous
   * session's data while the new fetch runs.
   *
   * Optional so D10 era specs that only stubbed the auth callback
   * keep working unchanged; production wiring in `main.ts`
   * supplies both callbacks (P12.2 D1 fix-up of the D10 bug where
   * `auth.session_required` cleared `auth` but left
   * `account.serviceAccounts` populated).
   *
   * Why a second callback rather than chaining inside
   * {@link clearSession}: SRP — the router shouldn't know which
   * Pinia stores back which slice of session state, but it *is*
   * the right layer to know "session expired = wipe every
   * session-scoped thing in one shot". Composition stays in
   * `main.ts` where every bootstrapped store is already in scope.
   */
  clearAccountSession?: () => void
}

/**
 * Install the two D10 router-level cross-cutting hooks: the
 * `requiresAuth` guard plus the session-expired bridge. Both live
 * here (rather than as separate `installXxx` functions) because:
 *
 * - they share the same `RouterGuardDeps` contract,
 * - they're always installed together at boot (one without the
 *   other leaves a half-wired auth flow),
 * - keeping a single entry point lets `main.ts` add this with one
 *   line right after `app.use(router)` instead of remembering two
 *   ordering-sensitive calls.
 *
 * Idempotent w.r.t. router state — calling twice would register two
 * `beforeEach` hooks (and replace the session-expired handler since
 * the invoke layer keeps a single slot). Tests that need a fresh
 * setup mount a fresh router via {@link createAppRouter}.
 */
export function installRouterGuards(router: Router, deps: RouterGuardDeps): void {
  router.beforeEach((to) => {
    const needsAuth = to.matched.some((record) => record.meta.requiresAuth === true)
    if (needsAuth && !deps.isAuthenticated()) {
      /*
       * Defensive: never carry `?redirect=` when the protected
       * target is itself somewhere inside the `/login` funnel —
       * that would either point the user back at the same spot
       * after login (infinite loop) or encode `/login?...` as a
       * redirect target, neither of which is useful UX. The
       * production route table never marks a login child
       * `requiresAuth: true`, but the check guards against a
       * future misconfiguration becoming user-visible.
       */
      const isLoginFunnel = to.path === '/login' || to.path.startsWith('/login/')
      return {
        path: '/login',
        query: isLoginFunnel ? undefined : { redirect: to.fullPath },
      }
    }
    return true
  })

  registerSessionExpiredHandler(() => {
    deps.clearSession()
    deps.clearAccountSession?.()
    void nextTick().then(() =>
      router
        .push({
          path: '/login',
          query: { sessionExpired: '1' },
        })
        .then((failure) => {
          if (failure) {
            console.warn('[router] session-expired redirect returned NavigationFailure', failure)
          }
        })
        .catch((err: unknown) => {
          console.error('[router] session-expired redirect threw', err)
        }),
    )
  })

  /*
   * Auto-fit window size to content. Uses a ResizeObserver on the
   * `[data-window-root]` element so the window tracks content
   * height changes (e.g. sections appearing/disappearing, async
   * data loading). Width comes from route meta.
   *
   * # Why double `requestAnimationFrame` (issue #236)
   *
   * Previously this code used `setTimeout(setupObserver, 80)` after
   * every `afterEach`. Four bugs fell out of that:
   *
   * 1. **Timing race** — 80ms is not tied to Vue's render flush or
   *    the browser's paint, so on a slow boot (config.xml IPC taking
   *    longer than 80ms) the observer attached to a half-rendered
   *    DOM and measured a shorter-than-final `scrollHeight`, leaving
   *    the user with a window that never grew to match the real
   *    content height.
   * 2. **Navigation storm** — on first launch the router fires
   *    `afterEach` twice in rapid succession: `/` → `/login/`
   *    (region picker empty child) → `/login/id-pass` (via
   *    `LoginRegionSelection.vue::watch(config.loaded)`). Both
   *    `setTimeout` s fired, the earlier one measuring the picker
   *    DOM just as it was being replaced. `scheduleOnNextPaint`
   *    auto-cancels the previous pending callback so only the final
   *    destination's DOM gets measured.
   * 3. **`setSize` → layout-restore race** — the old code ran
   *    `height='auto'` → measure → `setSize().then(() => height='100vh')`.
   *    `setSize` is an async IPC (10-50 ms), so the DOM stayed in
   *    "height: auto" for a frame-and-a-half while the window was
   *    still at its old size — user-visible if the new content was
   *    taller than the old window. The new `fitWindow` flips
   *    `auto` → `100vh` synchronously within one frame (the browser
   *    never paints the intermediate state) and fires the `setSize`
   *    IPC without awaiting the layout restore.
   * 4. **Observer loop** — the `height='auto'` flip fired the
   *    observer, which called `fitWindow`, which flipped it again.
   *    The same-frame flip-back in (3) coalesces the observer
   *    notifications to a no-op (`100vh` in → `100vh` out), so no
   *    explicit guard flag is needed.
   */
  const appWindow = getCurrentWindow()
  let currentWidth = 560
  let observer: ResizeObserver | null = null
  let pendingFrame: number | null = null

  /**
   * Available work area (screen minus taskbar) in **logical / CSS px**.
   *
   * Read live from `window.screen` on every fit, so it reflects whichever
   * monitor the window is currently on, the current OS scaling, and the
   * monitor's orientation — no cached per-monitor zoom, no hard-coded
   * resolution thresholds, no "only compensate on 4K" special-case. The
   * old approach keyed off `currentMonitor().size` (physical px) plus a
   * `≥3840` UHD test, which mis-classified e.g. a 1440×2560 **portrait**
   * panel as UHD and left every other scaled display sizing wrong.
   *
   * `availWidth` / `availHeight` are already CSS px (the browser divides
   * out the scale factor), so they compare directly against
   * `scrollHeight` and feed straight into `LogicalSize`. Falls back to a
   * roomy default when `window.screen` is unavailable (jsdom / CI).
   */
  function workArea(): { w: number; h: number } {
    const s = typeof window !== 'undefined' ? window.screen : undefined
    return {
      w: s && s.availWidth > 0 ? s.availWidth : 1280,
      h: s && s.availHeight > 0 ? s.availHeight : 900,
    }
  }

  /**
   * Measure `[data-window-root]`'s natural content size and resize the
   * OS window to match, shrinking (webview zoom < 1) only when the
   * content would overflow the screen's work area on **either** axis.
   * Both the `height: auto` measurement flip and the `height: 100vh`
   * restore land inside one synchronous block so the browser never
   * paints the intermediate "unclipped" state.
   *
   * Everything is computed in logical / CSS px and the window is sized
   * with `LogicalSize`, so the result keeps the same proportion at any
   * DPI / OS scaling and on landscape *or* portrait monitors. Safe to
   * call concurrently; the `pendingFrame` guard upstream keeps the rate
   * low enough that this isn't a hot path.
   */
  function fitWindow(): void {
    // A full-window overlay (AnnouncementModal) may be holding the window
    // at a fixed larger size; don't snap it back to content height.
    if (isWindowFitSuspended()) return
    const root = document.querySelector('[data-window-root]') as HTMLElement | null
    if (!root) return
    // Temporarily disable overflow clipping so scrollHeight reflects
    // the full content including all account rows + OTP footer.
    const scroll = root.querySelector('.account-list__scroll') as HTMLElement | null
    if (scroll) scroll.style.overflow = 'visible'
    root.style.height = 'auto'
    const scrollH = root.scrollHeight
    if (scroll) scroll.style.overflow = ''

    // Natural (design) size in logical px: width from the route meta,
    // height from the measured content.
    const naturalW = currentWidth
    const naturalH = Math.max(1, scrollH)

    // Cap to 95% of the work area on both axes (the 5% margin keeps the
    // window off the taskbar and draggable). A single uniform zoom keeps
    // the aspect ratio when the content is bigger than the screen — the
    // case that actually bites on small laptops and portrait panels.
    const area = workArea()
    const capW = Math.max(320, Math.floor(area.w * 0.95))
    const capH = Math.max(300, Math.floor(area.h * 0.95))
    const zoom = Math.min(1, capW / naturalW, capH / naturalH)

    const wLogical = Math.max(320, Math.round(naturalW * zoom))
    const hLogical = Math.max(300, Math.round(naturalH * zoom))
    root.style.height = '100vh'
    void getCurrentWebview().setZoom(zoom)
    void appWindow.setSize(new LogicalSize(wLogical, hLogical))
  }

  /**
   * Schedule `cb` to run after two animation frames.
   *
   * Two rAFs — not one — because Vue's async scheduler flushes
   * on the microtask immediately after the first rAF, so the DOM is
   * usually correct on the *second* rAF. Concrete symptom if we
   * only used one: `LoginRegionSelection.vue::watch(config.loaded)`
   * auto-redirects to `/login/id-pass` on the first post-mount
   * tick, and a single-rAF measurement would race that redirect.
   *
   * Cancels any previously pending callback so an `afterEach` storm
   * collapses to a single fit on the final settled destination.
   */
  function scheduleOnNextPaint(cb: () => void): void {
    if (typeof window === 'undefined' || typeof window.requestAnimationFrame !== 'function') {
      // jsdom / SSR path — just run immediately so specs don't need
      // a rAF polyfill.
      cb()
      return
    }
    if (pendingFrame !== null) window.cancelAnimationFrame(pendingFrame)
    pendingFrame = window.requestAnimationFrame(() => {
      pendingFrame = window.requestAnimationFrame(() => {
        pendingFrame = null
        cb()
      })
    })
  }

  /**
   * Tear down the previous route's observer, attach a fresh one to
   * the new route's `[data-window-root]` *and* its
   * `[data-window-content]` inner wrapper (if present), then perform
   * the first fit.
   *
   * # Why two observation targets (issue #236 follow-up)
   *
   * `[data-window-root]` is always `100vh` so `ResizeObserver`
   * callbacks never fire on changes that leave the outer frame alone
   * — e.g. a language switch that lengthens a label inside the page,
   * or async data arriving into the body after the initial fit.
   * Observing the inner `[data-window-content]` wrapper (added to
   * every top-level page template) catches these content-only height
   * changes so the window re-fits automatically. The outer root is
   * still observed so size changes to the window itself (F11, DPI
   * change) continue to trigger a re-fit.
   *
   * # Why the `initialNotificationsIgnored` rAF gate
   *
   * `ResizeObserver.observe()` fires a synthetic notification for
   * each observed target immediately after attaching, *before* the
   * first paint. The manual `fitWindow()` at the end of this
   * function already covers that initial measurement, so the
   * synthetic fires would double-fit. With two targets the old
   * "skip first callback" flag was racy (the two initials might
   * arrive in one callback or two depending on the browser), so we
   * instead swallow every callback until the first post-attach rAF
   * flips the gate — guaranteed to be after all synthetic initials
   * (they fire in the same microtask as `observe()`).
   */
  function attachObserver(): void {
    if (observer) observer.disconnect()
    const root = document.querySelector('[data-window-root]') as HTMLElement | null
    if (!root) return
    const content = root.querySelector('[data-window-content]') as HTMLElement | null
    let initialNotificationsIgnored = false
    observer = new ResizeObserver(() => {
      if (!initialNotificationsIgnored) return
      scheduleOnNextPaint(fitWindow)
    })
    observer.observe(root)
    if (content) observer.observe(content)

    // Also re-fit when the OS font size / DPI changes (Windows
    // Accessibility text-size slider changes the CSS rem base,
    // which ResizeObserver may not catch if the root stays 100vh).
    // A 1rem sentinel element detects font-size changes.
    if (typeof window !== 'undefined') {
      window.addEventListener('resize', () => scheduleOnNextPaint(fitWindow), { passive: true })
      // `workArea()` is read live inside `fitWindow`, so a moved window /
      // changed DPI just needs a re-fit — no cached metrics to refresh.
      scheduleOnNextPaint(fitWindow)

      // Sentinel: a 0-size element whose width is 1rem. When the
      // system font size changes, its pixel width changes and
      // ResizeObserver fires, triggering a re-fit.
      let sentinel = document.getElementById('__fit-sentinel')
      if (!sentinel) {
        sentinel = document.createElement('div')
        sentinel.id = '__fit-sentinel'
        sentinel.style.cssText =
          'position:fixed;top:-9999px;left:-9999px;width:1rem;height:1rem;pointer-events:none;'
        document.body.appendChild(sentinel)
      }
      observer.observe(sentinel)
    }
    if (typeof window !== 'undefined' && typeof window.requestAnimationFrame === 'function') {
      window.requestAnimationFrame(() => {
        initialNotificationsIgnored = true
      })
    } else {
      // jsdom / SSR path — the harness stubs `ResizeObserver` so
      // the gate never matters, but flipping immediately keeps the
      // behaviour consistent with real browsers in case a test ever
      // exercises the observer path.
      initialNotificationsIgnored = true
    }
    fitWindow()
  }

  router.afterEach((to) => {
    const w = to.meta.windowWidth as number | undefined
    if (w) currentWidth = w
    scheduleOnNextPaint(attachObserver)
  })
}
