import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { flushPromises } from '@vue/test-utils'
import { createMemoryHistory, createRouter } from 'vue-router'

const mocks = vi.hoisted(() => ({
  resize: vi.fn<(width: number, height: number) => Promise<{ status: 'ok'; data: null }>>(
    async () => ({ status: 'ok', data: null }),
  ),
  zoom: vi.fn(async () => {}),
  monitor: vi.fn(async () => ({ workArea: { size: { width: 1280, height: 900 } } })),
  scaleFactor: vi.fn(async () => 1),
  onScaleChanged: vi.fn(async () => vi.fn()),
  onMoved: vi.fn(async () => vi.fn()),
}))
vi.mock('../../../src/types/bindings', () => ({ commands: { resizeMainWindow: mocks.resize } }))
vi.mock('element-plus', () => ({ ElMessage: { error: vi.fn() } }))
vi.mock('@tauri-apps/api/window', () => ({
  currentMonitor: mocks.monitor,
  getCurrentWindow: () => ({
    label: 'main',
    scaleFactor: mocks.scaleFactor,
    onScaleChanged: mocks.onScaleChanged,
    onMoved: mocks.onMoved,
  }),
}))
vi.mock('@tauri-apps/api/webview', () => ({ getCurrentWebview: () => ({ setZoom: mocks.zoom }) }))

import { installWindowAutoFit } from '../../../src/services/windowAutoFit'
import { routes as applicationRoutes } from '../../../src/router'
import {
  mainWindowGeometry,
  resetMainWindowGeometryForTests,
} from '../../../src/services/mainWindowGeometry'

let notifyResize: (() => void) | null = null
let contentHeight = 500
let dispose: (() => void) | undefined
let disconnect = vi.fn()

beforeEach(() => {
  vi.useFakeTimers()
  vi.clearAllMocks()
  resetMainWindowGeometryForTests()
  contentHeight = 500
  notifyResize = null
  disconnect = vi.fn()
  document.body.innerHTML = '<main data-window-root><div data-window-content></div></main>'
  const root = document.querySelector('[data-window-root]') as HTMLElement
  Object.defineProperty(root, 'scrollHeight', { get: () => contentHeight, configurable: true })
  vi.stubGlobal(
    'ResizeObserver',
    class {
      constructor(cb: () => void) {
        notifyResize = cb
      }
      observe(): void {}
      disconnect = disconnect
    },
  )
})

afterEach(() => {
  dispose?.()
  dispose = undefined
  vi.useRealTimers()
  vi.unstubAllGlobals()
  vi.restoreAllMocks()
  document.body.innerHTML = ''
})

async function settle(): Promise<void> {
  await vi.advanceTimersByTimeAsync(180)
  await flushPromises()
  await mainWindowGeometry().whenIdle()
}

async function install() {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: [
      { path: '/settings', component: {}, meta: { windowWidth: 660 } },
      { path: '/about', component: {}, meta: { windowWidth: 420 } },
      { path: '/manage-account', component: {}, meta: { windowWidth: 880 } },
      { path: '/browser-bar', component: {}, meta: { fitsWindow: false } },
    ],
  })
  dispose = installWindowAutoFit(router)
  await router.push('/settings')
  await settle()
  return router
}

describe('main-window content measurement', () => {
  it('preserves measured dimensions through every directed main-route transition and return', async () => {
    const catalogue = createRouter({ history: createMemoryHistory(), routes: applicationRoutes })
      .getRoutes()
      .filter((route) => route.meta.windowWidth !== undefined && route.meta.fitsWindow !== false)
    expect(catalogue).toHaveLength(12)
    const router = createRouter({
      history: createMemoryHistory(),
      routes: catalogue.map(({ path, meta }) => ({ path, meta, component: {} })),
    })
    dispose = installWindowAutoFit(router)
    let transitions = 0
    for (const [sourceIndex, source] of catalogue.entries()) {
      const sourceHeight = 240 + sourceIndex * 23
      contentHeight = sourceHeight
      await router.push(source.path)
      await settle()
      for (const [targetIndex, target] of catalogue.entries()) {
        if (source.path === target.path) continue
        contentHeight = 240 + targetIndex * 23
        await router.push(target.path)
        await settle()
        expect(mocks.resize).toHaveBeenLastCalledWith(target.meta.windowWidth, contentHeight)
        contentHeight = sourceHeight
        await router.push(source.path)
        await settle()
        expect(mocks.resize).toHaveBeenLastCalledWith(source.meta.windowWidth, sourceHeight)
        transitions += 1
      }
    }
    expect(transitions).toBe(132)
  })

  it('keeps the one-pixel idempotence guard across repeated content notifications', async () => {
    await install()
    const count = mocks.resize.mock.calls.length
    expect(count).toBeGreaterThan(0)
    contentHeight += 1
    for (let i = 0; i < 5; i += 1) notifyResize?.()
    await settle()
    expect(mocks.resize).toHaveBeenCalledTimes(count)
    expect(mocks.zoom).toHaveBeenCalledTimes(1)
  })

  it('resizes for genuine content changes', async () => {
    await install()
    contentHeight = 700
    notifyResize?.()
    await settle()
    expect(mocks.resize).toHaveBeenLastCalledWith(660, 700)
  })

  it('remeasures the new layout after applying a route width', async () => {
    mocks.resize.mockImplementationOnce(async () => {
      contentHeight = 410
      return { status: 'ok', data: null }
    })
    await install()
    await settle()
    expect(mocks.resize).toHaveBeenLastCalledWith(660, 410)
  })

  it('keeps a single resize and DPI subscription while navigating', async () => {
    const add = vi.spyOn(window, 'addEventListener')
    const router = await install()
    await router.push('/about')
    await settle()
    await router.push('/manage-account')
    await settle()
    expect(add.mock.calls.filter(([event]) => event === 'resize')).toHaveLength(1)
    expect(mocks.onScaleChanged).toHaveBeenCalledTimes(1)
    expect(mocks.onMoved).toHaveBeenCalledTimes(1)
  })

  it('disconnects observers and cancels pending work on fitsWindow:false', async () => {
    const router = await install()
    const count = mocks.resize.mock.calls.length
    contentHeight = 700
    notifyResize?.()
    await router.push('/browser-bar')
    window.dispatchEvent(new Event('resize'))
    await settle()
    expect(mocks.resize).toHaveBeenCalledTimes(count)
    expect(disconnect).toHaveBeenCalled()
  })

  it('restores the final route after an override closes during navigation', async () => {
    const router = await install()
    const release = mainWindowGeometry().override({ width: 640, height: 720 })
    await mainWindowGeometry().whenIdle()
    await router.push('/about')
    await router.push('/manage-account')
    contentHeight = 600
    release()
    await settle()
    expect(mocks.resize).toHaveBeenLastCalledWith(880, 600)
  })

  it('still fits an occluded window when animation frames never fire', async () => {
    vi.stubGlobal(
      'requestAnimationFrame',
      vi.fn(() => 1),
    )
    vi.stubGlobal('cancelAnimationFrame', vi.fn())
    await install()
    expect(mocks.resize).toHaveBeenLastCalledWith(660, 500)
  })

  it('waits for an app that mounts after the initial paint backstop', async () => {
    document.body.innerHTML = ''
    await install()
    expect(mocks.resize).not.toHaveBeenCalled()
    const root = document.createElement('main')
    root.dataset.windowRoot = ''
    Object.defineProperty(root, 'scrollHeight', { value: 500 })
    document.body.appendChild(root)
    await settle()
    await settle()
    expect(mocks.resize).toHaveBeenLastCalledWith(660, 500)
  })

  it('disposes a late native event subscription after the owner is stopped', async () => {
    let finish!: (stop: () => void) => void
    const stop = vi.fn()
    mocks.onMoved.mockReturnValueOnce(
      new Promise((resolve) => {
        finish = resolve
      }),
    )
    await install()
    dispose?.()
    finish(stop)
    await flushPromises()
    expect(stop).toHaveBeenCalledTimes(1)
  })

  it('preserves the screen CSS work area when accessibility scaling differs from native logical pixels', async () => {
    mocks.monitor.mockResolvedValue({ workArea: { size: { width: 1200, height: 900 } } })
    mocks.scaleFactor.mockResolvedValueOnce(1.5)
    vi.stubGlobal('devicePixelRatio', 2.25)
    // Native logical area is 800 by 600. At 150% text scaling, the
    // screen API reports approximately 533 by 400 CSS pixels instead.
    vi.stubGlobal('screen', { availWidth: 533, availHeight: 400 })
    await install()
    const first = mocks.resize.mock.calls[0]
    expect(first).toEqual([752, 570])
    expect(mocks.zoom.mock.calls[0]).toEqual([0.76])
    expect(mocks.monitor).not.toHaveBeenCalled()
  })

  it('retains the original work-area fallback when the screen API is unavailable', async () => {
    mocks.monitor.mockResolvedValue({ workArea: { size: { width: 400, height: 300 } } })
    vi.stubGlobal('screen', { availWidth: 0, availHeight: 0 })
    await install()
    expect(mocks.resize).toHaveBeenLastCalledWith(660, 500)
    expect(mocks.zoom).toHaveBeenLastCalledWith(1)
    expect(mocks.monitor).not.toHaveBeenCalled()
  })
})
