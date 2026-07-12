/**
 * Specs for the Windows Accessibility "Text size" compensation in
 * services/windowFit.ts — the `textScaleFactor` maths that keeps the
 * OS window sized to its text-scaled content (regression: text size
 * ≥ 130% clipped the app after #337 dropped #257's
 * `--force-device-scale-factor=1`).
 */

import { describe, it, expect, beforeEach } from 'vitest'
import {
  textScaleFactor,
  setWindowScaleFactor,
  setAppliedWebviewZoom,
  resetWindowScaleTrackingForTests,
} from '../../../src/services/windowFit'

describe('textScaleFactor', () => {
  beforeEach(() => {
    resetWindowScaleTrackingForTests()
  })

  it('returns 1 before the OS scale factor has been reported', () => {
    // Pre-IPC / jsdom: no basis to split devicePixelRatio, so pass
    // sizes through unchanged (the pre-fix behaviour).
    expect(textScaleFactor(1.3)).toBe(1)
  })

  it('returns 1 at text size 100% on a 100% display', () => {
    setWindowScaleFactor(1)
    expect(textScaleFactor(1)).toBe(1)
  })

  it('extracts the text scale on a 100% display', () => {
    // Windows text size 130%: devicePixelRatio = 1 × 1.3.
    setWindowScaleFactor(1)
    expect(textScaleFactor(1.3)).toBeCloseTo(1.3)
  })

  it('divides out the display scale at >100% scaling', () => {
    // 150% display scaling with text size 130%:
    // devicePixelRatio = 1.5 × 1.3 = 1.95, factor must be 1.3 (the
    // display part is already handled by Tauri's LogicalSize).
    setWindowScaleFactor(1.5)
    expect(textScaleFactor(1.95)).toBeCloseTo(1.3)
  })

  it('returns 1 at >100% display scaling with text size 100%', () => {
    // The #337 case that must NOT regress: window already matches
    // content, no extra multiplication allowed.
    setWindowScaleFactor(1.25)
    expect(textScaleFactor(1.25)).toBe(1)
  })

  it('divides out the applied webview zoom', () => {
    // fitWindow shrank oversized content to zoom 0.8 on a 100% display
    // with text size 130%: devicePixelRatio = 1 × 1.3 × 0.8 = 1.04.
    setWindowScaleFactor(1)
    setAppliedWebviewZoom(0.8)
    expect(textScaleFactor(1.04)).toBeCloseTo(1.3)
  })

  it('treats sub-1 ratios (persisted Ctrl-zoom-out) as 1', () => {
    setWindowScaleFactor(1)
    expect(textScaleFactor(0.8)).toBe(1)
  })

  it('treats ratios above 3 (junk readings) as 1', () => {
    // Windows text size tops out at 225%; anything above 3 is a
    // transient mid-resize reading, not a real text scale.
    setWindowScaleFactor(1)
    expect(textScaleFactor(3.5)).toBe(1)
  })

  it('ignores invalid scale factors and zooms', () => {
    setWindowScaleFactor(0)
    setWindowScaleFactor(Number.NaN)
    expect(textScaleFactor(1.3)).toBe(1) // still untracked

    setWindowScaleFactor(1)
    setAppliedWebviewZoom(0)
    setAppliedWebviewZoom(Number.NaN)
    expect(textScaleFactor(1.3)).toBeCloseTo(1.3) // zoom still 1
  })

  it('ignores invalid devicePixelRatio readings', () => {
    setWindowScaleFactor(1)
    expect(textScaleFactor(0)).toBe(1)
    expect(textScaleFactor(Number.NaN)).toBe(1)
  })
})
