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

import { registerSessionExpiredHandler } from '../services/invoke'

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
  },
  {
    path: 'id-pass',
    name: ROUTE_NAMES.LoginIdPass,
    component: IdPassForm,
  },
  {
    path: 'qr',
    name: ROUTE_NAMES.LoginQr,
    component: QrForm,
  },
  {
    path: 'gamepass',
    name: ROUTE_NAMES.LoginGamepass,
    component: GamepassForm,
  },
  {
    path: 'totp',
    name: ROUTE_NAMES.LoginTotp,
    component: LoginTotp,
  },
  {
    path: 'wait',
    name: ROUTE_NAMES.LoginWait,
    component: LoginWait,
  },
  {
    path: 'verify',
    name: ROUTE_NAMES.LoginVerify,
    component: VerifyPage,
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
    meta: { requiresAuth: true },
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
    meta: { requiresAuth: true },
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
}
