import { getCurrentWindow } from '@tauri-apps/api/window'
import { getCurrentWebview } from '@tauri-apps/api/webview'
import { commands } from '../types/bindings'
import { wrapCommand } from './invoke'
import { WindowGeometryController, type WindowFit } from './windowGeometry'
import {
  resetWindowScaleTrackingForTests,
  setAppliedWebviewZoom,
  setWindowScaleFactor,
  textScaleFactor,
} from './windowFit'

const appliedListeners = new Set<(fit: WindowFit) => void>()
let controller: WindowGeometryController | null = null

/** The main webview's sole adapter to native resizing and placement. */
export function mainWindowGeometry(): WindowGeometryController {
  controller ??= new WindowGeometryController({
    async environment() {
      const scale = await getCurrentWindow().scaleFactor()
      if (!Number.isFinite(scale) || scale <= 0) {
        throw new Error('Cannot determine the current window scale factor')
      }
      setWindowScaleFactor(scale)
      // Preserve the original screen CSS units, including accessibility text
      // scaling. Native placement independently reads the physical work area.
      const screen = typeof window !== 'undefined' ? window.screen : undefined
      return {
        width: screen && screen.availWidth > 0 ? screen.availWidth : 1280,
        height: screen && screen.availHeight > 0 ? screen.availHeight : 900,
        textScale: textScaleFactor(window.devicePixelRatio || 1),
      }
    },
    async setZoom(zoom) {
      await getCurrentWebview().setZoom(zoom)
      setAppliedWebviewZoom(zoom)
    },
    async resize(width, height) {
      // Placement and the main-webview trust boundary belong to the native
      // manager. A failed background fit must not create a stream of toasts.
      await wrapCommand(commands.resizeMainWindow(width, height), { silent: true })
    },
    onApplied(fit) {
      for (const listener of appliedListeners) listener(fit)
    },
    onError(error) {
      console.warn('[window-geometry] Could not apply window dimensions', error)
    },
  })
  return controller
}

/** Width changes can cross CSS breakpoints; measure again after native application. */
export function onMainWindowFitApplied(listener: (fit: WindowFit) => void): () => void {
  appliedListeners.add(listener)
  return () => {
    appliedListeners.delete(listener)
  }
}

/** Test-only: isolate successive geometry sessions without involving native state. */
export function resetMainWindowGeometryForTests(): void {
  controller?.setEnabled(false)
  controller = null
  appliedListeners.clear()
  resetWindowScaleTrackingForTests()
}
