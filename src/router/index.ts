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
 * /manage-account                  ManageAccount             (P12.2 D9 ✓, requiresAuth)
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

import { getCurrentWindow } from '@tauri-apps/api/window'
import { LogicalSize } from '@tauri-apps/api/dpi'

import { registerSessionExpiredHandler } from '../services/invoke'

/**
 * Resize the Tauri window to the given logical dimensions.
 * Exported so individual pages can call it on mount when their
 * content height differs from the route meta default (e.g.
 * Settings page without the Game section when unauthenticated).
 */
export function resizeWindow(width: number, height: number): void {
  void getCurrentWindow().setSize(new LogicalSize(width, height))
}

import LoginPage from '../pages/LoginPage.vue'
import LoginRegionSelection from '../pages/LoginRegionSelection.vue'
import IdPassForm from '../pages/IdPassForm.vue'
import QrForm from '../pages/QrForm.vue'
import GamepassForm from '../pages/GamepassForm.vue'
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
      windowWidth: 560,
      windowHeight: 480,
    },
  },
  {
    path: 'id-pass',
    name: ROUTE_NAMES.LoginIdPass,
    component: IdPassForm,
    meta: { titleKey: 'titleBar.login', titleIcon: 'login', windowWidth: 560, windowHeight: 520 },
  },
  {
    path: 'qr',
    name: ROUTE_NAMES.LoginQr,
    component: QrForm,
    meta: {
      titleKey: 'titleBar.login',
      titleIcon: 'qr_code_2',
      windowWidth: 560,
      windowHeight: 680,
    },
  },
  {
    path: 'gamepass',
    name: ROUTE_NAMES.LoginGamepass,
    component: GamepassForm,
    meta: {
      titleKey: 'titleBar.login',
      titleIcon: 'verified_user',
      windowWidth: 560,
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
      windowWidth: 520,
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
      windowWidth: 480,
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
      windowWidth: 560,
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
      windowWidth: 560,
      windowHeight: 640,
    },
  },
  {
    path: '/settings',
    name: ROUTE_NAMES.Settings,
    component: SettingsPage,
    meta: {
      titleKey: 'titleBar.settings',
      titleIcon: 'settings',
      windowWidth: 880,
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
    meta: { titleKey: 'titleBar.about', titleIcon: 'info', windowWidth: 560, windowHeight: 680 },
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
     * delete + plaintext JSON import / export). Reachable today only
     * via direct hash navigation (`#/manage-account`); the Settings
     * page entry button lands in P12.4 alongside the other settings
     * surface — the WPF parent surface is `Setting.xaml` not
     * `Login.xaml`, so wiring an entry from `AccountList.vue` would
     * misplace the button. `requiresAuth: true` because the stored
     * accounts are session-scoped UX (the page only makes sense for
     * a logged-in user managing the credentials they just used).
     */
    meta: {
      requiresAuth: true,
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
    void router.push({
      path: '/login',
      query: { sessionExpired: '1' },
    })
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
   * Upper bound applied to the auto-fit height.
   *
   * Previously hard-coded to `900` which was narrower than several
   * pages (Settings with the Game section expanded, AccountList with
   * a populated service-account list) — the cap forced the inner
   * `__scroll` container to paint its own scrollbar and was the root
   * cause of issue #236 "returning from Settings still has a scroll
   * bar". Scaling to the actual display instead fits the content
   * naturally on any desktop without letting the window eat the
   * taskbar (50px safety margin keeps the window draggable on
   * Windows's default 40px taskbar).
   *
   * Falls back to 900 when `window.screen` is unavailable (jsdom /
   * headless CI) so the spec harness keeps working.
   */
  function maxFitHeight(): number {
    const avail = typeof window !== 'undefined' ? window.screen?.availHeight : undefined
    if (typeof avail === 'number' && avail > 0) {
      // Reserve space for the Windows taskbar + some breathing room.
      // On high-DPI displays (125%/150%), availHeight is already in
      // CSS pixels but the effective usable area is smaller because
      // the OS reserves more physical pixels for chrome. Using 80%
      // of availHeight ensures the window never clips off-screen.
      return Math.max(300, Math.floor(avail * 0.8))
    }
    return 900
  }

  /**
   * Measure the current `[data-window-root]`'s natural content
   * height and resize the OS window to match. Both the
   * `height: auto` measurement flip and the `height: 100vh` restore
   * land inside a single synchronous block so the browser never
   * paints the intermediate "unclipped" state — only one frame is
   * ever rendered per fit regardless of how long the IPC takes.
   *
   * Safe to call concurrently: successive invocations just re-run
   * the measurement cycle. The `pendingFrame` guard upstream keeps
   * the rate low enough that this isn't a hot path.
   */
  function fitWindow(): void {
    const root = document.querySelector('[data-window-root]') as HTMLElement | null
    if (!root) return
    // Both flips happen in the same synchronous block so the browser
    // never paints the intermediate `height: auto` state (it's a
    // forced-layout read followed by a write, both before the next
    // paint). See bug (3) in the header docblock above.
    root.style.height = 'auto'
    const h = Math.max(300, Math.min(Math.ceil(root.scrollHeight), maxFitHeight()))
    root.style.height = '100vh'
    void appWindow.setSize(new LogicalSize(currentWidth, h))
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
