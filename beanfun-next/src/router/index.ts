/**
 * Vue Router setup — hash-mode SPA with a single root placeholder
 * route during P11 infra phase.
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
 * # Why a single placeholder route in P11?
 *
 * P11 is the infra phase: i18n / Pinia / theme / IPC plumbing. The
 * placeholder lets `App.vue` render *something* via `<RouterView />`
 * so we can verify the whole frontend pipeline boots end-to-end
 * (i18n keys resolve, theme color applies, IPC round-trip works) —
 * before P12 starts wiring real pages. Each page added in P12 will
 * register its own route in this file alongside the placeholder.
 *
 * # 404 handling
 *
 * Catch-all `/:pathMatch(.*)*` redirects to `/`. Returning a real
 * 404 page is a P12 concern — until the rest of the app has its own
 * pages, "I typed something weird in dev tools" can just fall back
 * to the only available route.
 */

import { createRouter, createWebHashHistory } from 'vue-router'
import type { RouteRecordRaw } from 'vue-router'
import Placeholder from '../pages/Placeholder.vue'

export const ROUTE_NAMES = {
  Placeholder: 'placeholder',
} as const

export const routes: RouteRecordRaw[] = [
  {
    path: '/',
    name: ROUTE_NAMES.Placeholder,
    component: Placeholder,
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
