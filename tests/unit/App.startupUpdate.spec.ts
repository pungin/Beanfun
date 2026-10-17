import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { flushPromises, mount, type VueWrapper } from '@vue/test-utils'
import { defineComponent, h, inject, ref } from 'vue'
import App from '../../src/App.vue'
import AnnouncementModal from '../../src/components/AnnouncementModal.vue'
import { createAppUpdateController } from '../../src/composables/useAppUpdateController'

const state = vi.hoisted(() => ({
  windowLabel: 'main',
  webviewLabel: 'main',
  controllerKey: Symbol('update-controller-test'),
  createController: vi.fn(),
  checkForUpdate: vi.fn(),
  push: vi.fn(),
  route: { value: { path: '/login' } },
  config: { loaded: false, loadAll: vi.fn() },
  account: { loadAccounts: vi.fn() },
  ui: { language: 'zh-TW', askUpdate: true, applyAll: vi.fn() },
}))

vi.mock('vue-i18n', () => ({ useI18n: () => ({ t: (key: string) => key }) }))
vi.mock('vue-router', () => ({
  useRouter: () => ({ currentRoute: state.route, push: state.push }),
}))
vi.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: () => ({ label: state.windowLabel }),
}))
vi.mock('@tauri-apps/api/webview', () => ({
  getCurrentWebview: () => ({ label: state.webviewLabel }),
}))
vi.mock('element-plus', async () => {
  const { defineComponent, h } = await import('vue')
  return {
    ElConfigProvider: defineComponent({
      setup(_, { slots }) {
        return () => h('div', slots.default?.())
      },
    }),
    ElMessage: { warning: vi.fn() },
  }
})
vi.mock('../../src/components/AnnouncementModal.vue', () => ({
  default: { name: 'AnnouncementModal', emits: ['overlay-change'], render: () => null },
}))
vi.mock('../../src/windows/ClassicAccountPicker.vue', () => ({
  default: { name: 'ClassicAccountPicker', render: () => null },
}))
vi.mock('../../src/stores/config', () => ({ useConfigStore: () => state.config }))
vi.mock('../../src/stores/account', () => ({ useAccountStore: () => state.account }))
vi.mock('../../src/stores/ui', () => ({ useUiStore: () => state.ui }))
vi.mock('../../src/composables/useAppUpdateController', () => ({
  APP_UPDATE_CONTROLLER: state.controllerKey,
  createAppUpdateController: state.createController,
}))

const wrappers: VueWrapper[] = []
let injectedController: unknown

function deferred() {
  let resolve!: () => void
  const promise = new Promise<void>((complete) => {
    resolve = complete
  })
  return { promise, resolve }
}

function mountApp() {
  const RouteProbe = defineComponent({
    setup() {
      injectedController = inject(state.controllerKey)
      return () => h('div')
    },
  })
  const wrapper = mount(App, { global: { stubs: { RouterView: RouteProbe } } })
  wrappers.push(wrapper)
  return wrapper
}

function controllerOptions() {
  return state.createController.mock.calls[0]![0] as NonNullable<
    Parameters<typeof createAppUpdateController>[0]
  >
}

beforeEach(() => {
  vi.clearAllMocks()
  state.windowLabel = 'main'
  state.webviewLabel = 'main'
  state.route = ref({ path: '/login' })
  state.config.loaded = false
  state.config.loadAll.mockReset().mockImplementation(async () => {
    state.config.loaded = true
  })
  state.account.loadAccounts.mockReset().mockResolvedValue(undefined)
  state.ui.askUpdate = true
  state.ui.applyAll.mockReset()
  state.checkForUpdate.mockReset().mockResolvedValue(undefined)
  state.createController.mockReset().mockReturnValue({ checkForUpdate: state.checkForUpdate })
  state.push.mockReset().mockImplementation(async (path: string) => {
    state.route.value.path = path
  })
  injectedController = undefined
  vi.spyOn(console, 'error').mockImplementation(() => {})
  vi.spyOn(console, 'warn').mockImplementation(() => {})
})

afterEach(() => {
  for (const wrapper of wrappers.splice(0)) wrapper.unmount()
  vi.restoreAllMocks()
})

describe('startup update bootstrap', () => {
  it('waits for loaded settings, saved accounts and locale application before checking', async () => {
    const configLoad = deferred()
    const accountsLoad = deferred()
    state.config.loadAll.mockImplementation(async () => {
      await configLoad.promise
      state.config.loaded = true
    })
    state.account.loadAccounts.mockReturnValue(accountsLoad.promise)
    mountApp()
    await flushPromises()
    expect(state.account.loadAccounts).not.toHaveBeenCalled()
    expect(state.checkForUpdate).not.toHaveBeenCalled()
    configLoad.resolve()
    await flushPromises()
    expect(state.account.loadAccounts).toHaveBeenCalledOnce()
    expect(state.ui.applyAll).not.toHaveBeenCalled()
    expect(state.checkForUpdate).not.toHaveBeenCalled()
    accountsLoad.resolve()
    await flushPromises()
    expect(state.checkForUpdate).toHaveBeenCalledExactlyOnceWith(true)
    expect(state.ui.applyAll.mock.invocationCallOrder[0]).toBeLessThan(
      state.checkForUpdate.mock.invocationCallOrder[0]!,
    )
    expect(injectedController).toBe(state.createController.mock.results[0]!.value)
  })

  it.each([
    ['web-browser-123', 'web-browser-123-bar'],
    ['main', 'web-browser-123-bar'],
    ['web-browser-123', 'main'],
  ])('does not start a check in window %s with webview %s', async (windowLabel, webviewLabel) => {
    state.windowLabel = windowLabel
    state.webviewLabel = webviewLabel
    mountApp()
    await flushPromises()
    expect(state.ui.applyAll).toHaveBeenCalledOnce()
    expect(state.checkForUpdate).not.toHaveBeenCalled()
  })

  it('honours a disabled startup notification setting', async () => {
    state.ui.askUpdate = false
    mountApp()
    await flushPromises()
    expect(state.checkForUpdate).not.toHaveBeenCalled()
  })

  it('does not use default notification settings after configuration loading fails', async () => {
    state.config.loadAll.mockRejectedValueOnce(new Error('Configuration unavailable'))
    mountApp()
    await flushPromises()
    expect(state.account.loadAccounts).toHaveBeenCalledOnce()
    expect(state.ui.applyAll).toHaveBeenCalledOnce()
    expect(state.checkForUpdate).not.toHaveBeenCalled()
  })

  it('still checks when saved account loading fails after settings have loaded', async () => {
    state.account.loadAccounts.mockRejectedValueOnce(new Error('Account cache unavailable'))
    mountApp()
    await flushPromises()
    expect(state.ui.applyAll).toHaveBeenCalledOnce()
    expect(state.checkForUpdate).toHaveBeenCalledExactlyOnceWith(true)
  })

  it('does not restart the startup check when the current route changes', async () => {
    mountApp()
    await flushPromises()
    state.route.value.path = '/settings'
    await flushPromises()
    state.route.value.path = '/about'
    await flushPromises()
    expect(state.checkForUpdate).toHaveBeenCalledOnce()
  })

  it('keeps confirmation blocked until the announcement reports a clear screen', async () => {
    const wrapper = mountApp()
    const options = controllerOptions()
    expect(options.promptBlocked?.value).toBe(true)
    wrapper.getComponent(AnnouncementModal).vm.$emit('overlay-change', false)
    await flushPromises()
    expect(options.promptBlocked?.value).toBe(false)
    wrapper.getComponent(AnnouncementModal).vm.$emit('overlay-change', true)
    await flushPromises()
    expect(options.promptBlocked?.value).toBe(true)
  })
})

describe('startup update progress-page handoff', () => {
  it('uses the already-open About page without adding another navigation', async () => {
    state.route.value.path = '/about'
    mountApp()
    expect(await controllerOptions().showUpdatePage?.()).toBe(true)
    expect(state.push).not.toHaveBeenCalled()
  })

  it('opens About before allowing the update to start', async () => {
    mountApp()
    expect(await controllerOptions().showUpdatePage?.()).toBe(true)
    expect(state.push).toHaveBeenCalledExactlyOnceWith('/about')
    expect(state.route.value.path).toBe('/about')
  })

  it('rejects a failed navigation instead of starting an update without its progress page', async () => {
    state.push.mockResolvedValueOnce(new Error('Navigation cancelled'))
    mountApp()
    expect(await controllerOptions().showUpdatePage?.()).toBe(false)
    expect(state.route.value.path).toBe('/login')
  })

  it('rejects a redirect to a different page even if navigation itself succeeds', async () => {
    state.push.mockImplementationOnce(async () => {
      state.route.value.path = '/login'
    })
    mountApp()
    expect(await controllerOptions().showUpdatePage?.()).toBe(false)
  })
})
