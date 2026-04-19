import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createMemoryHistory, createRouter, type Router } from 'vue-router'
import {
  createAppRouter,
  installRouterGuards,
  LOGGED_IN_LANDING_PATH,
  ROUTE_NAMES,
  routes,
} from '../../../src/router'
import { __resetInvokeRegistriesForTesting } from '../../../src/services/invoke'

vi.mock('element-plus', () => ({ ElMessage: { error: vi.fn() } }))

/**
 * P12.1 D1 + D2 + D3 + D4 + D5 CP2 + D6 + D7 + D8 + D10 — login
 * shell routing with the region picker (default `/login`), id-pass
 * form (`/login/id-pass`), QR form (`/login/qr`), GamePass form
 * (`/login/gamepass`), TOTP challenge (`/login/totp`), the
 * "logging in…" wait page (`/login/wait`), the AdvanceCheck verify
 * page (`/login/verify`) wired up, plus the D10 router guards
 * (`requiresAuth` `beforeEach` + `auth.session_required` bridge).
 *
 * This spec asserts the scaffolding plus every D2-D8 child route:
 * root redirects to `/login`, `/login` lands on the region picker
 * (named empty-path child), `/login/id-pass` resolves to the
 * id-pass form, `/login/qr` resolves to the QR form,
 * `/login/gamepass` resolves to the GamePass form, `/login/totp`
 * resolves to the TOTP form, `/login/wait` resolves to the wait
 * holding page, `/login/verify` resolves to the verify page, and
 * unknown paths fall back via the catch-all. The D10 guard suite
 * exercises `installRouterGuards` over a memory-history router
 * with synthetic protected routes (P12.1 ships zero protected
 * routes; the spec proves the guard infrastructure is correct so
 * P12.2 D-steps just set `meta.requiresAuth: true` and inherit it).
 * Per-form behaviour tests live in their own D-step specs.
 */
describe('router config', () => {
  it('declares the root redirect, login shell, accounts page, manage-account page, and catch-all', () => {
    /*
     * P12.2 D9 added the stored-credential management page
     * (`/manage-account`) as a 5th top-level route, sandwiched
     * between the post-login landing (`/accounts`) and the
     * catch-all so the redirect short-circuit at the bottom still
     * wins for unknown paths.
     */
    expect(routes).toHaveLength(5)

    const [root, login, accounts, manageAccount, catchAll] = routes
    expect(root.path).toBe('/')
    expect(root.redirect).toBe('/login')

    expect(login.path).toBe('/login')
    expect(login.name).toBeUndefined()

    expect(accounts.path).toBe(LOGGED_IN_LANDING_PATH)
    expect(accounts.name).toBe(ROUTE_NAMES.Accounts)
    expect(accounts.meta?.requiresAuth).toBe(true)

    expect(manageAccount.path).toBe('/manage-account')
    expect(manageAccount.name).toBe(ROUTE_NAMES.ManageAccount)
    expect(manageAccount.meta?.requiresAuth).toBe(true)

    expect(catchAll.path).toBe('/:pathMatch(.*)*')
    expect(catchAll.redirect).toBe('/')
  })

  it('declares the region picker as the default login child', () => {
    const [, login] = routes
    const children = login.children ?? []
    expect(children.length).toBeGreaterThanOrEqual(1)

    const regionChild = children.find((child) => child.path === '')
    expect(regionChild).toBeDefined()
    expect(regionChild!.name).toBe(ROUTE_NAMES.LoginRegion)
    expect(regionChild!.component).toBeDefined()
  })

  it('declares the id-pass form as the /login/id-pass child', () => {
    const [, login] = routes
    const children = login.children ?? []

    const idPassChild = children.find((child) => child.path === 'id-pass')
    expect(idPassChild).toBeDefined()
    expect(idPassChild!.name).toBe(ROUTE_NAMES.LoginIdPass)
    expect(idPassChild!.component).toBeDefined()
  })

  it('declares the QR form as the /login/qr child', () => {
    const [, login] = routes
    const children = login.children ?? []

    const qrChild = children.find((child) => child.path === 'qr')
    expect(qrChild).toBeDefined()
    expect(qrChild!.name).toBe(ROUTE_NAMES.LoginQr)
    expect(qrChild!.component).toBeDefined()
  })

  it('declares the GamePass form as the /login/gamepass child', () => {
    const [, login] = routes
    const children = login.children ?? []

    const gamepassChild = children.find((child) => child.path === 'gamepass')
    expect(gamepassChild).toBeDefined()
    expect(gamepassChild!.name).toBe(ROUTE_NAMES.LoginGamepass)
    expect(gamepassChild!.component).toBeDefined()
  })

  it('declares the TOTP form as the /login/totp child', () => {
    const [, login] = routes
    const children = login.children ?? []

    const totpChild = children.find((child) => child.path === 'totp')
    expect(totpChild).toBeDefined()
    expect(totpChild!.name).toBe(ROUTE_NAMES.LoginTotp)
    expect(totpChild!.component).toBeDefined()
  })

  it('declares the wait page as the /login/wait child', () => {
    const [, login] = routes
    const children = login.children ?? []

    const waitChild = children.find((child) => child.path === 'wait')
    expect(waitChild).toBeDefined()
    expect(waitChild!.name).toBe(ROUTE_NAMES.LoginWait)
    expect(waitChild!.component).toBeDefined()
  })

  it('declares the verify page as the /login/verify child', () => {
    const [, login] = routes
    const children = login.children ?? []

    const verifyChild = children.find((child) => child.path === 'verify')
    expect(verifyChild).toBeDefined()
    expect(verifyChild!.name).toBe(ROUTE_NAMES.LoginVerify)
    expect(verifyChild!.component).toBeDefined()
  })

  it('exports stable route-name constants for every named route', () => {
    expect(ROUTE_NAMES.LoginRegion).toBe('login-region')
    expect(ROUTE_NAMES.LoginIdPass).toBe('login-id-pass')
    expect(ROUTE_NAMES.LoginQr).toBe('login-qr')
    expect(ROUTE_NAMES.LoginGamepass).toBe('login-gamepass')
    expect(ROUTE_NAMES.LoginTotp).toBe('login-totp')
    expect(ROUTE_NAMES.LoginWait).toBe('login-wait')
    expect(ROUTE_NAMES.LoginVerify).toBe('login-verify')
    expect(ROUTE_NAMES.Accounts).toBe('accounts')
    expect(ROUTE_NAMES.ManageAccount).toBe('manage-account')
  })
})

describe('createAppRouter', () => {
  it('redirects "/" to /login, which lands on the region picker', async () => {
    const router = createAppRouter()
    await router.push('/')
    await router.isReady()
    expect(router.currentRoute.value.name).toBe(ROUTE_NAMES.LoginRegion)
    expect(router.currentRoute.value.path).toBe('/login')
  })

  it('resolves "/login" to the region picker (named empty-path child)', async () => {
    const router = createAppRouter()
    await router.push('/login')
    await router.isReady()
    expect(router.currentRoute.value.name).toBe(ROUTE_NAMES.LoginRegion)
    expect(router.currentRoute.value.path).toBe('/login')
  })

  it('resolves the LoginRegion route name back to /login', async () => {
    const router = createAppRouter()
    await router.push({ name: ROUTE_NAMES.LoginRegion })
    await router.isReady()
    expect(router.currentRoute.value.path).toBe('/login')
  })

  it('routes unknown paths back through the catch-all to /login', async () => {
    const router = createAppRouter()
    await router.push('/not-a-real-route')
    await router.isReady()
    expect(router.currentRoute.value.path).toBe('/login')
    expect(router.currentRoute.value.name).toBe(ROUTE_NAMES.LoginRegion)
  })

  it('resolves /login/id-pass to the IdPassForm route', async () => {
    const router = createAppRouter()
    await router.push('/login/id-pass')
    await router.isReady()
    expect(router.currentRoute.value.name).toBe(ROUTE_NAMES.LoginIdPass)
    expect(router.currentRoute.value.path).toBe('/login/id-pass')
  })

  it('resolves /login/qr to the QrForm route', async () => {
    const router = createAppRouter()
    await router.push('/login/qr')
    await router.isReady()
    expect(router.currentRoute.value.name).toBe(ROUTE_NAMES.LoginQr)
    expect(router.currentRoute.value.path).toBe('/login/qr')
  })

  it('resolves /login/gamepass to the GamepassForm route', async () => {
    const router = createAppRouter()
    await router.push('/login/gamepass')
    await router.isReady()
    expect(router.currentRoute.value.name).toBe(ROUTE_NAMES.LoginGamepass)
    expect(router.currentRoute.value.path).toBe('/login/gamepass')
  })

  it('resolves /login/totp to the LoginTotp route', async () => {
    const router = createAppRouter()
    await router.push('/login/totp')
    await router.isReady()
    expect(router.currentRoute.value.name).toBe(ROUTE_NAMES.LoginTotp)
    expect(router.currentRoute.value.path).toBe('/login/totp')
  })

  it('resolves /login/wait to the LoginWait route', async () => {
    const router = createAppRouter()
    await router.push('/login/wait')
    await router.isReady()
    expect(router.currentRoute.value.name).toBe(ROUTE_NAMES.LoginWait)
    expect(router.currentRoute.value.path).toBe('/login/wait')
  })

  it('resolves /login/verify to the VerifyPage route', async () => {
    const router = createAppRouter()
    await router.push('/login/verify')
    await router.isReady()
    expect(router.currentRoute.value.name).toBe(ROUTE_NAMES.LoginVerify)
    expect(router.currentRoute.value.path).toBe('/login/verify')
  })

  it('resolves /accounts to the AccountList route (post-login landing)', async () => {
    const router = createAppRouter()
    /*
     * Without the D10 guard installed, even an unauthenticated visit
     * resolves the route — the guard is what redirects, the route
     * itself is always reachable. The guard-installed integration is
     * exercised separately below.
     */
    await router.push(LOGGED_IN_LANDING_PATH)
    await router.isReady()
    expect(router.currentRoute.value.name).toBe(ROUTE_NAMES.Accounts)
    expect(router.currentRoute.value.path).toBe('/accounts')
    expect(router.currentRoute.value.meta.requiresAuth).toBe(true)
  })

  it('resolves /manage-account to the ManageAccount route (P12.2 D9)', async () => {
    /*
     * D9 ships only the route + page — no in-app entry button yet
     * (the Settings entry lands in P12.4). Direct hash navigation
     * (`#/manage-account`) is the only path today, so the route
     * resolution itself is the public contract this spec locks in.
     * `requiresAuth: true` mirrors `/accounts`; the guard-installed
     * redirect is covered by the integration block below.
     */
    const router = createAppRouter()
    await router.push('/manage-account')
    await router.isReady()
    expect(router.currentRoute.value.name).toBe(ROUTE_NAMES.ManageAccount)
    expect(router.currentRoute.value.path).toBe('/manage-account')
    expect(router.currentRoute.value.meta.requiresAuth).toBe(true)
  })

  it('returns a fresh instance per call (no shared singleton state)', () => {
    const a = createAppRouter()
    const b = createAppRouter()
    expect(a).not.toBe(b)
    expect(a.options.routes).toEqual(b.options.routes)
  })
})

describe('LOGGED_IN_LANDING_PATH', () => {
  it('points at the AccountList route registered in `routes`', () => {
    /*
     * The constant + route entry must agree — every login-success
     * call site (`IdPassForm` / `LoginTotp` / `QrForm` /
     * `GamepassForm`) pushes `LOGGED_IN_LANDING_PATH`, so a typo in
     * either side would silently route the user back through the
     * catch-all to /login. This test makes the contract explicit.
     */
    expect(LOGGED_IN_LANDING_PATH).toBe('/accounts')
    const accountsRoute = routes.find((r) => r.path === LOGGED_IN_LANDING_PATH)
    expect(accountsRoute).toBeDefined()
    expect(accountsRoute!.name).toBe(ROUTE_NAMES.Accounts)
  })
})

describe('installRouterGuards — integration with production /accounts route', () => {
  beforeEach(() => {
    __resetInvokeRegistriesForTesting()
  })
  afterEach(() => {
    __resetInvokeRegistriesForTesting()
  })

  it('redirects unauthenticated /accounts visits back to /login with the deep link preserved', async () => {
    /*
     * End-to-end check that the production route table's
     * `meta.requiresAuth: true` on `/accounts` is actually wired to
     * the D10 guard. The synthetic-router specs above prove the
     * guard semantics; this case proves the **integration** so a
     * future contributor that forgets the meta flag (or removes
     * `installRouterGuards` from `main.ts`) trips a red test.
     */
    const router = createAppRouter()
    installRouterGuards(router, { isAuthenticated: () => false, clearSession: () => {} })

    await router.push('/accounts')
    await router.isReady()

    expect(router.currentRoute.value.path).toBe('/login')
    expect(router.currentRoute.value.query.redirect).toBe('/accounts')
  })

  it('lets authenticated /accounts visits land on the AccountList route', async () => {
    const router = createAppRouter()
    installRouterGuards(router, { isAuthenticated: () => true, clearSession: () => {} })

    await router.push('/accounts')
    await router.isReady()

    expect(router.currentRoute.value.name).toBe(ROUTE_NAMES.Accounts)
    expect(router.currentRoute.value.path).toBe('/accounts')
  })

  it('redirects unauthenticated /manage-account visits back to /login with the deep link preserved', async () => {
    /*
     * Same contract as `/accounts`: D9 marks `/manage-account` as
     * `requiresAuth: true`, and the production guard must honour
     * the meta flag end-to-end. A missing flag would silently let
     * a logged-out user land on the credential CRUD page.
     */
    const router = createAppRouter()
    installRouterGuards(router, { isAuthenticated: () => false, clearSession: () => {} })

    await router.push('/manage-account')
    await router.isReady()

    expect(router.currentRoute.value.path).toBe('/login')
    expect(router.currentRoute.value.query.redirect).toBe('/manage-account')
  })

  it('lets authenticated /manage-account visits land on the ManageAccount route', async () => {
    const router = createAppRouter()
    installRouterGuards(router, { isAuthenticated: () => true, clearSession: () => {} })

    await router.push('/manage-account')
    await router.isReady()

    expect(router.currentRoute.value.name).toBe(ROUTE_NAMES.ManageAccount)
    expect(router.currentRoute.value.path).toBe('/manage-account')
  })
})

describe('installRouterGuards — requiresAuth beforeEach', () => {
  /*
   * Build a minimal in-memory router with two synthetic routes
   * (one public, one protected) so the guard's behaviour is tested
   * in isolation from the production route table. Using
   * `createMemoryHistory` instead of hash history keeps tests
   * deterministic without a jsdom URL.
   */
  function buildSyntheticRouter(): Router {
    return createRouter({
      history: createMemoryHistory(),
      routes: [
        { path: '/', component: { template: '<div />' } },
        { path: '/public', component: { template: '<div />' } },
        {
          path: '/protected',
          component: { template: '<div />' },
          meta: { requiresAuth: true },
        },
        {
          path: '/protected-deep/:id',
          component: { template: '<div />' },
          meta: { requiresAuth: true },
        },
        { path: '/login', component: { template: '<div />' } },
      ],
    })
  }

  beforeEach(() => {
    __resetInvokeRegistriesForTesting()
  })
  afterEach(() => {
    __resetInvokeRegistriesForTesting()
  })

  it('lets navigations to public routes through regardless of auth state', async () => {
    const router = buildSyntheticRouter()
    installRouterGuards(router, { isAuthenticated: () => false, clearSession: () => {} })

    await router.push('/public')
    await router.isReady()
    expect(router.currentRoute.value.path).toBe('/public')
  })

  it('redirects unauthenticated navigation to a requiresAuth route to /login', async () => {
    const router = buildSyntheticRouter()
    installRouterGuards(router, { isAuthenticated: () => false, clearSession: () => {} })

    await router.push('/protected')
    await router.isReady()
    expect(router.currentRoute.value.path).toBe('/login')
  })

  it('carries the original path under ?redirect=... so post-login can replay deep links', async () => {
    const router = buildSyntheticRouter()
    installRouterGuards(router, { isAuthenticated: () => false, clearSession: () => {} })

    await router.push('/protected-deep/42?foo=bar')
    await router.isReady()
    expect(router.currentRoute.value.path).toBe('/login')
    expect(router.currentRoute.value.query.redirect).toBe('/protected-deep/42?foo=bar')
  })

  it('lets authenticated navigation to a requiresAuth route through unchanged', async () => {
    const router = buildSyntheticRouter()
    installRouterGuards(router, { isAuthenticated: () => true, clearSession: () => {} })

    await router.push('/protected')
    await router.isReady()
    expect(router.currentRoute.value.path).toBe('/protected')
  })

  it('does not encode a redirect query when the protected target is itself inside the /login funnel', async () => {
    /*
     * Defence in depth: if a future misconfiguration marks
     * `/login/something` as `requiresAuth: true`, the guard must
     * NOT push `/login?redirect=/login/something` — that would
     * push the user back into the same protected route after a
     * theoretical post-login deep-link replay, looping forever.
     * The production route table never marks a login child
     * requiresAuth (the guard's path-funnel check guards against
     * the misconfiguration becoming user-visible).
     *
     * We test by mocking a hostile config and asserting the redirect
     * query is suppressed before vue-router's own infinite-redirect
     * detector kicks in (which would also catch the loop, just less
     * informatively).
     */
    const router = createRouter({
      history: createMemoryHistory(),
      routes: [
        { path: '/login', component: { template: '<div />' } },
        {
          path: '/login/protected',
          component: { template: '<div />' },
          meta: { requiresAuth: true },
        },
      ],
    })
    installRouterGuards(router, { isAuthenticated: () => false, clearSession: () => {} })

    await router.push('/login/protected')
    await router.isReady()
    expect(router.currentRoute.value.path).toBe('/login')
    expect(router.currentRoute.value.query.redirect).toBeUndefined()
  })
})

describe('installRouterGuards — session-expired bridge', () => {
  beforeEach(() => {
    __resetInvokeRegistriesForTesting()
  })
  afterEach(() => {
    __resetInvokeRegistriesForTesting()
  })

  it('registers a handler that calls deps.clearSession on auth.session_required', async () => {
    const router = createAppRouter()
    const clearSession = vi.fn()
    installRouterGuards(router, { isAuthenticated: () => false, clearSession })

    /*
     * Trip the handler the same way the production code does — by
     * surfacing an `auth.session_required` `CommandError` through
     * `surfaceCommandError`, which is the single fan-out point that
     * looks up the registered session-expired hook. `silent: true`
     * suppresses the toast so the spec stays focused on the
     * router-side wiring (the toast itself is exercised by the
     * `services/invoke.ts` spec).
     */
    const { surfaceCommandError } = await import('../../../src/services/invoke')
    surfaceCommandError(
      { code: 'auth.session_required', message: 'gone', details: null },
      { silent: true },
    )

    expect(clearSession).toHaveBeenCalledTimes(1)
  })

  it('routes the user back to /login?sessionExpired=1 when the handler fires', async () => {
    const router = createAppRouter()
    await router.push('/login/id-pass')
    await router.isReady()
    expect(router.currentRoute.value.path).toBe('/login/id-pass')

    const clearSession = vi.fn()
    installRouterGuards(router, { isAuthenticated: () => false, clearSession })

    const { surfaceCommandError } = await import('../../../src/services/invoke')
    surfaceCommandError(
      { code: 'auth.session_required', message: 'gone', details: null },
      { silent: true },
    )

    /* Allow the queued router.push microtask to flush. */
    await new Promise((r) => setTimeout(r, 0))
    expect(router.currentRoute.value.path).toBe('/login')
    expect(router.currentRoute.value.query.sessionExpired).toBe('1')
  })

  it('also fires clearAccountSession (when supplied) so non-auth stores wipe in lockstep', async () => {
    /*
     * P12.2 D1 fix-up of the D10 bug. Production wiring in
     * `main.ts` passes both `clearSession` (auth store) and
     * `clearAccountSession` (account store's
     * `clearSessionData()`); the bridge must fire both so a
     * subsequent re-login doesn't flash the previous session's
     * cached service-account list / email / remain-point /
     * contract while the new fetch runs.
     *
     * Order matters loosely: `clearSession` first so the
     * `isLoggedIn` flag flips before any reactive watcher on
     * the account store reacts to the wipe. We assert the
     * relative order via the call-time index.
     */
    const router = createAppRouter()
    const order: string[] = []
    const clearSession = vi.fn(() => {
      order.push('auth')
    })
    const clearAccountSession = vi.fn(() => {
      order.push('account')
    })
    installRouterGuards(router, {
      isAuthenticated: () => false,
      clearSession,
      clearAccountSession,
    })

    const { surfaceCommandError } = await import('../../../src/services/invoke')
    surfaceCommandError(
      { code: 'auth.session_required', message: 'gone', details: null },
      { silent: true },
    )

    expect(clearSession).toHaveBeenCalledTimes(1)
    expect(clearAccountSession).toHaveBeenCalledTimes(1)
    expect(order).toEqual(['auth', 'account'])
  })

  it('omits clearAccountSession gracefully when the caller did not supply it (back-compat)', async () => {
    /*
     * The `clearAccountSession` field is `?: () => void`, so
     * pre-D1 specs that only stub `clearSession` keep working.
     * The bridge must not throw when the field is undefined.
     */
    const router = createAppRouter()
    const clearSession = vi.fn()
    installRouterGuards(router, { isAuthenticated: () => false, clearSession })

    const { surfaceCommandError } = await import('../../../src/services/invoke')
    expect(() =>
      surfaceCommandError(
        { code: 'auth.session_required', message: 'gone', details: null },
        { silent: true },
      ),
    ).not.toThrow()

    expect(clearSession).toHaveBeenCalledTimes(1)
  })
})
