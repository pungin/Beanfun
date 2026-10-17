import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { flushPromises } from '@vue/test-utils'
import { effectScope, reactive, ref, type EffectScope } from 'vue'
import { ElMessage, ElMessageBox } from 'element-plus'
import { createAppUpdateController } from '../../../src/composables/useAppUpdateController'
import {
  commands,
  type CommandError,
  type Result,
  type UpdateInfo,
} from '../../../src/types/bindings'

const state = vi.hoisted(() => ({
  ui: { askUpdate: true, updateChannel: 'Stable' as 'Stable' | 'Beta' },
  translate: vi.fn((key: string) => key),
  downloadAndInstall: vi.fn(),
}))

vi.mock('vue-i18n', () => ({ useI18n: () => ({ t: state.translate }) }))
vi.mock('element-plus', () => ({
  ElMessage: { info: vi.fn(), warning: vi.fn(), error: vi.fn() },
  ElMessageBox: { confirm: vi.fn() },
}))
vi.mock('../../../src/stores/ui', () => ({ useUiStore: () => state.ui }))
vi.mock('../../../src/types/bindings', () => ({
  commands: { version: vi.fn(), checkUpdate: vi.fn() },
}))
vi.mock('../../../src/composables/useAppUpdate', () => ({
  useAppUpdate: () => ({
    isUpdating: ref(false),
    phase: ref('idle'),
    downloaded: ref(0),
    total: ref(null),
    percentage: ref(null),
    downloadAndInstall: state.downloadAndInstall,
  }),
}))

const release: UpdateInfo = {
  new_version_display: '6.2.3',
  tag_name: 'v6.2.3.2610181200',
  body: 'Synthetic release notes',
  download_url: 'https://example.invalid/Beanfun.exe',
}
const scopes: EffectScope[] = []

function deferred<T>() {
  let resolve!: (value: T) => void
  const promise = new Promise<T>((complete) => {
    resolve = complete
  })
  return { promise, resolve }
}

function createHarness(options?: Parameters<typeof createAppUpdateController>[0]) {
  const scope = effectScope()
  scopes.push(scope)
  return { scope, controller: scope.run(() => createAppUpdateController(options))! }
}

beforeEach(() => {
  vi.clearAllMocks()
  state.ui = reactive({ askUpdate: true, updateChannel: 'Stable' })
  state.downloadAndInstall.mockReset().mockResolvedValue(undefined)
  vi.mocked(commands.version).mockReset().mockResolvedValue({ app: '6.2.2', tauri: '2.0.0' })
  vi.mocked(commands.checkUpdate).mockReset().mockResolvedValue({ status: 'ok', data: release })
  vi.mocked(ElMessageBox.confirm).mockReset().mockResolvedValue('confirm')
  vi.spyOn(console, 'warn').mockImplementation(() => {})
})

afterEach(() => {
  for (const scope of scopes.splice(0)) scope.stop()
  vi.restoreAllMocks()
})

describe('startup update checks', () => {
  it('does not check or prompt when startup notifications are disabled', async () => {
    state.ui.askUpdate = false
    const { controller } = createHarness()
    await controller.checkForUpdate(true)
    expect(commands.version).not.toHaveBeenCalled()
    expect(commands.checkUpdate).not.toHaveBeenCalled()
    expect(ElMessageBox.confirm).not.toHaveBeenCalled()
    expect(state.downloadAndInstall).not.toHaveBeenCalled()
  })

  it('checks once and remains silent when no newer release exists', async () => {
    vi.mocked(commands.checkUpdate).mockResolvedValue({ status: 'ok', data: null })
    const { controller } = createHarness()
    await controller.checkForUpdate(true)
    await controller.checkForUpdate(true)
    expect(commands.checkUpdate).toHaveBeenCalledExactlyOnceWith('Stable', null)
    expect(ElMessage.info).not.toHaveBeenCalled()
    expect(ElMessage.warning).not.toHaveBeenCalled()
    expect(ElMessageBox.confirm).not.toHaveBeenCalled()
    expect(controller.updateBusy.value).toBe(false)
  })

  it.each(['result', 'exception'] as const)(
    'keeps a %s failure silent and allows a manual retry',
    async (failure) => {
      if (failure === 'result') {
        vi.mocked(commands.checkUpdate).mockResolvedValueOnce({
          status: 'error',
          error: { code: 'update.timeout', message: 'Timed out', details: null },
        })
      } else {
        vi.mocked(commands.checkUpdate).mockRejectedValueOnce(new Error('Network unavailable'))
      }
      const { controller } = createHarness()
      await controller.checkForUpdate(true)
      await controller.checkForUpdate(true)
      expect(commands.checkUpdate).toHaveBeenCalledOnce()
      expect(ElMessage.warning).not.toHaveBeenCalled()
      expect(ElMessageBox.confirm).not.toHaveBeenCalled()
      expect(controller.updateBusy.value).toBe(false)
      await controller.checkForUpdate()
      expect(commands.checkUpdate).toHaveBeenCalledTimes(2)
      expect(state.downloadAndInstall).toHaveBeenCalledOnce()
    },
  )

  it('does not repeat a manual check when the startup trigger arrives afterwards', async () => {
    vi.mocked(commands.checkUpdate).mockResolvedValue({ status: 'ok', data: null })
    const { controller } = createHarness()
    await controller.checkForUpdate()
    await controller.checkForUpdate(true)
    expect(commands.checkUpdate).toHaveBeenCalledOnce()
  })

  it('shares the busy guard with manual checks while a startup request is pending', async () => {
    const check = deferred<Result<UpdateInfo | null, CommandError>>()
    vi.mocked(commands.checkUpdate).mockReturnValueOnce(check.promise)
    const { controller } = createHarness()
    const running = controller.checkForUpdate(true)
    await flushPromises()
    expect(controller.updateBusy.value).toBe(true)
    await controller.checkForUpdate()
    await controller.checkForUpdate(true)
    expect(commands.checkUpdate).toHaveBeenCalledOnce()
    check.resolve({ status: 'ok', data: release })
    await running
    expect(ElMessageBox.confirm).toHaveBeenCalledOnce()
    expect(state.downloadAndInstall).toHaveBeenCalledOnce()
  })

  it('waits until the announcement closes before requesting confirmation', async () => {
    const promptBlocked = ref(true)
    const { controller } = createHarness({ promptBlocked })
    const running = controller.checkForUpdate(true)
    await flushPromises()
    expect(commands.checkUpdate).toHaveBeenCalledOnce()
    expect(ElMessageBox.confirm).not.toHaveBeenCalled()
    expect(state.downloadAndInstall).not.toHaveBeenCalled()
    promptBlocked.value = false
    await running
    expect(ElMessageBox.confirm).toHaveBeenCalledOnce()
    expect(state.downloadAndInstall).toHaveBeenCalledOnce()
  })

  it.each(['disabled', 'channel-changed'] as const)(
    'drops a waiting startup prompt when settings are %s',
    async (change) => {
      const promptBlocked = ref(true)
      const { controller } = createHarness({ promptBlocked })
      const running = controller.checkForUpdate(true)
      await flushPromises()
      if (change === 'disabled') state.ui.askUpdate = false
      else state.ui.updateChannel = 'Beta'
      promptBlocked.value = false
      await running
      expect(ElMessageBox.confirm).not.toHaveBeenCalled()
      expect(state.downloadAndInstall).not.toHaveBeenCalled()
      expect(controller.updateBusy.value).toBe(false)
    },
  )
})

describe('shared update confirmation', () => {
  it('preserves manual feedback even when startup notifications are disabled', async () => {
    state.ui.askUpdate = false
    vi.mocked(commands.checkUpdate).mockResolvedValue({ status: 'ok', data: null })
    const { controller } = createHarness()
    await controller.checkForUpdate()
    expect(commands.checkUpdate).toHaveBeenCalledOnce()
    expect(ElMessage.info).toHaveBeenCalledWith('NoUpdatesDetected')
  })

  it('reports a failed manual check', async () => {
    vi.mocked(commands.checkUpdate).mockRejectedValueOnce(new Error('Network unavailable'))
    const { controller } = createHarness()
    await controller.checkForUpdate()
    expect(ElMessage.warning).toHaveBeenCalledWith('UpdateCheckFailed')
    expect(state.downloadAndInstall).not.toHaveBeenCalled()
    expect(controller.updateBusy.value).toBe(false)
  })

  it('keeps the approved channel and waits for the update page before downloading', async () => {
    const confirmation = deferred<'confirm'>()
    const navigation = deferred<boolean>()
    vi.mocked(ElMessageBox.confirm).mockReturnValueOnce(confirmation.promise)
    const showUpdatePage = vi.fn(() => navigation.promise)
    const { controller } = createHarness({ showUpdatePage })
    const running = controller.checkForUpdate()
    await flushPromises()
    expect(state.translate).toHaveBeenCalledWith('appUpdate.confirmBody', {
      version: release.new_version_display,
      current: '6.2.2',
      notes: release.body,
    })
    expect(ElMessageBox.confirm).toHaveBeenCalledWith(
      'appUpdate.confirmBody',
      'CheckUpdate',
      expect.objectContaining({ confirmButtonText: 'appUpdate.downloadAndRestart' }),
    )
    state.ui.updateChannel = 'Beta'
    confirmation.resolve('confirm')
    await flushPromises()
    expect(showUpdatePage).toHaveBeenCalledOnce()
    expect(state.downloadAndInstall).not.toHaveBeenCalled()
    navigation.resolve(true)
    await running
    expect(state.downloadAndInstall).toHaveBeenCalledExactlyOnceWith('Stable', release.tag_name)
  })

  it('does not navigate or download when confirmation is cancelled', async () => {
    vi.mocked(ElMessageBox.confirm).mockRejectedValueOnce('cancel')
    const showUpdatePage = vi.fn(async () => true)
    const { controller } = createHarness({ showUpdatePage })
    await controller.checkForUpdate(true)
    expect(showUpdatePage).not.toHaveBeenCalled()
    expect(state.downloadAndInstall).not.toHaveBeenCalled()
    expect(controller.updateBusy.value).toBe(false)
  })

  it('does not download when navigation to the progress page is rejected', async () => {
    const { controller } = createHarness({ showUpdatePage: async () => false })
    await controller.checkForUpdate(true)
    expect(state.downloadAndInstall).not.toHaveBeenCalled()
    expect(controller.updateBusy.value).toBe(false)
  })

  it('ignores a late check result after its owning scope is disposed', async () => {
    const check = deferred<Result<UpdateInfo | null, CommandError>>()
    vi.mocked(commands.checkUpdate).mockReturnValueOnce(check.promise)
    const { scope, controller } = createHarness()
    const running = controller.checkForUpdate(true)
    await flushPromises()
    scope.stop()
    check.resolve({ status: 'ok', data: release })
    await running
    expect(ElMessageBox.confirm).not.toHaveBeenCalled()
    expect(state.downloadAndInstall).not.toHaveBeenCalled()
  })

  it('cancels a blocked prompt when its owning scope is disposed', async () => {
    const promptBlocked = ref(true)
    const { scope, controller } = createHarness({ promptBlocked })
    const running = controller.checkForUpdate(true)
    await flushPromises()
    scope.stop()
    promptBlocked.value = false
    await running
    expect(ElMessageBox.confirm).not.toHaveBeenCalled()
    expect(state.downloadAndInstall).not.toHaveBeenCalled()
  })
})
