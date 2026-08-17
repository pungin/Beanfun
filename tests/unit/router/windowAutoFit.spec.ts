/**
 * Specs for the content-fit resizer's idempotence (issue #367).
 *
 * The resizer measures content in CSS px and applies the result in
 * rounded logical px, so a fixed point is not guaranteed: a height that
 * rounds to H can measure back as H±1. Re-applying that echo fires the
 * ResizeObserver, which fits again, which re-applies — the window
 * jitters by a pixel for as long as the page is open.
 *
 * These tests pin the guard that breaks the cycle: apply only what is
 * not already applied.
 */

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

const setSize = vi.fn()
const setZoom = vi.fn()

vi.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: () => ({ setSize }),
}))
vi.mock('@tauri-apps/api/webview', () => ({
  getCurrentWebview: () => ({ setZoom }),
}))
vi.mock('element-plus', () => ({ ElMessage: { error: vi.fn() } }))

import { createAppRouter, installRouterGuards } from '../../../src/router'
import { setWindowFitSuspended } from '../../../src/services/windowFit'

/** Fires whatever the resizer observed, on demand. */
let notifyResize: (() => void) | null = null

/** Content height the fake `[data-window-root]` reports. */
let contentHeight = 500

function installDom(): void {
  document.body.innerHTML = '<main data-window-root></main>'
  const root = document.querySelector('[data-window-root]') as HTMLElement
  Object.defineProperty(root, 'scrollHeight', { get: () => contentHeight, configurable: true })
}

beforeEach(() => {
  setSize.mockClear()
  setZoom.mockClear()
  notifyResize = null
  contentHeight = 500
  setWindowFitSuspended(false)
  installDom()

  // Run scheduled work immediately so a fit is synchronous with the
  // notification that triggered it.
  vi.stubGlobal('requestAnimationFrame', ((cb: FrameRequestCallback) => {
    cb(0)
    return 1
  }) as typeof requestAnimationFrame)
  vi.stubGlobal('cancelAnimationFrame', () => {})
  vi.stubGlobal(
    'ResizeObserver',
    class {
      constructor(cb: () => void) {
        notifyResize = cb
      }
      observe(): void {}
      disconnect(): void {}
    },
  )
})

afterEach(() => {
  vi.unstubAllGlobals()
  document.body.innerHTML = ''
})

async function install(): Promise<void> {
  const router = createAppRouter()
  installRouterGuards(router, { isAuthenticated: () => true, clearSession: () => {} })
  await router.push('/settings')
  await router.isReady()
}

describe('content-fit resizer idempotence (#367)', () => {
  it('resizes once for a given content height, however often it is notified', async () => {
    await install()
    const afterFirstFit = setSize.mock.calls.length
    expect(afterFirstFit).toBeGreaterThan(0)

    // Every notification re-measures the same content. Without the
    // guard each one issues another setSize, and each setSize provokes
    // the next notification — the jitter loop.
    for (let i = 0; i < 5; i += 1) notifyResize?.()

    expect(setSize.mock.calls.length).toBe(afterFirstFit)
  })

  it('still resizes when the content genuinely changes', async () => {
    await install()
    const before = setSize.mock.calls.length

    contentHeight = 900
    notifyResize?.()

    expect(setSize.mock.calls.length).toBeGreaterThan(before)
  })

  it('does not re-apply the same zoom', async () => {
    await install()
    const before = setZoom.mock.calls.length

    for (let i = 0; i < 3; i += 1) notifyResize?.()

    expect(setZoom.mock.calls.length).toBe(before)
  })

  it('re-applies after an overlay held the window at its own size', async () => {
    await install()
    const before = setSize.mock.calls.length

    // While suspended the overlay resizes the window itself, so what we
    // last applied is stale — the restoring fit must not be skipped as
    // "already applied".
    setWindowFitSuspended(true)
    notifyResize?.()
    expect(setSize.mock.calls.length).toBe(before)

    setWindowFitSuspended(false)
    notifyResize?.()

    expect(setSize.mock.calls.length).toBeGreaterThan(before)
  })
})
