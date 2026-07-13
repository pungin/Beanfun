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
import { ANNOUNCEMENT_ID, ANNOUNCEMENT_FORCED_SECONDS } from '../../../src/constants/announcement'
import AnnouncementModal from '../../../src/components/AnnouncementModal.vue'

const ok = <T>(data: T): Promise<Result<T, CommandError>> => Promise.resolve({ status: 'ok', data })

const mockOpenUrl = vi.mocked(commands.openUrl)
const mockSetConfig = vi.mocked(commands.setConfig)

const SEEN_KEY = 'announcementSeenVersion'
/** Forced-read countdown in ms. */
const READ_MS = ANNOUNCEMENT_FORCED_SECONDS * 1000

let pinia: ReturnType<typeof createPinia>

function mountModal() {
  return mount(AnnouncementModal, { global: { plugins: [pinia, createAppI18n()] } })
}

describe('AnnouncementModal', () => {
  beforeEach(() => {
    pinia = createPinia()
    setActivePinia(pinia)
    // The modal defers its seen-check until the config store has loaded
    // Config.xml (see `waitForConfigLoaded`). Boot has completed by the
    // time it mounts in the app, so model that here; the dedicated race
    // test below flips this back to `false`.
    useConfigStore().loaded = true
    vi.useFakeTimers()
    // The "seen" flag is now also mirrored into localStorage, which is a
    // shared global across tests in this file — wipe it so each test
    // starts from an unacknowledged state.
    localStorage.clear()
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

  it('does not auto-show when acknowledged, but offers the re-open chip', async () => {
    const config = useConfigStore()
    config.entries[SEEN_KEY] = ANNOUNCEMENT_ID
    const wrapper = mountModal()
    await flushPromises()
    expect(wrapper.find('[data-testid="announcement"]').exists()).toBe(false)
    expect(wrapper.find('[data-testid="announcement-banner-open"]').exists()).toBe(true)
  })

  it('re-opens in review mode (no countdown) when the chip is clicked', async () => {
    const config = useConfigStore()
    config.entries[SEEN_KEY] = ANNOUNCEMENT_ID
    const wrapper = mountModal()
    await flushPromises()

    await wrapper.get('[data-testid="announcement-banner-open"]').trigger('click')
    await flushPromises()

    expect(wrapper.find('[data-testid="announcement"]').exists()).toBe(true)
    // Review mode: the button is immediately usable (no 60s gate).
    expect(
      (wrapper.get('[data-testid="announcement-dismiss"]').element as HTMLButtonElement).disabled,
    ).toBe(false)

    // Closing returns to the chip, without re-persisting (already seen).
    await wrapper.get('[data-testid="announcement-dismiss"]').trigger('click')
    await flushPromises()
    expect(wrapper.find('[data-testid="announcement"]').exists()).toBe(false)
    expect(wrapper.find('[data-testid="announcement-banner-open"]').exists()).toBe(true)
    expect(commands.setConfig).not.toHaveBeenCalled()
  })

  it('keeps the re-open banner permanent (no session-hide control)', async () => {
    const config = useConfigStore()
    config.entries[SEEN_KEY] = ANNOUNCEMENT_ID
    const wrapper = mountModal()
    await flushPromises()

    expect(wrapper.find('[data-testid="announcement-banner"]').exists()).toBe(true)
    // The banner is permanent — there is no × / hide affordance.
    expect(wrapper.find('[data-testid="announcement-banner-hide"]').exists()).toBe(false)
  })

  it('treats a legacy app-version value as seen (pre-ID builds, no re-force on update)', async () => {
    // Builds before the ID mechanism stored the acknowledged APP VERSION.
    // Everyone who acknowledged any of those versions read the current
    // (issue #323) notice, so upgrading must not force them again.
    const config = useConfigStore()
    config.entries[SEEN_KEY] = '6.0.5'
    const wrapper = mountModal()
    await flushPromises()
    expect(wrapper.find('[data-testid="announcement"]').exists()).toBe(false)
    expect(wrapper.find('[data-testid="announcement-banner"]').exists()).toBe(true)
  })

  it('forces the read when the stored value is a different announcement ID', async () => {
    // A previously published (hypothetical) announcement was read, but
    // the shipped ID has since been bumped — the new notice must force.
    const config = useConfigStore()
    config.entries[SEEN_KEY] = '2020-01-some-older-announcement'
    const wrapper = mountModal()
    await flushPromises()
    expect(wrapper.find('[data-testid="announcement"]').exists()).toBe(true)
  })

  it('treats the announcement as seen when only localStorage records it (Config.xml wiped)', async () => {
    // Config.xml has no seen entry, but localStorage does — either store
    // alone must suppress the forced read so hand-editing one file can't
    // re-trigger the countdown on the next launch.
    localStorage.setItem(SEEN_KEY, ANNOUNCEMENT_ID)
    const wrapper = mountModal()
    await flushPromises()

    expect(wrapper.find('[data-testid="announcement"]').exists()).toBe(false)
    expect(wrapper.find('[data-testid="announcement-banner"]').exists()).toBe(true)
  })

  it('mirrors the acknowledged ID into localStorage on forced dismiss', async () => {
    const wrapper = mountModal()
    await flushPromises()

    vi.advanceTimersByTime(READ_MS)
    await nextTick()
    await wrapper.get('[data-testid="announcement-dismiss"]').trigger('click')
    await flushPromises()

    expect(localStorage.getItem(SEEN_KEY)).toBe(ANNOUNCEMENT_ID)
  })

  it('disables dismiss during the forced-read countdown, then enables it', async () => {
    const wrapper = mountModal()
    await flushPromises()
    const btn = () => wrapper.get('[data-testid="announcement-dismiss"]')
    expect((btn().element as HTMLButtonElement).disabled).toBe(true)

    vi.advanceTimersByTime(READ_MS)
    await nextTick()
    expect((btn().element as HTMLButtonElement).disabled).toBe(false)
  })

  it('persists the ID and hides on dismiss (after the countdown)', async () => {
    const wrapper = mountModal()
    await flushPromises()

    vi.advanceTimersByTime(READ_MS)
    await nextTick()
    await wrapper.get('[data-testid="announcement-dismiss"]').trigger('click')
    await flushPromises()

    expect(commands.setConfig).toHaveBeenCalledWith(SEEN_KEY, ANNOUNCEMENT_ID)
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

  it('waits for Config.xml to load before the seen-check (no premature forced read)', async () => {
    // Regression: the seen-check used to run on mount before the config
    // cache was populated, so an already-acknowledged announcement read
    // as unseen and re-forced the 30s read on every launch.
    const config = useConfigStore()
    config.loaded = false // still booting; cache is empty
    const wrapper = mountModal()
    await flushPromises()
    // Must not force the read while the cache is still loading.
    expect(wrapper.find('[data-testid="announcement"]').exists()).toBe(false)

    // loadAll() resolves — the acknowledged ID lands in the cache.
    config.entries[SEEN_KEY] = ANNOUNCEMENT_ID
    config.loaded = true
    await flushPromises()

    // Seen → stays closed (and the re-open banner is available).
    expect(wrapper.find('[data-testid="announcement"]').exists()).toBe(false)
    expect(wrapper.find('[data-testid="announcement-banner"]').exists()).toBe(true)
  })

  it('opens external links via openUrl', async () => {
    const wrapper = mountModal()
    await flushPromises()
    await wrapper.get('[data-testid="announcement-maplelink"]').trigger('click')
    expect(commands.openUrl).toHaveBeenCalledWith('https://github.com/lshw54/maplelink')
  })
})
