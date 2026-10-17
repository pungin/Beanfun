import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { flushPromises, shallowMount } from '@vue/test-utils'
import { listen } from '@tauri-apps/api/event'
import { ElMessage, ElMessageBox, ElProgress } from 'element-plus'
import About from '../../../src/pages/About.vue'
import UpdateConfirmation from '../../../src/components/UpdateConfirmation.vue'
import UpdateFallback from '../../../src/components/UpdateFallback.vue'
import { commands, type CommandError, type Result } from '../../../src/types/bindings'
import type { AppUpdateProgress } from '../../../src/composables/useAppUpdate'
import {
  __resetInvokeRegistriesForTesting,
  registerErrorTranslator,
} from '../../../src/services/invoke'

const { state, releaseListener } = vi.hoisted(() => ({
  state: {
    leaveGuard: undefined as (() => boolean) | undefined,
    progressHandler: undefined as ((event: { payload: AppUpdateProgress }) => void) | undefined,
    updateChannel: 'Stable' as 'Stable' | 'Beta',
  },
  releaseListener: vi.fn(),
}))

vi.mock('vue-i18n', () => ({ useI18n: () => ({ t: (key: string) => key }) }))
vi.mock('vue-router', () => ({
  useRouter: () => ({ back: vi.fn() }),
  onBeforeRouteLeave: (guard: () => boolean) => {
    state.leaveGuard = guard
  },
}))
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn() }))
vi.mock('element-plus', async (importOriginal) => {
  const original = await importOriginal<typeof import('element-plus')>()
  return {
    ...original,
    ElMessage: Object.assign(vi.fn(), { error: vi.fn(), info: vi.fn(), warning: vi.fn() }),
    ElMessageBox: { confirm: vi.fn(), alert: vi.fn() },
  }
})
vi.mock('../../../src/stores/ui', () => ({ useUiStore: () => state }))
vi.mock('../../../src/types/bindings', () => ({
  commands: {
    version: vi.fn(async () => ({ app: '6.1.4' })),
    checkUpdate: vi.fn(),
    prepareUpdate: vi.fn(),
    installUpdate: vi.fn(),
    openUrl: vi.fn(),
  },
}))

beforeEach(() => {
  state.updateChannel = 'Stable'
  state.leaveGuard = undefined
  state.progressHandler = undefined
  registerErrorTranslator((key) => key)
  vi.mocked(listen)
    .mockReset()
    .mockImplementation(async (_event, handler) => {
      state.progressHandler = handler as typeof state.progressHandler
      return releaseListener
    })
  vi.mocked(ElMessageBox.confirm).mockReset().mockResolvedValue('confirm')
  vi.mocked(ElMessageBox.alert).mockReset().mockResolvedValue('confirm')
  vi.mocked(commands.prepareUpdate).mockReset().mockResolvedValue({ status: 'ok', data: null })
  vi.mocked(commands.installUpdate).mockReset().mockResolvedValue({ status: 'ok', data: null })
})

function expectUpdateFailure(code: string) {
  expect(ElMessageBox.alert).toHaveBeenCalledExactlyOnceWith(
    expect.objectContaining({
      type: UpdateFallback,
      props: { reason: `errors.update.${code}`, releaseTag: 'v6.3.0.2610181200' },
    }),
    'appUpdate.failedTitle',
    expect.objectContaining({ confirmButtonText: 'Close' }),
  )
  expect(ElMessage.error).not.toHaveBeenCalled()
  expect(ElMessage.warning).not.toHaveBeenCalled()
  expect(commands.openUrl).not.toHaveBeenCalled()
}

afterEach(() => {
  vi.restoreAllMocks()
  vi.clearAllMocks()
  vi.unstubAllGlobals()
  __resetInvokeRegistriesForTesting()
})

describe('QQ contact copying', () => {
  it.each([true, false])('uses translated copy feedback when success is %s', async (success) => {
    const writeText = success
      ? vi.fn().mockResolvedValue(undefined)
      : vi.fn().mockRejectedValue(new Error('clipboard unavailable'))
    vi.stubGlobal('navigator', { clipboard: { writeText } })
    vi.spyOn(console, 'error').mockImplementation(() => {})
    const wrapper = shallowMount(About)
    const contact = wrapper.findAll('a').find((link) => link.text() === 'QQ 2157875454')!
    await contact.trigger('click')
    await flushPromises()
    expect(writeText).toHaveBeenCalledWith('2157875454')
    if (success) {
      expect(ElMessage).toHaveBeenCalledWith(
        expect.objectContaining({ message: 'about.qqCopied', type: 'success' }),
      )
    } else {
      expect(ElMessage.error).toHaveBeenCalledWith('about.copyManually')
    }
    wrapper.unmount()
  })
})

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
        expect(ElMessageBox.alert).toHaveBeenCalledExactlyOnceWith(
          expect.objectContaining({
            type: UpdateFallback,
            props: { reason: 'errors.update.timeout', releaseTag: null },
          }),
          'UpdateCheckFailed',
          expect.any(Object),
        )
        expect(wrapper.findComponent(UpdateFallback).props('releaseTag')).toBeNull()
        expect(warning).not.toHaveBeenCalled()
        expect(info).not.toHaveBeenCalled()
      }
      expect(wrapper.get('[data-test="about-check-update"]').classes()).not.toContain(
        'about__check-update--disabled',
      )
      wrapper.unmount()
    },
  )
})

describe('automatic update installation', () => {
  beforeEach(() => {
    vi.mocked(commands.checkUpdate).mockResolvedValue({
      status: 'ok',
      data: {
        new_version_display: '6.3.0',
        tag_name: 'v6.3.0.2610181200',
        download_url: 'https://github.com/pungin/Beanfun/releases/download/example/Beanfun.exe',
        body: 'Release notes',
      },
    })
    vi.spyOn(console, 'error').mockImplementation(() => {})
    vi.spyOn(console, 'warn').mockImplementation(() => {})
  })

  it('subscribes before downloading, renders progress and locks navigation until restart', async () => {
    let finishPreparing!: (value: Result<null, CommandError>) => void
    vi.mocked(commands.prepareUpdate).mockReturnValueOnce(
      new Promise((resolve) => {
        finishPreparing = resolve
      }),
    )
    state.updateChannel = 'Beta'
    const wrapper = shallowMount(About)
    await wrapper.get('[data-test="about-check-update"]').trigger('click')
    await flushPromises()

    expect(ElMessageBox.confirm).toHaveBeenCalledWith(
      expect.objectContaining({ type: UpdateConfirmation }),
      'CheckUpdate',
      expect.objectContaining({ confirmButtonText: 'appUpdate.downloadAndRestart' }),
    )
    expect(commands.checkUpdate).toHaveBeenCalledWith('Beta', null)
    expect(commands.prepareUpdate).toHaveBeenCalledWith('Beta', 'v6.3.0.2610181200')
    expect(vi.mocked(listen).mock.invocationCallOrder[0]).toBeLessThan(
      vi.mocked(commands.prepareUpdate).mock.invocationCallOrder[0]!,
    )
    expect(state.leaveGuard?.()).toBe(false)
    expect(commands.installUpdate).not.toHaveBeenCalled()

    state.progressHandler?.({ payload: { phase: 'downloading', downloaded: 100, total: null } })
    await flushPromises()
    expect(wrapper.findComponent(ElProgress).props('indeterminate')).toBe(true)
    state.progressHandler?.({ payload: { phase: 'downloading', downloaded: 100, total: 200 } })
    await flushPromises()
    expect(wrapper.findComponent(ElProgress).props('percentage')).toBe(50)
    expect(wrapper.get('[data-test="about-update-status"]').text()).toContain(
      'appUpdate.downloading',
    )
    await wrapper.get('[data-test="about-check-update"]').trigger('click')
    expect(commands.checkUpdate).toHaveBeenCalledTimes(1)

    finishPreparing({ status: 'ok', data: null })
    await flushPromises()
    expect(commands.installUpdate).toHaveBeenCalledOnce()
    expect(commands.openUrl).not.toHaveBeenCalled()
    expect(wrapper.get('[data-test="about-update-status"]').text()).toContain(
      'appUpdate.installing',
    )
    expect(state.leaveGuard?.()).toBe(false)
    expect(releaseListener).toHaveBeenCalledOnce()
    wrapper.unmount()
    expect(releaseListener).toHaveBeenCalledOnce()
  })

  it('does not prepare or install when confirmation is cancelled', async () => {
    vi.mocked(ElMessageBox.confirm).mockRejectedValueOnce('cancel')
    const wrapper = shallowMount(About)
    await wrapper.get('[data-test="about-check-update"]').trigger('click')
    await flushPromises()
    expect(listen).not.toHaveBeenCalled()
    expect(commands.prepareUpdate).not.toHaveBeenCalled()
    expect(commands.installUpdate).not.toHaveBeenCalled()
    expect(commands.openUrl).not.toHaveBeenCalled()
    expect(wrapper.findComponent(UpdateFallback).props()).toEqual({
      releaseTag: 'v6.3.0.2610181200',
      reason: null,
    })
    expect(state.leaveGuard?.()).toBe(true)
    wrapper.unmount()
  })

  it.each(['download_failed', 'signature_invalid'] as const)(
    'does not install after %s and allows a fresh retry',
    async (code) => {
      vi.mocked(commands.prepareUpdate).mockResolvedValueOnce({
        status: 'error',
        error: { code: `update.${code}`, message: 'Synthetic failure', details: null },
      })
      const wrapper = shallowMount(About)
      await wrapper.get('[data-test="about-check-update"]').trigger('click')
      await flushPromises()
      expectUpdateFailure(code)
      expect(commands.installUpdate).not.toHaveBeenCalled()
      expect(state.leaveGuard?.()).toBe(true)
      expect(releaseListener).toHaveBeenCalledOnce()
      expect(wrapper.get('[data-test="about-update-status"]').text()).toContain('appUpdate.failed')
      expect(wrapper.findComponent(UpdateFallback).props()).toEqual({
        releaseTag: 'v6.3.0.2610181200',
        reason: `errors.update.${code}`,
      })

      await wrapper.get('[data-test="about-check-update"]').trigger('click')
      await flushPromises()
      expect(commands.prepareUpdate).toHaveBeenCalledTimes(2)
      expect(commands.installUpdate).toHaveBeenCalledOnce()
      expect(releaseListener).toHaveBeenCalledTimes(2)
      wrapper.unmount()
    },
  )

  it.each(['prepare', 'install'] as const)('releases the UI when %s IPC fails', async (stage) => {
    vi.mocked(
      stage === 'prepare' ? commands.prepareUpdate : commands.installUpdate,
    ).mockRejectedValueOnce(new Error('IPC connection lost'))
    const wrapper = shallowMount(About)
    await wrapper.get('[data-test="about-check-update"]').trigger('click')
    await flushPromises()
    expectUpdateFailure(`${stage}_failed`)
    expect(state.leaveGuard?.()).toBe(true)
    expect(releaseListener).toHaveBeenCalledOnce()
    wrapper.unmount()
  })

  it('surfaces an installation rejection without leaving the controls locked', async () => {
    vi.mocked(commands.installUpdate).mockResolvedValueOnce({
      status: 'error',
      error: {
        code: 'update.multiple_instances',
        message: 'Other instances are open',
        details: null,
      },
    })
    const wrapper = shallowMount(About)
    await wrapper.get('[data-test="about-check-update"]').trigger('click')
    await flushPromises()
    expectUpdateFailure('multiple_instances')
    expect(state.leaveGuard?.()).toBe(true)
    expect(releaseListener).toHaveBeenCalledOnce()
    wrapper.unmount()
  })

  it('does not start a download without a progress subscription', async () => {
    vi.mocked(listen).mockRejectedValueOnce(new Error('Event bridge unavailable'))
    const wrapper = shallowMount(About)
    await wrapper.get('[data-test="about-check-update"]').trigger('click')
    await flushPromises()
    expect(commands.prepareUpdate).not.toHaveBeenCalled()
    expectUpdateFailure('prepare_failed')
    expect(state.leaveGuard?.()).toBe(true)
    wrapper.unmount()
  })

  it('cleans up a pending subscription when the page is disposed', async () => {
    let finishListening!: (value: () => void) => void
    vi.mocked(listen).mockReturnValueOnce(
      new Promise((resolve) => {
        finishListening = resolve
      }),
    )
    const wrapper = shallowMount(About)
    await wrapper.get('[data-test="about-check-update"]').trigger('click')
    await flushPromises()
    wrapper.unmount()
    finishListening(releaseListener)
    await flushPromises()
    expect(releaseListener).toHaveBeenCalledOnce()
    expect(commands.prepareUpdate).not.toHaveBeenCalled()
  })

  it('releases the listener and does not start installation after the page is disposed', async () => {
    let finishPreparing!: (value: Result<null, CommandError>) => void
    vi.mocked(commands.prepareUpdate).mockReturnValueOnce(
      new Promise((resolve) => {
        finishPreparing = resolve
      }),
    )
    const wrapper = shallowMount(About)
    await wrapper.get('[data-test="about-check-update"]').trigger('click')
    await flushPromises()
    wrapper.unmount()
    expect(releaseListener).toHaveBeenCalledOnce()
    finishPreparing({ status: 'ok', data: null })
    await flushPromises()
    expect(commands.installUpdate).not.toHaveBeenCalled()
    expect(releaseListener).toHaveBeenCalledOnce()
  })
})
