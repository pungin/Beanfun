/** CSS content dimensions before accessibility text scaling and webview zoom. */
export interface WindowDemand {
  width: number
  height: number
}

/** Available work area in screen CSS pixels; distinct from native logical pixels. */
export interface WindowEnvironment {
  width: number
  height: number
  textScale: number
  /** Native monitor DPI scale, independent of text scaling and webview zoom. */
  scaleFactor: number
}

export interface WindowFit extends WindowDemand {
  zoom: number
}

export interface WindowGeometryAdapter {
  environment(): Promise<WindowEnvironment>
  setZoom(zoom: number): Promise<void>
  resize(width: number, height: number): Promise<void>
  onApplied?(fit: WindowFit): void
  onError?(error: unknown): void
}

/** Preserve the established content-fit policy, including its text-scale order. */
export function calculateWindowFit(demand: WindowDemand, area: WindowEnvironment): WindowFit {
  const values = [demand.width, demand.height, area.width, area.height, area.textScale]
  if (values.some((value) => !Number.isFinite(value) || value <= 0)) {
    throw new Error('Window dimensions and scale must be positive finite numbers')
  }
  const capWidth = Math.max(320, Math.floor(area.width * 0.95))
  const capHeight = Math.max(300, Math.floor(area.height * 0.95))
  const zoom = Math.min(1, capWidth / demand.width, capHeight / demand.height)
  return {
    width: Math.max(320, Math.round(demand.width * zoom * area.textScale)),
    height: Math.max(160, Math.round(demand.height * zoom * area.textScale)),
    zoom,
  }
}

interface SizeRequest {
  demand: WindowDemand
  sizing: 'content' | 'text-scaled' | 'logical'
}

interface ContentMeasurement {
  demand: WindowDemand
  environment: WindowEnvironment
}

/**
 * Own every main-window size request. Native placement remains in Rust.
 *
 * Route measurements continue beneath temporary overlays, so releasing an
 * override restores the latest route rather than an obsolete physical size.
 * IPCs are serial: a newer demand supersedes work that has not yet been sent.
 */
export class WindowGeometryController {
  private route: SizeRequest | null = null
  private readonly overrides = new Map<symbol, WindowDemand>()
  private enabled = true
  private revision = 0
  private processedRevision = 0
  private running: Promise<void> | null = null
  private applied: WindowFit | null = null
  private appliedZoom: number | null = null
  private appliedContent: ContentMeasurement | null = null

  constructor(private readonly adapter: WindowGeometryAdapter) {}

  setRoute(demand: WindowDemand | null, sizing: 'content' | 'text-scaled' = 'content'): void {
    if (!demand || sizing !== 'content') this.appliedContent = null
    this.route = demand ? { demand, sizing } : null
    this.refresh()
  }

  setEnabled(enabled: boolean): void {
    this.enabled = enabled
    if (!enabled) {
      this.route = null
      this.appliedContent = null
    }
    this.refresh()
  }

  /** Fixed logical size; release is independent and idempotent, even out of order. */
  override(demand: WindowDemand): () => void {
    this.appliedContent = null
    const token = Symbol('window-size-override')
    this.overrides.set(token, demand)
    this.refresh()
    return () => {
      if (this.overrides.delete(token)) this.refresh()
    }
  }

  /** Re-read monitor metrics after a move, DPI change or an explicit retry. */
  refresh(): void {
    this.revision += 1
    if (this.running) return
    this.running = Promise.resolve()
      .then(() => this.applyLatest())
      .finally(() => {
        this.running = null
        // A demand can arrive after the final await but before this settlement
        // microtask. Keep it queued rather than leaving it until another event.
        if (this.processedRevision !== this.revision) this.refresh()
      })
  }

  async whenIdle(): Promise<void> {
    while (this.running) await this.running
  }

  private desired(): SizeRequest | null {
    if (!this.enabled) return null
    const overrides = [...this.overrides.values()]
    const override = overrides[overrides.length - 1]
    return override ? { demand: override, sizing: 'logical' } : this.route
  }

  private isMeasurementNoise({ demand, environment }: ContentMeasurement): boolean {
    const previous = this.appliedContent
    return (
      previous !== null &&
      demand.width === previous.demand.width &&
      Math.abs(demand.height - previous.demand.height) <= 1 &&
      environment.width === previous.environment.width &&
      environment.height === previous.environment.height &&
      environment.scaleFactor === previous.environment.scaleFactor &&
      // Removing webview zoom from devicePixelRatio can introduce round-off.
      Math.abs(environment.textScale - previous.environment.textScale) < 0.000001
    )
  }

  private async applyLatest(): Promise<void> {
    while (this.processedRevision !== this.revision) {
      const revision = this.revision
      this.processedRevision = revision
      const request = this.desired()
      if (!request) continue
      try {
        const { demand, sizing } = request
        let fit: WindowFit
        let content: ContentMeasurement | null = null
        if (sizing === 'logical') {
          // The announcement has always requested a fixed logical size while
          // retaining the route's existing zoom and accessibility text scale.
          fit = { ...demand, zoom: this.appliedZoom ?? 1 }
        } else {
          const environment = await this.adapter.environment()
          if (sizing === 'content')
            content = { demand: { ...demand }, environment: { ...environment } }
          fit =
            sizing === 'content'
              ? calculateWindowFit(demand, environment)
              : {
                  width: Math.ceil(demand.width * environment.textScale),
                  height: Math.ceil(demand.height * environment.textScale),
                  zoom: this.appliedZoom ?? 1,
                }
        }
        if (revision !== this.revision) continue
        // A one-pixel measurement can change zoom even when rounded native
        // dimensions stay the same. Suppress both against a fixed anchor;
        // ignored samples must not hide gradual growth by moving that anchor.
        if (content && this.isMeasurementNoise(content)) continue
        const zoomChanged =
          sizing === 'content' &&
          (this.appliedZoom === null || Math.abs(this.appliedZoom - fit.zoom) > 0.001)
        const sizeChanged =
          this.applied === null ||
          Math.abs(this.applied.width - fit.width) > 1 ||
          Math.abs(this.applied.height - fit.height) > 1
        if (!zoomChanged && !sizeChanged) continue
        // An interrupted or partially failed apply must not reuse the old
        // measurement after the native zoom or size may already have changed.
        this.appliedContent = null
        if (zoomChanged) {
          await this.adapter.setZoom(fit.zoom)
          this.appliedZoom = fit.zoom
        }
        if (revision !== this.revision) continue
        if (sizeChanged) await this.adapter.resize(fit.width, fit.height)
        // A failed command never becomes the remembered successful state.
        this.applied = fit
        if (revision === this.revision) {
          this.appliedContent = content
          this.adapter.onApplied?.(fit)
        }
      } catch (error) {
        // Retry only when another request arrives; never spin on an IPC failure.
        this.adapter.onError?.(error)
      }
    }
  }
}
