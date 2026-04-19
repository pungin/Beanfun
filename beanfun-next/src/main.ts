/**
 * App entry point — wires Pinia, vue-i18n, Element Plus and Vue Router
 * onto the root component, then mounts.
 *
 * # Plugin order
 *
 * 1. **Pinia first.** Stores constructed inside `App.vue::setup()`
 *    require an active Pinia, so it must register before Vue invokes
 *    component setup (i.e. before `mount`).
 * 2. **i18n second + wireI18n.** vue-i18n only needs to be present
 *    when components call `useI18n()` / template `$t(...)`. We call
 *    `wireI18n` immediately after construction so the UI store's
 *    `setLanguage` action and the invoke layer's error toast already
 *    speak the right language by the time `App.vue` mounts.
 * 3. **Element Plus.** Element Plus's `<el-config-provider>` reads
 *    locale via prop binding from `App.vue`; the plugin install only
 *    needs to be done before any `<el-*>` template renders, which
 *    happens in `App.vue::onMounted`.
 * 4. **Router last.** No reverse dependency on the others; placing it
 *    last keeps a stable mental model ("everything plugins app
 *    services first, then add navigation").
 * 5. **`installRouterGuards` after `app.use(router)`.** D10 wires
 *    the `requiresAuth` guard plus the session-expired bridge here
 *    — needs the auth store (Pinia ready) and the router (just
 *    installed). Reads the store via `useAuthStore()`; safe to call
 *    here because Pinia was installed in step 1. Passes the store
 *    methods as bound functions so the guard layer never imports
 *    Pinia internals (see `router/index.ts::RouterGuardDeps`
 *    rationale).
 *
 * # Why no `pinia-plugin-persistedstate` install
 *
 * P11 Q5 = B: `Config.xml` is the single source of truth for
 * persistent UI state; mirroring the same data into localStorage
 * would create a "which copy wins on next launch?" sync conflict
 * for zero observable speed-up. The plugin stays in `package.json`
 * (cheap to add later if a P12 page genuinely needs ephemeral
 * frontend persistence) but is intentionally not registered here.
 */

import { createApp } from 'vue'
import { createPinia } from 'pinia'
import ElementPlus from 'element-plus'
import 'element-plus/dist/index.css'
/*
 * Project-wide design tokens + utility classes (P12.2 D1). Loaded
 * after Element Plus's stylesheet so our `--bf-*` custom properties
 * and `.bf-*` utilities can override / compose with ELP's base
 * resets without an `!important` arms race. Order between the two
 * project files matters: tokens declare `--bf-*` vars at `:root`,
 * utilities consume them.
 */
import './styles/design-tokens.css'
import './styles/utilities.css'

import App from './App.vue'
import { createAppI18n, wireI18n } from './i18n'
import { createAppRouter, installRouterGuards } from './router'
import { useAccountStore } from './stores/account'
import { useAuthStore } from './stores/auth'

const app = createApp(App)

const pinia = createPinia()
app.use(pinia)

const i18n = createAppI18n()
wireI18n(i18n)
app.use(i18n)

app.use(ElementPlus)

const router = createAppRouter()
app.use(router)

const auth = useAuthStore()
const account = useAccountStore()
/*
 * P12.2 D1 follow-up to the D10 session-expired bridge: also
 * wipe the account store's session-scoped cache (service accounts,
 * email, remain-point, contract, OTP selection) when the backend
 * reports the session is gone. Without this, a re-login briefly
 * flashes the previous user's service-account list while the
 * fresh `getServiceAccounts()` round-trip runs.
 *
 * Composition lives here (not inside `clearSession()` itself) so
 * the router and the auth store both stay unaware of which
 * non-auth Pinia stores carry session-scoped state — `main.ts` is
 * the only place where every store is in scope.
 */
installRouterGuards(router, {
  isAuthenticated: () => auth.isLoggedIn,
  clearSession: () => auth.clearSession(),
  clearAccountSession: () => account.clearSessionData(),
})

app.mount('#app')
