import { describe, expect, it } from 'vitest'
import { createAppRouter, ROUTE_NAMES, routes } from '../../../src/router'

describe('router config', () => {
  it('declares only the placeholder route + catch-all redirect during P11', () => {
    expect(routes).toHaveLength(2)
    expect(routes[0].path).toBe('/')
    expect(routes[0].name).toBe(ROUTE_NAMES.Placeholder)
    expect(routes[1].path).toBe('/:pathMatch(.*)*')
    expect(routes[1].redirect).toBe('/')
  })

  it('exports stable route name constants for downstream pages', () => {
    expect(ROUTE_NAMES.Placeholder).toBe('placeholder')
  })
})

describe('createAppRouter', () => {
  it('resolves "/" to the placeholder route', async () => {
    const router = createAppRouter()
    await router.push('/')
    await router.isReady()
    expect(router.currentRoute.value.name).toBe(ROUTE_NAMES.Placeholder)
  })

  it('redirects unknown paths back to /', async () => {
    const router = createAppRouter()
    await router.push('/not-a-real-route')
    await router.isReady()
    expect(router.currentRoute.value.path).toBe('/')
    expect(router.currentRoute.value.name).toBe(ROUTE_NAMES.Placeholder)
  })

  it('returns a fresh instance per call (no shared singleton state)', () => {
    const a = createAppRouter()
    const b = createAppRouter()
    expect(a).not.toBe(b)
    expect(a.options.routes).toEqual(b.options.routes)
  })
})
