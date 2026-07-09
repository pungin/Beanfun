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
/** Forced-read countdown in ms — mirrors READ_SECONDS (30) in the SUT. */
const READ_MS = 30_000
const VERSION = { app: '6.0.3', tauri: '2.0.0' } as Awaited<ReturnType<typeof commands.version>>

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

  it('does not auto-show when acknowledged, but offers the re-open chip', async () => {
    const config = useConfigStore()
    config.entries[SEEN_KEY] = VERSION.app
    const wrapper = mountModal()
    await flushPromises()
    expect(wrapper.find('[data-testid="announcement"]').exists()).toBe(false)
    expect(wrapper.find('[data-testid="announcement-banner-open"]').exists()).toBe(true)
  })

  it('re-opens in review mode (no countdown) when the chip is clicked', async () => {
    const config = useConfigStore()
    config.entries[SEEN_KEY] = VERSION.app
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
    config.entries[SEEN_KEY] = VERSION.app
    const wrapper = mountModal()
    await flushPromises()

    expect(wrapper.find('[data-testid="announcement-banner"]').exists()).toBe(true)
    // The banner is permanent — there is no × / hide affordance.
    expect(wrapper.find('[data-testid="announcement-banner-hide"]').exists()).toBe(false)
  })

  it('treats the version as seen when only localStorage records it (Config.xml wiped)', async () => {
    // Config.xml has no seen entry, but localStorage does — either store
    // alone must suppress the forced read so hand-editing one file can't
    // re-trigger the countdown on the next launch.
    localStorage.setItem(SEEN_KEY, VERSION.app)
    const wrapper = mountModal()
    await flushPromises()

    expect(wrapper.find('[data-testid="announcement"]').exists()).toBe(false)
    expect(wrapper.find('[data-testid="announcement-banner"]').exists()).toBe(true)
  })

  it('mirrors the acknowledged version into localStorage on forced dismiss', async () => {
    const wrapper = mountModal()
    await flushPromises()

    vi.advanceTimersByTime(READ_MS)
    await nextTick()
    await wrapper.get('[data-testid="announcement-dismiss"]').trigger('click')
    await flushPromises()

    expect(localStorage.getItem(SEEN_KEY)).toBe(VERSION.app)
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

  it('persists the version and hides on dismiss (after the countdown)', async () => {
    const wrapper = mountModal()
    await flushPromises()

    vi.advanceTimersByTime(READ_MS)
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

  it('waits for Config.xml to load before the seen-check (no premature forced read)', async () => {
    // Regression: the seen-check used to run on mount before the config
    // cache was populated, so an already-acknowledged version read as
    // unseen and re-forced the 30s read on every launch.
    const config = useConfigStore()
    config.loaded = false // still booting; cache is empty
    const wrapper = mountModal()
    await flushPromises()
    // Must not force the read while the cache is still loading.
    expect(wrapper.find('[data-testid="announcement"]').exists()).toBe(false)

    // loadAll() resolves — the acknowledged version lands in the cache.
    config.entries[SEEN_KEY] = VERSION.app
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
