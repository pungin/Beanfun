import { afterEach, describe, expect, it, vi } from 'vitest'
import { ref } from 'vue'
import { startCoreCalculation } from '../../../src/services/coreCalculatorTask'

const labels = { mainLabel: 'Main', notFound: 'None', groupTemplate: 'Group {index}' }

function mockWorker() {
  const worker = {
    postMessage: vi.fn(),
    terminate: vi.fn(),
    onmessage: null as ((event: MessageEvent<string>) => void) | null,
    onerror: null as (() => void) | null,
    onmessageerror: null as (() => void) | null,
  }
  vi.stubGlobal(
    'Worker',
    class {
      constructor() {
        return worker
      }
    },
  )
  return worker
}

afterEach(() => vi.unstubAllGlobals())

describe('core calculation worker', () => {
  it('sends a cloneable snapshot and terminates after returning the result', async () => {
    const worker = mockWorker()
    const items = ref([{ skill1: 'A', skill2: 'B', skill3: 'C' }])
    const task = startCoreCalculation(items.value, ['A'], labels)
    const payload = worker.postMessage.mock.calls[0][0]
    expect(() => structuredClone(payload)).not.toThrow()
    items.value[0].skill1 = 'Changed'
    expect(payload.items[0].skill1).toBe('A')
    worker.onmessage!({ data: 'Group 1' } as MessageEvent<string>)
    await expect(task.result).resolves.toBe('Group 1')
    expect(worker.terminate).toHaveBeenCalledOnce()
  })

  it('cancels computation instead of leaving a pending promise', async () => {
    const worker = mockWorker()
    const task = startCoreCalculation([], [], labels)
    const assertion = expect(task.result).rejects.toMatchObject({ name: 'AbortError' })
    task.cancel()
    await assertion
    expect(worker.terminate).toHaveBeenCalledOnce()
  })

  it.each(['onerror', 'onmessageerror'] as const)('cleans up on %s', async (event) => {
    const worker = mockWorker()
    const task = startCoreCalculation([], [], labels)
    const assertion = expect(task.result).rejects.toBeInstanceOf(Error)
    worker[event]!()
    await assertion
    expect(worker.terminate).toHaveBeenCalledOnce()
  })
})
