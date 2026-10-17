import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { flushPromises, mount } from '@vue/test-utils'
import { createI18n } from 'vue-i18n'
import { ElMessage } from 'element-plus'
import UpdateConfirmation from '../../../src/components/UpdateConfirmation.vue'
import UpdateFallback from '../../../src/components/UpdateFallback.vue'
import UpdateReleaseLink from '../../../src/components/UpdateReleaseLink.vue'
import { FRONTEND_ONLY_MESSAGES } from '../../../src/i18n/messages'
import { commands } from '../../../src/types/bindings'

vi.mock('../../../src/types/bindings', () => ({ commands: { openUrl: vi.fn() } }))
vi.mock('element-plus', () => ({ ElMessage: { warning: vi.fn() } }))

function mountConfirmation(locale = 'en-US', releaseTag = 'v6.2.3.2610181200') {
  return mount(UpdateConfirmation, {
    props: { currentVersion: '6.2.2', newVersion: '6.2.3', releaseTag },
    global: {
      plugins: [createI18n({ legacy: false, locale, messages: FRONTEND_ONLY_MESSAGES })],
    },
  })
}

beforeEach(() => {
  vi.mocked(commands.openUrl).mockReset().mockResolvedValue({ status: 'ok', data: null })
  vi.spyOn(console, 'warn').mockImplementation(() => {})
})

afterEach(() => {
  vi.clearAllMocks()
  vi.restoreAllMocks()
})

describe('compact update confirmation', () => {
  it.each(['zh-TW', 'zh-CN', 'en-US'] as const)(
    'shows translated versions, installation details and a release notes link in %s',
    (locale) => {
      const wrapper = mountConfirmation(locale)
      expect(wrapper.text()).toContain('6.2.2')
      expect(wrapper.text()).toContain('6.2.3')
      expect(wrapper.text()).toContain(FRONTEND_ONLY_MESSAGES[locale].appUpdate.confirmBody)
      expect(wrapper.text()).toContain(
        FRONTEND_ONLY_MESSAGES[locale].appUpdate.cancelForManualDownload,
      )
      expect(wrapper.get('a').text()).toBe(FRONTEND_ONLY_MESSAGES[locale].appUpdate.releasePage)
      expect(wrapper.text()).not.toContain('appUpdate.')
      expect(wrapper.text()).not.toMatch(/\{(?:current|version|notes)\}/)
      wrapper.unmount()
    },
  )

  it('opens only the official release page and encodes the whole tag as one path segment', async () => {
    const wrapper = mountConfirmation(
      'en-US',
      'v6.2.3/notes?redirect=https://example.invalid#details',
    )
    const expectedUrl =
      'https://github.com/pungin/Beanfun/releases/tag/v6.2.3%2Fnotes%3Fredirect%3Dhttps%3A%2F%2Fexample.invalid%23details'
    expect(wrapper.get('a').attributes('href')).toBe(expectedUrl)
    await wrapper.get('a').trigger('click')
    await flushPromises()
    expect(commands.openUrl).toHaveBeenCalledExactlyOnceWith(expectedUrl)
    expect(ElMessage.warning).not.toHaveBeenCalled()
    wrapper.unmount()
  })

  it.each(['result', 'exception'] as const)(
    'reports a %s failure opening release notes',
    async (failure) => {
      if (failure === 'result') {
        vi.mocked(commands.openUrl).mockResolvedValueOnce({
          status: 'error',
          error: { code: 'system.open_url_failed', message: 'Could not open URL', details: null },
        })
      } else {
        vi.mocked(commands.openUrl).mockRejectedValueOnce(new Error('IPC unavailable'))
      }
      const wrapper = mountConfirmation()
      await wrapper.get('a').trigger('click')
      await flushPromises()
      expect(ElMessage.warning).toHaveBeenCalledWith(
        FRONTEND_ONLY_MESSAGES['en-US'].appUpdate.releasePageFailed,
      )
      expect(wrapper.exists()).toBe(true)
      wrapper.unmount()
    },
  )

  it.each(['zh-TW', 'zh-CN', 'en-US'] as const)(
    'keeps the manual release page and the failure reason accessible in %s',
    (locale) => {
      const wrapper = mount(UpdateFallback, {
        props: { reason: 'Synthetic failure', releaseTag: 'v6.2.3.2610181200' },
        global: {
          plugins: [createI18n({ legacy: false, locale, messages: FRONTEND_ONLY_MESSAGES })],
        },
      })
      expect(wrapper.get('[data-test="update-failure-reason"]').text()).toBe('Synthetic failure')
      expect(wrapper.text()).toContain(FRONTEND_ONLY_MESSAGES[locale].appUpdate.manualDownloadHint)
      expect(wrapper.get('a').attributes('href')).toBe(
        'https://github.com/pungin/Beanfun/releases/tag/v6.2.3.2610181200',
      )
      wrapper.unmount()
    },
  )

  it('uses the releases index when a failed check has no release tag', async () => {
    const wrapper = mount(UpdateReleaseLink, {
      global: {
        plugins: [createI18n({ legacy: false, locale: 'en-US', messages: FRONTEND_ONLY_MESSAGES })],
      },
    })
    expect(commands.openUrl).not.toHaveBeenCalled()
    await wrapper.get('a').trigger('click')
    await flushPromises()
    expect(commands.openUrl).toHaveBeenCalledExactlyOnceWith(
      'https://github.com/pungin/Beanfun/releases',
    )
    wrapper.unmount()
  })
})
