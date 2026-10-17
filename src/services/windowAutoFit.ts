import type { Router } from 'vue-router'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { mainWindowGeometry, onMainWindowFitApplied } from './mainWindowGeometry'
import { setWindowScaleFactor } from './windowFit'

/** Paint-aligned measurements must also progress in an occluded WebView2 window. */
function paintScheduler() {
  let frame: number | null = null
  let timer: ReturnType<typeof setTimeout> | null = null
  function cancel(): void {
    if (frame !== null) window.cancelAnimationFrame(frame)
    if (timer !== null) clearTimeout(timer)
    frame = null
    timer = null
  }
  return {
    cancel,
    schedule(callback: () => void): void {
      cancel()
      const run = () => {
        cancel()
        callback()
      }
      if (typeof window.requestAnimationFrame !== 'function') {
        run()
        return
      }
      frame = window.requestAnimationFrame(() => {
        frame = window.requestAnimationFrame(run)
      })
      timer = setTimeout(run, 150)
    },
  }
}

let disposePrevious: (() => void) | null = null

/**
 * Measure page content; the shared coordinator alone chooses and applies size.
 * Navigation replaces observers immediately, before the new DOM is measured.
 */
export function installWindowAutoFit(router: Router): () => void {
  disposePrevious?.()
  const appWindow = getCurrentWindow()
  // The browser toolbar is another SPA instance inside a child webview.
  if (appWindow.label && appWindow.label !== 'main') return () => {}
  const geometry = mainWindowGeometry()
  const fitScheduler = paintScheduler()
  const attachScheduler = paintScheduler()
  let disposed = false
  let fitting = false
  let attached = false
  let currentWidth = 560
  let observer: ResizeObserver | null = null
  let pendingRoot: MutationObserver | null = null
  const unlisten: (() => void)[] = []

  function measure(): void {
    if (disposed || !fitting || !attached) return
    const root = document.querySelector<HTMLElement>('[data-window-root]')
    if (!root) return
    const scroll = root.querySelector<HTMLElement>('.account-list__scroll')
    const previousHeight = root.style.height
    const previousOverflow = scroll?.style.overflow ?? ''
    // Restore synchronously: an async IPC must never leave a page unclipped.
    let height: number
    try {
      if (scroll) scroll.style.overflow = 'visible'
      root.style.height = 'auto'
      height = Math.max(1, root.scrollHeight)
    } finally {
      root.style.height = previousHeight || '100vh'
      if (scroll) scroll.style.overflow = previousOverflow
    }
    geometry.setRoute({ width: currentWidth, height })
  }

  function scheduleFit(): void {
    if (!disposed && fitting) fitScheduler.schedule(measure)
  }

  function attach(): void {
    if (disposed || !fitting) return
    const root = document.querySelector<HTMLElement>('[data-window-root]')
    if (!root) {
      // Native startup checks can delay app.mount beyond the paint backstop.
      // Wait for the real page instead of polling or measuring an empty shell.
      pendingRoot ??= new MutationObserver(() => attachScheduler.schedule(attach))
      pendingRoot.observe(document.body, { childList: true, subtree: true })
      return
    }
    pendingRoot?.disconnect()
    pendingRoot = null
    attached = true
    observer?.disconnect()
    observer = new ResizeObserver(scheduleFit)
    observer.observe(root)
    const content = root.querySelector('[data-window-content]')
    if (content) observer.observe(content)
    // A rem sentinel catches accessibility font changes independently of height.
    let sentinel = document.getElementById('__fit-sentinel')
    if (!sentinel) {
      sentinel = document.createElement('div')
      sentinel.id = '__fit-sentinel'
      sentinel.style.cssText =
        'position:fixed;top:-9999px;left:-9999px;width:1rem;height:1rem;pointer-events:none;'
      document.body.appendChild(sentinel)
    }
    observer.observe(sentinel)
    measure()
  }

  // There is one subscription per main-window session, never one per route.
  window.addEventListener('resize', scheduleFit, { passive: true })
  unlisten.push(() => window.removeEventListener('resize', scheduleFit))
  unlisten.push(onMainWindowFitApplied(scheduleFit))

  function track(subscription: Promise<() => void>): void {
    void subscription
      .then((stop) => {
        if (disposed) stop()
        else unlisten.push(stop)
      })
      .catch((error: unknown) => {
        console.warn('[window-geometry] Could not observe window changes', error)
      })
  }
  if (typeof appWindow.onScaleChanged === 'function') {
    track(
      appWindow.onScaleChanged(({ payload }) => {
        setWindowScaleFactor(payload.scaleFactor)
        scheduleFit()
      }),
    )
  }
  if (typeof appWindow.onMoved === 'function') track(appWindow.onMoved(scheduleFit))

  unlisten.push(
    router.afterEach((to, _from, failure) => {
      if (failure) return
      observer?.disconnect()
      observer = null
      pendingRoot?.disconnect()
      pendingRoot = null
      attached = false
      fitScheduler.cancel()
      attachScheduler.cancel()
      geometry.setRoute(null)
      fitting = to.meta.fitsWindow !== false
      geometry.setEnabled(fitting)
      if (!fitting) return
      currentWidth = (to.meta.windowWidth as number | undefined) ?? 560
      attachScheduler.schedule(attach)
    }),
  )

  const dispose = (): void => {
    if (disposed) return
    disposed = true
    fitting = false
    attached = false
    observer?.disconnect()
    pendingRoot?.disconnect()
    fitScheduler.cancel()
    attachScheduler.cancel()
    geometry.setEnabled(false)
    for (const stop of unlisten) stop()
    if (disposePrevious === dispose) disposePrevious = null
  }
  disposePrevious = dispose
  return dispose
}
