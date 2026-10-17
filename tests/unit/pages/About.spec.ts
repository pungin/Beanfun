import { afterEach, describe, expect, it, vi } from 'vitest'
import { flushPromises, shallowMount } from '@vue/test-utils'
import { ElMessage } from 'element-plus'
import About from '../../../src/pages/About.vue'
import { commands } from '../../../src/types/bindings'

vi.mock('vue-i18n', () => ({ useI18n: () => ({ t: (key: string) => key }) }))
vi.mock('vue-router', () => ({ useRouter: () => ({ back: vi.fn() }) }))
vi.mock('../../../src/stores/ui', () => ({ useUiStore: () => ({ updateChannel: 'Stable' }) }))
vi.mock('../../../src/types/bindings', () => ({
  commands: { version: vi.fn(async () => ({ app: '6.1.4' })), checkUpdate: vi.fn() },
}))

afterEach(() => vi.restoreAllMocks())

describe('manual update checks', () => {
  it.each(['no-update', 'failed'] as const)(
    'distinguishes %s and releases the loading state',
    async (outcome) => {
      const info = vi.spyOn(ElMessage, 'info').mockImplementation(() => ({ close: vi.fn() }))
      const warning = vi.spyOn(ElMessage, 'warning').mockImplementation(() => ({ close: vi.fn() }))
      vi.mocked(commands.checkUpdate).mockResolvedValue(
        outcome === 'no-update'
          ? { status: 'ok', data: null }
          : {
              status: 'error',
              error: { code: 'update.timeout', message: 'Timed out', details: null },
            },
      )
      const wrapper = shallowMount(About)
      await wrapper.get('[data-test="about-check-update"]').trigger('click')
      await flushPromises()
      if (outcome === 'no-update') {
        expect(info).toHaveBeenCalledWith('NoUpdatesDetected')
        expect(warning).not.toHaveBeenCalled()
      } else {
        expect(warning).toHaveBeenCalledWith('UpdateCheckFailed')
        expect(info).not.toHaveBeenCalled()
      }
      expect(wrapper.get('[data-test="about-check-update"]').classes()).not.toContain(
        'about__check-update--disabled',
      )
      wrapper.unmount()
    },
  )
})
