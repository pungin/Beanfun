import { describe, expect, it } from 'vitest'
import { createAppRouter, ROUTE_NAMES, routes } from '../../../src/router'

/**
 * P12.1 D1 + D2 + D3 + D4 + D5 CP2 — login shell routing with the
 * region picker (default `/login`), id-pass form
 * (`/login/id-pass`), QR form (`/login/qr`), and GamePass form
 * (`/login/gamepass`) wired up.
 *
 * Sub-routes for the remaining login forms (`/login/totp`,
 * `/login/wait`, `/login/verify`) land via D6-D8. This spec asserts
 * the scaffolding plus the D2/D3/D4/D5 child routes: root redirects
 * to `/login`, `/login` lands on the region picker (named empty-path
 * child), `/login/id-pass` resolves to the id-pass form, `/login/qr`
 * resolves to the QR form, `/login/gamepass` resolves to the
 * GamePass form, and unknown paths fall back via the catch-all.
 * Per-form behaviour tests live in their own D-step specs.
 */
describe('router config', () => {
  it('declares the root redirect, login shell, and catch-all', () => {
    expect(routes).toHaveLength(3)

    const [root, login, catchAll] = routes
    expect(root.path).toBe('/')
    expect(root.redirect).toBe('/login')

    expect(login.path).toBe('/login')
    expect(login.name).toBeUndefined()

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

  it('exports stable login route-name constants', () => {
    expect(ROUTE_NAMES.LoginRegion).toBe('login-region')
    expect(ROUTE_NAMES.LoginIdPass).toBe('login-id-pass')
    expect(ROUTE_NAMES.LoginQr).toBe('login-qr')
    expect(ROUTE_NAMES.LoginGamepass).toBe('login-gamepass')
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

  it('returns a fresh instance per call (no shared singleton state)', () => {
    const a = createAppRouter()
    const b = createAppRouter()
    expect(a).not.toBe(b)
    expect(a.options.routes).toEqual(b.options.routes)
  })
})
