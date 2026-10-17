import { describe, expect, it, vi } from 'vitest'
import { flushPromises, shallowMount } from '@vue/test-utils'
import CoreCalculator from '../../../src/windows/CoreCalculator.vue'
import { startCoreCalculation } from '../../../src/services/coreCalculatorTask'

vi.mock('vue-i18n', () => ({ useI18n: () => ({ t: (key: string) => key }) }))
vi.mock('../../../src/services/coreCalculatorTask', () => ({ startCoreCalculation: vi.fn() }))

describe('core calculator lifetime', () => {
  it('cancels a pending worker when hidden and ignores its obsolete result', async () => {
    let resolve!: (text: string) => void
    const cancel = vi.fn()
    vi.mocked(startCoreCalculation).mockReturnValue({
      result: new Promise<string>((done) => {
        resolve = done
      }),
      cancel,
    })
    const wrapper = shallowMount(CoreCalculator, {
      props: { visible: true },
      global: { renderStubDefaultSlot: true },
    })
    await wrapper.get('[data-test="core-calculator-run"]').trigger('click')
    expect(startCoreCalculation).toHaveBeenCalled()
    await wrapper.setProps({ visible: false })
    expect(cancel).toHaveBeenCalledOnce()
    resolve('Obsolete result')
    await flushPromises()
    expect(wrapper.find('[data-test="core-calculator-result"]').attributes('modelvalue')).not.toBe(
      'Obsolete result',
    )
    wrapper.unmount()
  })
})
