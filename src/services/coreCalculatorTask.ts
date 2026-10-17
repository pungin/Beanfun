import type { CoreItem } from './coreCalculator'

export interface CoreCalculationRequest {
  items: CoreItem[]
  skills: string[]
  labels: { mainLabel: string; notFound: string; groupTemplate: string }
}

export interface CoreCalculationTask {
  result: Promise<string>
  cancel: () => void
}

/** Snapshot reactive input before crossing the worker boundary. */
export function startCoreCalculation(
  items: readonly CoreItem[],
  skills: readonly string[],
  labels: CoreCalculationRequest['labels'],
): CoreCalculationTask {
  const worker = new Worker(new URL('./coreCalculator.worker.ts', import.meta.url), {
    type: 'module',
  })
  let cancel = (): void => {}
  const result = new Promise<string>((resolve, reject) => {
    const finish = (error?: Error, value?: string): void => {
      worker.terminate()
      if (error) reject(error)
      else resolve(value ?? '')
    }
    cancel = () => finish(new DOMException('Calculation cancelled', 'AbortError'))
    worker.onmessage = (event: MessageEvent<string>) => finish(undefined, event.data)
    worker.onerror = () => finish(new Error('Core calculation failed'))
    worker.onmessageerror = () => finish(new Error('Core calculation response could not be read'))
    const request: CoreCalculationRequest = {
      items: items.map(({ skill1, skill2, skill3 }) => ({ skill1, skill2, skill3 })),
      skills: [...skills],
      labels,
    }
    try {
      worker.postMessage(request)
    } catch (error) {
      finish(error instanceof Error ? error : new Error(String(error)))
    }
  })
  return { result, cancel }
}
