import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { flushPromises, mount } from '@vue/test-utils'
import { createPinia, setActivePinia } from 'pinia'
import { nextTick } from 'vue'

import type { CommandError, Result } from '../../../src/types/bindings'

vi.mock('element-plus', () => ({ ElMessage: { error: vi.fn(), warning: vi.fn() } }))

vi.mock('../../../src/types/bindings', () => ({
  commands: {
    version: vi.fn(),
    openUrl: vi.fn(),
    getAllConfig: vi.fn(),
    setConfig: vi.fn(),
  },
}))

import { commands } from '../../../src/types/bindings'
import { createAppI18n } from '../../../src/i18n'
import { useConfigStore } from '../../../src/stores/config'
import AnnouncementModal from '../../../src/components/AnnouncementModal.vue'

const ok = <T>(data: T): Promise<Result<T, CommandError>> => Promise.resolve({ status: 'ok', data })

const mockVersion = vi.mocked(commands.version)
const mockOpenUrl = vi.mocked(commands.openUrl)
const mockSetConfig = vi.mocked(commands.setConfig)

const SEEN_KEY = 'announcementSeenVersion'
const VERSION = { app: '6.0.3', tauri: '2.0.0' } as Awaited<ReturnType<typeof commands.version>>

let pinia: ReturnType<typeof createPinia>

function mountModal() {
  return mount(AnnouncementModal, { global: { plugins: [pinia, createAppI18n()] } })
}

describe('AnnouncementModal', () => {
  beforeEach(() => {
    pinia = createPinia()
    setActivePinia(pinia)
    vi.useFakeTimers()
    for (const fn of Object.values(commands) as ReturnType<typeof vi.fn>[]) fn.mockReset()
    mockVersion.mockResolvedValue(VERSION)
    mockOpenUrl.mockReturnValue(ok(null))
    mockSetConfig.mockReturnValue(ok(null))
  })

  afterEach(() => {
    vi.useRealTimers()
  })

  it('shows on first launch when the version has not been acknowledged', async () => {
    const wrapper = mountModal()
    await flushPromises()
    expect(wrapper.find('[data-testid="announcement"]').exists()).toBe(true)
  })

  it('stays hidden when the current version was already acknowledged', async () => {
    const config = useConfigStore()
    config.entries[SEEN_KEY] = VERSION.app
    const wrapper = mountModal()
    await flushPromises()
    expect(wrapper.find('[data-testid="announcement"]').exists()).toBe(false)
  })

  it('disables dismiss during the forced-read countdown, then enables it', async () => {
    const wrapper = mountModal()
    await flushPromises()
    const btn = () => wrapper.get('[data-testid="announcement-dismiss"]')
    expect((btn().element as HTMLButtonElement).disabled).toBe(true)

    vi.advanceTimersByTime(60_000)
    await nextTick()
    expect((btn().element as HTMLButtonElement).disabled).toBe(false)
  })

  it('persists the version and hides on dismiss (after the countdown)', async () => {
    const wrapper = mountModal()
    await flushPromises()

    vi.advanceTimersByTime(60_000)
    await nextTick()
    await wrapper.get('[data-testid="announcement-dismiss"]').trigger('click')
    await flushPromises()

    expect(commands.setConfig).toHaveBeenCalledWith(SEEN_KEY, VERSION.app)
    expect(wrapper.find('[data-testid="announcement"]').exists()).toBe(false)
  })

  it('does not persist or close while the countdown is still running', async () => {
    const wrapper = mountModal()
    await flushPromises()

    await wrapper.get('[data-testid="announcement-dismiss"]').trigger('click')
    await flushPromises()

    expect(commands.setConfig).not.toHaveBeenCalled()
    expect(wrapper.find('[data-testid="announcement"]').exists()).toBe(true)
  })

  it('opens external links via openUrl', async () => {
    const wrapper = mountModal()
    await flushPromises()
    await wrapper.get('[data-testid="announcement-maplelink"]').trigger('click')
    expect(commands.openUrl).toHaveBeenCalledWith('https://github.com/lshw54/maplelink')
  })
})
