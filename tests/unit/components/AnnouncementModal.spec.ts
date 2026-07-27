/**
 * Announcement modal specs — the three levels, the per-id acknowledgement
 * record, and the history dialog.
 */

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
import {
  ANNOUNCEMENTS,
  ANNOUNCEMENT_SEEN_KEY,
  LATEST_ANNOUNCEMENT,
  LEGACY_ANNOUNCEMENT_SEEN_KEY,
} from '../../../src/constants/announcement'
import { closeAnnouncementList, openAnnouncementList } from '../../../src/services/announcementUi'
import AnnouncementModal from '../../../src/components/AnnouncementModal.vue'

const ok = <T>(data: T): Promise<Result<T, CommandError>> => Promise.resolve({ status: 'ok', data })

const mockOpenUrl = vi.mocked(commands.openUrl)
const mockSetConfig = vi.mocked(commands.setConfig)

/** The shipped notice: `forced`, with its own short countdown. */
const CURRENT = LATEST_ANNOUNCEMENT
const READ_MS = CURRENT.forcedSeconds * 1000

let pinia: ReturnType<typeof createPinia>

function mountModal() {
  return mount(AnnouncementModal, { global: { plugins: [pinia, createAppI18n()] } })
}

describe('AnnouncementModal', () => {
  beforeEach(() => {
    pinia = createPinia()
    setActivePinia(pinia)
    // The modal defers its seen-check until the config store has loaded
    // Config.xml; boot has completed by the time it mounts in the app.
    useConfigStore().loaded = true
    vi.useFakeTimers()
    localStorage.clear()
    closeAnnouncementList()
    for (const fn of Object.values(commands) as ReturnType<typeof vi.fn>[]) fn.mockReset()
    mockOpenUrl.mockReturnValue(ok(null))
    mockSetConfig.mockReturnValue(ok(null))
  })

  afterEach(() => {
    vi.useRealTimers()
  })

  it('shows on first launch when the announcement has not been acknowledged', async () => {
    const wrapper = mountModal()
    await flushPromises()
    expect(wrapper.find('[data-testid="announcement"]').exists()).toBe(true)
  })

  it("counts down for the announcement's own forcedSeconds, not a global constant", async () => {
    const wrapper = mountModal()
    await flushPromises()
    const btn = () => wrapper.get('[data-testid="announcement-dismiss"]')
    expect((btn().element as HTMLButtonElement).disabled).toBe(true)

    // One second short of this announcement's countdown: still locked.
    vi.advanceTimersByTime(READ_MS - 1000)
    await nextTick()
    expect((btn().element as HTMLButtonElement).disabled).toBe(true)

    vi.advanceTimersByTime(1000)
    await nextTick()
    expect((btn().element as HTMLButtonElement).disabled).toBe(false)
  })

  it('does not auto-show when acknowledged, but leaves the banner', async () => {
    const config = useConfigStore()
    config.entries[ANNOUNCEMENT_SEEN_KEY] = CURRENT.id
    const wrapper = mountModal()
    await flushPromises()
    expect(wrapper.find('[data-testid="announcement"]').exists()).toBe(false)
    expect(wrapper.find('[data-testid="announcement-banner-open"]').exists()).toBe(true)
  })

  it('records the acknowledged id in both stores on dismiss', async () => {
    const wrapper = mountModal()
    await flushPromises()

    vi.advanceTimersByTime(READ_MS)
    await nextTick()
    await wrapper.get('[data-testid="announcement-dismiss"]').trigger('click')
    await flushPromises()

    expect(commands.setConfig).toHaveBeenCalledWith(ANNOUNCEMENT_SEEN_KEY, CURRENT.id)
    expect(localStorage.getItem(ANNOUNCEMENT_SEEN_KEY)).toBe(CURRENT.id)
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

  it('treats a lone-id value in localStorage as acknowledged (Config.xml wiped)', async () => {
    localStorage.setItem(ANNOUNCEMENT_SEEN_KEY, CURRENT.id)
    const wrapper = mountModal()
    await flushPromises()
    expect(wrapper.find('[data-testid="announcement"]').exists()).toBe(false)
  })

  it('treats a legacy app-version value as acknowledging the inaugural notice', async () => {
    // Pre-registry builds stored the app version under the old key.
    const config = useConfigStore()
    config.entries[LEGACY_ANNOUNCEMENT_SEEN_KEY] = '6.0.5'
    const wrapper = mountModal()
    await flushPromises()
    expect(wrapper.find('[data-testid="announcement"]').exists()).toBe(false)
  })

  it('waits for Config.xml to load before the seen-check', async () => {
    const config = useConfigStore()
    config.loaded = false
    const wrapper = mountModal()
    await flushPromises()
    expect(wrapper.find('[data-testid="announcement"]').exists()).toBe(false)

    config.entries[ANNOUNCEMENT_SEEN_KEY] = CURRENT.id
    config.loaded = true
    await flushPromises()

    expect(wrapper.find('[data-testid="announcement"]').exists()).toBe(false)
    expect(wrapper.find('[data-testid="announcement-banner"]').exists()).toBe(true)
  })

  it('opens external links via openUrl', async () => {
    const wrapper = mountModal()
    await flushPromises()
    const link = CURRENT.links?.[0]
    expect(link).toBeDefined()
    await wrapper.get(`[data-testid="announcement-link-${link!.labelKey}"]`).trigger('click')
    expect(commands.openUrl).toHaveBeenCalledWith(link!.url)
  })

  describe('history dialog', () => {
    beforeEach(() => {
      // Acknowledged, so the modal is idle and the banner is up.
      useConfigStore().entries[ANNOUNCEMENT_SEEN_KEY] = CURRENT.id
    })

    it('opens from the banner and lists every announcement with its state', async () => {
      const wrapper = mountModal()
      await flushPromises()

      await wrapper.get('[data-testid="announcement-banner-open"]').trigger('click')
      await flushPromises()

      const list = wrapper.get('[data-testid="announcement-list"]')
      for (const def of ANNOUNCEMENTS) {
        expect(list.find(`[data-testid="announcement-history-${def.id}"]`).exists()).toBe(true)
      }
      expect(list.text()).toContain('已讀')
    })

    it('opens from anywhere via the shared opener (Settings uses this)', async () => {
      const wrapper = mountModal()
      await flushPromises()
      expect(wrapper.find('[data-testid="announcement-list"]').exists()).toBe(false)

      openAnnouncementList()
      await flushPromises()

      expect(wrapper.find('[data-testid="announcement-list"]').exists()).toBe(true)
    })

    it('re-reads a picked announcement without a countdown and without re-recording', async () => {
      const wrapper = mountModal()
      await flushPromises()
      openAnnouncementList()
      await flushPromises()

      await wrapper.get(`[data-testid="announcement-history-${CURRENT.id}"]`).trigger('click')
      await flushPromises()

      expect(wrapper.find('[data-testid="announcement"]').exists()).toBe(true)
      const btn = wrapper.get('[data-testid="announcement-dismiss"]')
      expect((btn.element as HTMLButtonElement).disabled).toBe(false)

      await btn.trigger('click')
      await flushPromises()
      // Already acknowledged — a review close must not write again.
      expect(commands.setConfig).not.toHaveBeenCalled()
    })

    it('closes from its own button', async () => {
      const wrapper = mountModal()
      await flushPromises()
      openAnnouncementList()
      await flushPromises()

      await wrapper.get('[data-testid="announcement-list-close"]').trigger('click')
      await flushPromises()

      expect(wrapper.find('[data-testid="announcement-list"]').exists()).toBe(false)
    })
  })
})
