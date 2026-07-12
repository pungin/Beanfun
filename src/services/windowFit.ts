/**
 * Tiny shared toggle for the router's content-fit window resizer.
 *
 * `router/index.ts` installs a `ResizeObserver` that snaps the OS window
 * to its `[data-window-root]` content height on every viewport change.
 * That's exactly what a full-window overlay ({@link AnnouncementModal})
 * fights when it grows the window: growing the window changes `100vh`,
 * which fires the observer, which shrinks the window straight back.
 *
 * The overlay flips this flag on while it wants a fixed larger size, and
 * off (restoring the prior size itself) on close. Kept in its own
 * dependency-free module so a component can import the setter without
 * pulling the whole router (and its page/tauri imports) into a unit test.
 */
let suspended = false

/** `true` while the content-fit resizer should be a no-op. */
export function isWindowFitSuspended(): boolean {
  return suspended
}

/** Suspend / resume the router's content-fit window resizer. */
export function setWindowFitSuspended(value: boolean): void {
  suspended = value
}

/*
 * ---------------------------------------------------------------------
 * Windows Accessibility "Text size" compensation (issue: text size
 * ≥ 130% clips the app; regressed when #337 removed #257's
 * `--force-device-scale-factor=1`).
 *
 * WebView2's rasterization scale is `monitor DPI scale × system text
 * scale`, and the webview surfaces the combined value as
 * `devicePixelRatio`. Tauri's `LogicalSize`, however, multiplies by the
 * *window's* scale factor only (monitor DPI). So at text size 130% one
 * CSS px paints 1.3× larger than the logical px the window was sized
 * in, and the content overflows the window by exactly that ratio.
 *
 * The fix: derive the text-scale ratio at runtime —
 *
 *     textScale = devicePixelRatio / (windowScaleFactor × webviewZoom)
 *
 * — and multiply it into every `LogicalSize` the resizer emits. Big
 * text stays big (accessibility respected, unlike #257's approach of
 * forcing the whole webview to 100%), the window simply grows to fit,
 * and at text size 100% the ratio is 1 so nothing changes.
 *
 * `webviewZoom` must be divided out because the router's `fitWindow`
 * applies `setZoom(<1)` to shrink oversized content, and browser zoom
 * also multiplies `devicePixelRatio`.
 *
 * State lives here (not in router/index.ts) so it is unit-testable
 * without pulling the router's Tauri/page imports into jsdom.
 */

let windowScaleFactor: number | null = null
let webviewZoom = 1

/**
 * Record the OS window scale factor (from `Window.scaleFactor()` /
 * `onScaleChanged`). Non-finite or non-positive values are ignored.
 */
export function setWindowScaleFactor(factor: number): void {
  if (Number.isFinite(factor) && factor > 0) windowScaleFactor = factor
}

/**
 * Record the zoom the resizer last applied via `Webview.setZoom()` so
 * it can be divided back out of `devicePixelRatio`.
 */
export function setAppliedWebviewZoom(zoom: number): void {
  if (Number.isFinite(zoom) && zoom > 0) webviewZoom = zoom
}

/**
 * Multiplier converting CSS px to Tauri logical px — the system text
 * scale (1.0 at 100%, 1.3 at 130%, …).
 *
 * Returns 1 until {@link setWindowScaleFactor} has reported a real OS
 * scale (pre-IPC, jsdom), and clamps to [1, 3]: Windows offers
 * 100–225% text size, so values outside that band are transient
 * mid-resize readings or a user's persisted Ctrl±zoom, not a text
 * scale — treated as 1 rather than propagated into the window size.
 * (Persisted zoom-in > 1 within the band intentionally passes through:
 * growing the window to fit zoomed content is the desired outcome
 * there too.)
 */
export function textScaleFactor(devicePixelRatio: number): number {
  if (windowScaleFactor === null || !Number.isFinite(devicePixelRatio) || devicePixelRatio <= 0) {
    return 1
  }
  const factor = devicePixelRatio / (windowScaleFactor * webviewZoom)
  return factor >= 1 && factor <= 3 ? factor : 1
}

/** Test-only: return the module to its pristine pre-tracking state. */
export function resetWindowScaleTrackingForTests(): void {
  windowScaleFactor = null
  webviewZoom = 1
}
