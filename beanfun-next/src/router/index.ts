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
 * # Route hierarchy (P12.1 — login flow)
 *
 * ```
 * /                                redirect → /login
 * /login                           LoginPage (shell with <RouterView />)
 *   "" (default child)             LoginRegionSelection      (D2 ✓)
 *   /login/id-pass                 IdPassForm                (D3 ✓)
 *   /login/qr                      QrForm                    (D4 ✓)
 *   /login/gamepass                GamepassForm              (D5 CP2 ✓)
 *   /login/totp                    LoginTotp                 (D6)
 *   /login/wait                    LoginWait                 (D7)
 *   /login/verify                  VerifyPage                (D8)
 * /:pathMatch(.*)*                 redirect → /
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
 */

import type { RouteRecordRaw } from 'vue-router'
import { createRouter, createWebHashHistory } from 'vue-router'

import LoginPage from '../pages/LoginPage.vue'
import LoginRegionSelection from '../pages/LoginRegionSelection.vue'
import IdPassForm from '../pages/IdPassForm.vue'
import QrForm from '../pages/QrForm.vue'
import GamepassForm from '../pages/GamepassForm.vue'

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
 * `main.ts` calls `createAppRouter()` exactly once.
 */
export function createAppRouter() {
  return createRouter({
    history: createWebHashHistory(),
    routes,
  })
}
