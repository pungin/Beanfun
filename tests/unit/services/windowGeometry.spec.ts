import { describe, expect, it, vi } from 'vitest'
import { flushPromises } from '@vue/test-utils'
import {
  calculateWindowFit,
  WindowGeometryController,
  type WindowGeometryAdapter,
} from '../../../src/services/windowGeometry'

function deferred() {
  let resolve!: () => void
  const promise = new Promise<void>((done) => {
    resolve = done
  })
  return { promise, resolve }
}

function harness() {
  const adapter: WindowGeometryAdapter = {
    environment: vi.fn(async () => ({ width: 1000, height: 800, textScale: 1, scaleFactor: 1 })),
    setZoom: vi.fn(async () => {}),
    resize: vi.fn(async () => {}),
    onError: vi.fn(),
    onApplied: vi.fn(),
  }
  return { adapter, controller: new WindowGeometryController(adapter) }
}

describe('main-window geometry contract', () => {
  it('preserves the existing cap-before-text-scale order', () => {
    const fitted = calculateWindowFit(
      { width: 880, height: 680 },
      { width: 900, height: 700, textScale: 1.5, scaleFactor: 1 },
    )
    expect(fitted).toEqual({ width: 1283, height: 991, zoom: 855 / 880 })
  })

  it('preserves the existing size floors on a tiny work area', () => {
    const fitted = calculateWindowFit(
      { width: 420, height: 500 },
      { width: 280, height: 150, textScale: 1, scaleFactor: 1 },
    )
    expect(fitted).toEqual({ width: 320, height: 300, zoom: 0.6 })
  })

  it.each([
    { demand: [420, 210], area: [1920, 1040], textScale: 1, expected: [420, 210, 1] },
    { demand: [480, 800], area: [1280, 680], textScale: 1, expected: [388, 646, 0.8075] },
    { demand: [660, 900], area: [1280, 680], textScale: 1.3, expected: [616, 840, 646 / 900] },
    { demand: [880, 640], area: [768, 960], textScale: 1.5, expected: [1094, 795, 729 / 880] },
    { demand: [420, 1], area: [1920, 1040], textScale: 1, expected: [420, 160, 1] },
  ])(
    'matches the established page sizing for $demand at text scale $textScale',
    ({ demand, area, textScale, expected }) => {
      expect(
        calculateWindowFit(
          { width: demand[0], height: demand[1] },
          { width: area[0], height: area[1], textScale, scaleFactor: 1 },
        ),
      ).toEqual({ width: expected[0], height: expected[1], zoom: expected[2] })
    },
  )

  it('holds an announcement at exactly 640 by 720 logical pixels without changing zoom', async () => {
    const { adapter, controller } = harness()
    vi.mocked(adapter.environment).mockResolvedValue({
      width: 800,
      height: 600,
      textScale: 1.5,
      scaleFactor: 1,
    })
    controller.setRoute({ width: 660, height: 900 })
    await controller.whenIdle()
    const previousZoom = vi.mocked(adapter.setZoom).mock.calls.at(-1)?.[0]
    const zoomCalls = vi.mocked(adapter.setZoom).mock.calls.length
    const release = controller.override({ width: 640, height: 720 })
    await controller.whenIdle()
    expect(adapter.resize).toHaveBeenLastCalledWith(640, 720)
    expect(adapter.setZoom).toHaveBeenCalledTimes(zoomCalls)
    expect(previousZoom).toBe(570 / 900)
    controller.setRoute({ width: 420, height: 400 })
    release()
    await controller.whenIdle()
    expect(adapter.resize).toHaveBeenLastCalledWith(630, 600)
    expect(adapter.setZoom).toHaveBeenLastCalledWith(1)
  })

  it('does not introduce zoom when an announcement opens before the first route fit', async () => {
    const { adapter, controller } = harness()
    controller.override({ width: 640, height: 720 })
    await controller.whenIdle()
    expect(adapter.resize).toHaveBeenCalledExactlyOnceWith(640, 720)
    expect(adapter.setZoom).not.toHaveBeenCalled()
  })

  it('preserves the public helper rounding and text scaling without a new zoom cap', async () => {
    const { adapter, controller } = harness()
    vi.mocked(adapter.environment).mockResolvedValue({
      width: 800,
      height: 600,
      textScale: 1.3,
      scaleFactor: 1,
    })
    controller.setRoute({ width: 880.2, height: 640.2 }, 'text-scaled')
    await controller.whenIdle()
    expect(adapter.resize).toHaveBeenCalledExactlyOnceWith(1145, 833)
    expect(adapter.setZoom).not.toHaveBeenCalled()
  })

  it('keeps text enlarged when the current work area can contain it', () => {
    expect(
      calculateWindowFit(
        { width: 420, height: 500 },
        { width: 1920, height: 1040, textScale: 1.5, scaleFactor: 1 },
      ),
    ).toEqual({ width: 630, height: 750, zoom: 1 })
  })

  it('serialises zoom before resize and coalesces obsolete route demands', async () => {
    const { adapter, controller } = harness()
    const pending = deferred()
    vi.mocked(adapter.setZoom).mockReturnValueOnce(pending.promise)
    controller.setRoute({ width: 420, height: 400 })
    await flushPromises()
    controller.setRoute({ width: 660, height: 500 })
    controller.setRoute({ width: 880, height: 600 })
    expect(adapter.resize).not.toHaveBeenCalled()
    pending.resolve()
    await controller.whenIdle()
    expect(adapter.resize).toHaveBeenCalledTimes(1)
    expect(adapter.resize).toHaveBeenLastCalledWith(880, 600)
  })

  it('does not remember a failed resize and retries only on a subsequent request', async () => {
    const { adapter, controller } = harness()
    vi.mocked(adapter.resize).mockRejectedValueOnce(new Error('IPC failed'))
    controller.setRoute({ width: 660, height: 500 })
    await controller.whenIdle()
    expect(adapter.resize).toHaveBeenCalledTimes(1)
    expect(adapter.onApplied).not.toHaveBeenCalled()
    expect(adapter.onError).toHaveBeenCalledTimes(1)
    controller.setRoute({ width: 660, height: 500 })
    await controller.whenIdle()
    expect(adapter.resize).toHaveBeenCalledTimes(2)
    expect(adapter.onApplied).toHaveBeenCalledTimes(1)
  })

  it('never resizes with unconfirmed zoom after a zoom IPC failure', async () => {
    const { adapter, controller } = harness()
    vi.mocked(adapter.setZoom).mockRejectedValueOnce(new Error('Zoom unavailable'))
    controller.setRoute({ width: 660, height: 500 })
    await controller.whenIdle()
    expect(adapter.resize).not.toHaveBeenCalled()
    controller.refresh()
    await controller.whenIdle()
    expect(adapter.setZoom).toHaveBeenCalledTimes(2)
    expect(adapter.resize).toHaveBeenCalledExactlyOnceWith(660, 500)
  })

  it('waits for valid monitor metrics rather than inventing a work area on failure', async () => {
    const { adapter, controller } = harness()
    vi.mocked(adapter.environment).mockRejectedValueOnce(new Error('Monitor unavailable'))
    controller.setRoute({ width: 660, height: 500 })
    await controller.whenIdle()
    expect(adapter.setZoom).not.toHaveBeenCalled()
    expect(adapter.resize).not.toHaveBeenCalled()
    controller.refresh()
    await controller.whenIdle()
    expect(adapter.resize).toHaveBeenCalledExactlyOnceWith(660, 500)
  })

  it('retains a request arriving between the final apply and promise settlement', async () => {
    const { adapter, controller } = harness()
    vi.mocked(adapter.onApplied!).mockImplementationOnce(() => {
      queueMicrotask(() => controller.setRoute({ width: 880, height: 600 }))
    })
    controller.setRoute({ width: 420, height: 400 })
    await controller.whenIdle()
    expect(adapter.resize).toHaveBeenLastCalledWith(880, 600)
  })

  it('holds nested overrides and restores the latest route rather than a saved old size', async () => {
    const { adapter, controller } = harness()
    controller.setRoute({ width: 420, height: 400 })
    await controller.whenIdle()
    const releaseAnnouncement = controller.override({ width: 640, height: 720 })
    await controller.whenIdle()
    const releaseNested = controller.override({ width: 720, height: 600 })
    await controller.whenIdle()
    controller.setRoute({ width: 880, height: 640 })
    releaseAnnouncement()
    releaseAnnouncement()
    await controller.whenIdle()
    expect(adapter.resize).toHaveBeenLastCalledWith(720, 600)
    releaseNested()
    await controller.whenIdle()
    expect(adapter.resize).toHaveBeenLastCalledWith(880, 640)
  })

  it('retains the one-pixel tolerance without suppressing genuine changes', async () => {
    const { adapter, controller } = harness()
    controller.setRoute({ width: 660, height: 500 })
    await controller.whenIdle()
    controller.setRoute({ width: 660, height: 501 })
    await controller.whenIdle()
    expect(adapter.resize).toHaveBeenCalledTimes(1)
    controller.setRoute({ width: 660, height: 550 })
    await controller.whenIdle()
    expect(adapter.resize).toHaveBeenCalledTimes(2)
    expect(adapter.setZoom).toHaveBeenCalledTimes(1)
  })

  it('cancels a pending measurement when a non-fitting route takes over', async () => {
    const { adapter, controller } = harness()
    const pending = deferred()
    vi.mocked(adapter.setZoom).mockReturnValueOnce(pending.promise)
    controller.setRoute({ width: 660, height: 500 })
    await flushPromises()
    controller.setEnabled(false)
    pending.resolve()
    await controller.whenIdle()
    expect(adapter.resize).not.toHaveBeenCalled()
  })

  it('supersedes an in-flight resize without starting overlapping native requests', async () => {
    const { adapter, controller } = harness()
    const pending = deferred()
    vi.mocked(adapter.resize).mockReturnValueOnce(pending.promise)
    controller.setRoute({ width: 420, height: 400 })
    await flushPromises()
    controller.setRoute({ width: 660, height: 500 })
    controller.setRoute({ width: 880, height: 600 })
    await flushPromises()
    expect(adapter.resize).toHaveBeenCalledTimes(1)
    pending.resolve()
    await controller.whenIdle()
    expect(adapter.resize).toHaveBeenCalledTimes(2)
    expect(adapter.resize).toHaveBeenLastCalledWith(880, 600)
  })
})

describe('content measurement stability', () => {
  const smallScreen = { width: 1536, height: 832, textScale: 1, scaleFactor: 1 }
  const demand = { width: 660, height: 800 }

  function contentHarness() {
    const result = harness()
    vi.mocked(result.adapter.environment).mockResolvedValue(smallScreen)
    return result
  }

  it.each([
    { name: 'smaller', area: smallScreen },
    { name: 'larger', area: { ...smallScreen, width: 2560, height: 1400 } },
  ])('stops both zoom and resize feedback on the $name screen', async ({ area }) => {
    const { adapter, controller } = contentHarness()
    vi.mocked(adapter.environment).mockResolvedValue(area)
    for (let pass = 0; pass < 10; pass += 1) {
      controller.setRoute({ ...demand, height: demand.height + (pass % 2) })
      await controller.whenIdle()
    }
    expect(adapter.setZoom).toHaveBeenCalledTimes(1)
    expect(adapter.resize).toHaveBeenCalledTimes(1)
    expect(adapter.onApplied).toHaveBeenCalledTimes(1)
  })

  it('allows gradual growth beyond the last applied measurement', async () => {
    const { adapter, controller } = contentHarness()
    controller.setRoute(demand)
    await controller.whenIdle()
    controller.setRoute({ ...demand, height: 801 })
    await controller.whenIdle()
    expect(adapter.setZoom).toHaveBeenCalledTimes(1)
    controller.setRoute({ ...demand, height: 802 })
    await controller.whenIdle()
    expect(adapter.setZoom).toHaveBeenCalledTimes(2)
    expect(adapter.setZoom).toHaveBeenLastCalledWith(790 / 802)
    expect(adapter.resize).toHaveBeenLastCalledWith(650, 790)
  })

  it('tolerates floating-point round-off in the recovered text scale', async () => {
    const { adapter, controller } = contentHarness()
    controller.setRoute(demand)
    await controller.whenIdle()
    vi.mocked(adapter.environment).mockResolvedValue({
      ...smallScreen,
      textScale: 1 + Number.EPSILON,
    })
    controller.setRoute({ ...demand, height: 801 })
    await controller.whenIdle()
    expect(adapter.setZoom).toHaveBeenCalledTimes(1)
    expect(adapter.resize).toHaveBeenCalledTimes(1)
  })

  it.each([
    { name: 'work-area width', area: { ...smallScreen, width: 1200 } },
    { name: 'work-area height', area: { ...smallScreen, height: 1400 } },
    { name: 'text scale', area: { ...smallScreen, textScale: 1.5 } },
    { name: 'native DPI', area: { ...smallScreen, scaleFactor: 1.5 } },
  ])('recalculates after a change in $name', async ({ area }) => {
    const { adapter, controller } = contentHarness()
    controller.setRoute(demand)
    await controller.whenIdle()
    vi.mocked(adapter.environment).mockResolvedValue(area)
    const nextDemand = { ...demand, height: 801 }
    controller.setRoute(nextDemand)
    await controller.whenIdle()
    expect(adapter.setZoom).toHaveBeenCalledTimes(2)
    expect(adapter.setZoom).toHaveBeenLastCalledWith(calculateWindowFit(nextDemand, area).zoom)
  })

  it.each(['navigation', 'disabled fitting'] as const)(
    'discards the previous measurement after %s',
    async (change) => {
      const { adapter, controller } = contentHarness()
      controller.setRoute(demand)
      await controller.whenIdle()
      if (change === 'navigation') controller.setRoute(null)
      else {
        controller.setEnabled(false)
        controller.setEnabled(true)
      }
      controller.setRoute({ ...demand, height: 801 })
      await controller.whenIdle()
      expect(adapter.setZoom).toHaveBeenCalledTimes(2)
      expect(adapter.setZoom).toHaveBeenLastCalledWith(790 / 801)
    },
  )

  it('restores content fitting after a temporary override at the same route', async () => {
    const { adapter, controller } = contentHarness()
    controller.setRoute(demand)
    await controller.whenIdle()
    const release = controller.override({ width: 640, height: 720 })
    await controller.whenIdle()
    controller.setRoute({ ...demand, height: 801 })
    await controller.whenIdle()
    release()
    await controller.whenIdle()
    expect(adapter.resize).toHaveBeenLastCalledWith(651, 790)
    expect(adapter.setZoom).toHaveBeenLastCalledWith(790 / 801)
  })

  it('restores content fitting after a text-scaled size request', async () => {
    const { adapter, controller } = contentHarness()
    controller.setRoute(demand)
    await controller.whenIdle()
    controller.setRoute({ width: 640, height: 720 }, 'text-scaled')
    await controller.whenIdle()
    controller.setRoute(demand)
    await controller.whenIdle()
    expect(adapter.resize).toHaveBeenLastCalledWith(652, 790)
  })

  it('does not reuse the old measurement after zoom succeeds but resizing fails', async () => {
    const { adapter, controller } = contentHarness()
    controller.setRoute(demand)
    await controller.whenIdle()
    vi.mocked(adapter.resize).mockRejectedValueOnce(new Error('IPC failed'))
    controller.setRoute({ ...demand, height: 810 })
    await controller.whenIdle()
    expect(adapter.onError).toHaveBeenCalledTimes(1)
    controller.setRoute(demand)
    await controller.whenIdle()
    expect(adapter.setZoom).toHaveBeenLastCalledWith(790 / 800)
    expect(adapter.onApplied).toHaveBeenCalledTimes(2)
  })

  it('restores the previous demand when it supersedes an in-flight zoom', async () => {
    const { adapter, controller } = contentHarness()
    controller.setRoute(demand)
    await controller.whenIdle()
    const pending = deferred()
    vi.mocked(adapter.setZoom).mockReturnValueOnce(pending.promise)
    controller.setRoute({ ...demand, height: 810 })
    await flushPromises()
    controller.setRoute(demand)
    pending.resolve()
    await controller.whenIdle()
    expect(adapter.setZoom).toHaveBeenLastCalledWith(790 / 800)
    expect(adapter.onApplied).toHaveBeenCalledTimes(2)
  })

  it('does not restore an obsolete route measurement after an in-flight resize', async () => {
    const { adapter, controller } = contentHarness()
    const pending = deferred()
    vi.mocked(adapter.resize).mockReturnValueOnce(pending.promise)
    controller.setRoute(demand)
    await flushPromises()
    controller.setRoute(null)
    controller.setRoute({ ...demand, height: 801 })
    pending.resolve()
    await controller.whenIdle()
    expect(adapter.setZoom).toHaveBeenLastCalledWith(790 / 801)
    expect(adapter.onApplied).toHaveBeenCalledTimes(1)
  })
})
