/**
 * Announcement overlay / banner / archive specs.
 */

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { flushPromises, mount } from '@vue/test-utils'
import { createPinia, setActivePinia } from 'pinia'

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
  ANNOUNCEMENT_BANNER_KEY,
  ANNOUNCEMENT_SEEN_KEY,
  LATEST_ANNOUNCEMENT,
  LEGACY_ANNOUNCEMENT_SEEN_KEY,
} from '../../../src/constants/announcement'
import { closeAnnouncementList, openAnnouncementList } from '../../../src/services/announcementUi'
import AnnouncementModal from '../../../src/components/AnnouncementModal.vue'

const ok = <T>(data: T): Promise<Result<T, CommandError>> => Promise.resolve({ status: 'ok', data })

const mockOpenUrl = vi.mocked(commands.openUrl)
const mockSetConfig = vi.mocked(commands.setConfig)

/** The shipped notice — `info`, so it is closable at once. */
const CURRENT = LATEST_ANNOUNCEMENT

let pinia: ReturnType<typeof createPinia>

function mountModal() {
  return mount(AnnouncementModal, { global: { plugins: [pinia, createAppI18n()] } })
}

describe('AnnouncementModal', () => {
  beforeEach(() => {
    pinia = createPinia()
    setActivePinia(pinia)
    // The modal defers its read-check until Config.xml has loaded.
    useConfigStore().loaded = true
    localStorage.clear()
    closeAnnouncementList()
    for (const fn of Object.values(commands) as ReturnType<typeof vi.fn>[]) fn.mockReset()
    mockOpenUrl.mockReturnValue(ok(null))
    mockSetConfig.mockReturnValue(ok(null))
  })

  afterEach(() => {
    vi.useRealTimers()
  })

  it('shows the newest unread announcement on launch', async () => {
    const wrapper = mountModal()
    await flushPromises()
    expect(wrapper.find('[data-testid="announcement"]').exists()).toBe(true)
    expect(wrapper.find(`[data-testid="announcement-body-${CURRENT.id}"]`).exists()).toBe(true)
  })

  it('is closable at once at info level, and closing counts as read', async () => {
    const wrapper = mountModal()
    await flushPromises()

    // No countdown: the acknowledge button is live immediately…
    const btn = wrapper.get('[data-testid="announcement-dismiss"]')
    expect((btn.element as HTMLButtonElement).disabled).toBe(false)
    // …and the × is offered too.
    expect(wrapper.find('[data-testid="announcement-close"]').exists()).toBe(true)

    await btn.trigger('click')
    await flushPromises()

    expect(wrapper.find('[data-testid="announcement"]').exists()).toBe(false)
    expect(commands.setConfig).toHaveBeenCalledWith(ANNOUNCEMENT_SEEN_KEY, CURRENT.id)
    expect(localStorage.getItem(ANNOUNCEMENT_SEEN_KEY)).toBe(CURRENT.id)
  })

  it('closes from the × and still counts as read at info level', async () => {
    const wrapper = mountModal()
    await flushPromises()

    await wrapper.get('[data-testid="announcement-close"]').trigger('click')
    await flushPromises()

    expect(wrapper.find('[data-testid="announcement"]').exists()).toBe(false)
    expect(commands.setConfig).toHaveBeenCalledWith(ANNOUNCEMENT_SEEN_KEY, CURRENT.id)
  })

  it('closes from the backdrop', async () => {
    const wrapper = mountModal()
    await flushPromises()

    await wrapper.get('[data-testid="announcement"]').trigger('click')
    await flushPromises()

    expect(wrapper.find('[data-testid="announcement"]').exists()).toBe(false)
  })

  it('does not re-open once read', async () => {
    useConfigStore().entries[ANNOUNCEMENT_SEEN_KEY] = CURRENT.id
    const wrapper = mountModal()
    await flushPromises()
    expect(wrapper.find('[data-testid="announcement"]').exists()).toBe(false)
    expect(wrapper.find('[data-testid="announcement-banner"]').exists()).toBe(true)
  })

  it('treats a legacy app-version value as acknowledging the inaugural notice', async () => {
    useConfigStore().entries[LEGACY_ANNOUNCEMENT_SEEN_KEY] = '6.0.5'
    const wrapper = mountModal()
    await flushPromises()
    expect(wrapper.find('[data-testid="announcement"]').exists()).toBe(false)
  })

  it('waits for Config.xml to load before the read-check', async () => {
    const config = useConfigStore()
    config.loaded = false
    const wrapper = mountModal()
    await flushPromises()
    expect(wrapper.find('[data-testid="announcement"]').exists()).toBe(false)

    config.entries[ANNOUNCEMENT_SEEN_KEY] = CURRENT.id
    config.loaded = true
    await flushPromises()

    expect(wrapper.find('[data-testid="announcement"]').exists()).toBe(false)
  })

  it('opens external links from the body via openUrl', async () => {
    const wrapper = mountModal()
    await flushPromises()
    await wrapper.get('[data-testid="announcement-maplelink"]').trigger('click')
    expect(commands.openUrl).toHaveBeenCalledWith('https://github.com/lshw54/maplelink')
  })

  describe('banner', () => {
    beforeEach(() => {
      useConfigStore().entries[ANNOUNCEMENT_SEEN_KEY] = CURRENT.id
    })

    it('opens the notice itself — no intermediate list', async () => {
      const wrapper = mountModal()
      await flushPromises()

      await wrapper.get('[data-testid="announcement-banner-open"]').trigger('click')
      await flushPromises()

      expect(wrapper.find('[data-testid="announcement"]').exists()).toBe(true)
      expect(wrapper.find('[data-testid="announcement-list"]').exists()).toBe(false)
    })

    it('can be dismissed for good, and stays gone on the next launch', async () => {
      const wrapper = mountModal()
      await flushPromises()

      await wrapper.get('[data-testid="announcement-banner-dismiss"]').trigger('click')
      await flushPromises()

      expect(wrapper.find('[data-testid="announcement-banner"]').exists()).toBe(false)
      expect(commands.setConfig).toHaveBeenCalledWith(ANNOUNCEMENT_BANNER_KEY, CURRENT.id)

      // A fresh mount with that record keeps it hidden.
      useConfigStore().entries[ANNOUNCEMENT_BANNER_KEY] = CURRENT.id
      const next = mountModal()
      await flushPromises()
      expect(next.find('[data-testid="announcement-banner"]').exists()).toBe(false)
    })
  })

  describe('archive', () => {
    beforeEach(() => {
      useConfigStore().entries[ANNOUNCEMENT_SEEN_KEY] = CURRENT.id
    })

    it('opens from the shared opener (Settings uses this) and lists subject + date', async () => {
      const wrapper = mountModal()
      await flushPromises()
      expect(wrapper.find('[data-testid="announcement-list"]').exists()).toBe(false)

      openAnnouncementList()
      await flushPromises()

      const list = wrapper.get('[data-testid="announcement-list"]')
      for (const def of ANNOUNCEMENTS) {
        const row = list.get(`[data-testid="announcement-history-${def.id}"]`)
        expect(row.text()).toContain(def.date)
      }
    })

    it('opens a picked notice, rendering the same body the overlay does', async () => {
      const wrapper = mountModal()
      await flushPromises()
      openAnnouncementList()
      await flushPromises()

      await wrapper.get(`[data-testid="announcement-history-${CURRENT.id}"]`).trigger('click')
      await flushPromises()

      expect(wrapper.find('[data-testid="announcement-list"]').exists()).toBe(false)
      expect(wrapper.find(`[data-testid="announcement-body-${CURRENT.id}"]`).exists()).toBe(true)
      // Already read — re-reading must not write again.
      expect(commands.setConfig).not.toHaveBeenCalled()
    })

    it('closes from its own ×', async () => {
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
