/**
 * AccountList — Enter hotkey coverage (legacy Beanfun B6 / WPF parity).
 *
 * Locks down `handleGlobalEnter` in `src/pages/AccountList.vue`:
 *
 * | Scenario                                         | Expected       |
 * |--------------------------------------------------|----------------|
 * | Selected row + Enter                             | fires GetOtp   |
 * | No selection + Enter                             | silent no-op   |
 * | IME composition / key-repeat                     | ignored        |
 * | Focus in input                                   | ignored        |
 * | Focus inside an `.el-overlay` (ElDialog/MsgBox)  | ignored        |
 *
 * # Why this lives in a separate file from `AccountList.spec.ts`
 *
 * The SUT attaches a `window`-level `keydown` listener on mount and
 * removes it on unmount. `AccountList.spec.ts` follows the P12 spec
 * convention of NOT calling `wrapper.unmount()` between cases, so its
 * mounts' listeners accumulate across the ~50 tests in that file.
 * Dispatching `window.dispatchEvent(new KeyboardEvent(...))` inside
 * those leaked listeners would flake our assertions (`getOtp` gets
 * called N times instead of once, depending on how many prior mounts
 * happened to leave a non-null `selectedSid` in their pinia). Vitest
 * isolates each spec file in a fresh jsdom window, so hosting the
 * Enter cases here gives the listener tests a clean slate regardless
 * of what the sibling file does.
 *
 * The setup duplicates the minimal mocks from the sibling file
 * intentionally — sharing a harness would couple the two suites and
 * mask a common-mocks regression in one if the other was the only
 * caller that exercised it. See DRY vs SRP trade-off discussion in
 * the sibling file's header.
 */

import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { flushPromises, mount } from '@vue/test-utils'
import { createMemoryHistory, createRouter, type Router } from 'vue-router'
import { defineComponent, h, type Component } from 'vue'

import type {
  AccountListResult,
  CommandError,
  Result,
  ServiceAccount,
} from '../../../src/types/bindings'

vi.mock('element-plus', () => {
  const inertStub = (name: string) =>
    defineComponent({
      name,
      setup:
        (_, { slots }) =>
        () =>
          h('div', slots.default?.()),
    })
  return {
    ElButton: defineComponent({
      name: 'ElButtonStub',
      props: { type: { type: String, default: '' } },
      emits: ['click'],
      setup(_, { slots, emit }) {
        return () =>
          h(
            'button',
            { class: 'el-button-stub', onClick: (e: MouseEvent) => emit('click', e) },
            slots.default?.(),
          )
      },
    }),
    ElCheckbox: defineComponent({
      name: 'ElCheckboxStub',
      props: { modelValue: { type: Boolean, default: false } },
      emits: ['update:modelValue', 'change'],
      setup(props, { slots, emit }) {
        return () =>
          h('label', { class: 'el-checkbox-stub' }, [
            h('input', {
              type: 'checkbox',
              checked: props.modelValue,
              onChange: (e: Event) => {
                const next = (e.target as HTMLInputElement).checked
                emit('update:modelValue', next)
                emit('change', next)
              },
            }),
            h('span', slots.default?.()),
          ])
      },
    }),
    ElIcon: defineComponent({
      name: 'ElIconStub',
      setup(_, { slots }) {
        return () => h('span', { class: 'el-icon-stub' }, slots.default?.())
      },
    }),
    ElDropdown: defineComponent({
      name: 'ElDropdownStub',
      setup(_, { slots, attrs }) {
        return () =>
          h('div', { ...attrs, class: 'el-dropdown-stub' }, [
            slots.default?.(),
            h('div', { class: 'el-dropdown-stub__menu' }, slots.dropdown?.()),
          ])
      },
    }),
    ElDropdownMenu: inertStub('ElDropdownMenuStub'),
    ElDropdownItem: defineComponent({
      name: 'ElDropdownItemStub',
      emits: ['click'],
      setup(_, { slots, emit, attrs }) {
        return () =>
          h(
            'button',
            {
              ...attrs,
              class: 'el-dropdown-item-stub',
              onClick: (e: MouseEvent) => emit('click', e),
            },
            slots.default?.(),
          )
      },
    }),
    ElDialog: inertStub('ElDialogInertStub'),
    ElForm: inertStub('ElFormInertStub'),
    ElFormItem: inertStub('ElFormItemInertStub'),
    ElInput: inertStub('ElInputInertStub'),
    ElMessage: { error: vi.fn(), success: vi.fn(), warning: vi.fn(), info: vi.fn() },
    ElMessageBox: { confirm: vi.fn() },
  }
})

vi.mock('@element-plus/icons-vue', () => {
  const stub = (name: string): Component => defineComponent({ name, render: () => h('svg') })
  return {
    DocumentCopy: stub('DocumentCopyStub'),
    EditPen: stub('EditPenStub'),
    InfoFilled: stub('InfoFilledStub'),
    Key: stub('KeyStub'),
    Message: stub('MessageStub'),
    MoreFilled: stub('MoreFilledStub'),
    Operation: stub('OperationStub'),
    Plus: stub('PlusStub'),
    Refresh: stub('RefreshStub'),
    Service: stub('ServiceStub'),
    Iphone: stub('IphoneStub'),
    SwitchButton: stub('SwitchButtonStub'),
    Wallet: stub('WalletStub'),
    User: stub('UserStub'),
    VideoPlay: stub('VideoPlayStub'),
    Check: stub('CheckStub'),
    CircleClose: stub('CircleCloseStub'),
    CirclePlus: stub('CirclePlusStub'),
    CopyDocument: stub('CopyDocumentStub'),
    Document: stub('DocumentStub'),
  }
})

vi.mock('sortablejs', () => ({
  default: { create: vi.fn(() => ({ destroy: vi.fn() })) },
}))

vi.mock('../../../src/types/bindings', () => ({
  commands: {
    getAccounts: vi.fn(),
    refresh: vi.fn(),
    addServiceAccount: vi.fn(),
    changeDisplayName: vi.fn(),
    getOtp: vi.fn(),
    getEmail: vi.fn(),
    getRemainPoint: vi.fn(),
    getContract: vi.fn(),
    loadAccounts: vi.fn(),
    saveAccount: vi.fn(),
    removeAccount: vi.fn(),
    importRecords: vi.fn(),
    exportRecords: vi.fn(),
    logout: vi.fn(),
    autoPaste: vi.fn(),
    setConfig: vi.fn(),
    getAllConfig: vi.fn(),
    listGames: vi.fn(),
    setActiveService: vi.fn(),
    detectGamePath: vi.fn(),
    listGameProcesses: vi.fn(),
    killGameProcesses: vi.fn(),
    launchGame: vi.fn(),
    openUrl: vi.fn(),
    openInAppBrowser: vi.fn(),
    openMemberCenterBrowser: vi.fn(),
  },
}))

import { ElMessage } from 'element-plus'
import { commands } from '../../../src/types/bindings'
import AccountList from '../../../src/pages/AccountList.vue'
import { useAccountStore } from '../../../src/stores/account'
import { createAppI18n } from '../../../src/i18n'

const ok = <T>(data: T): Promise<Result<T, CommandError>> => Promise.resolve({ status: 'ok', data })

const SERVICE_ACCOUNT: ServiceAccount = {
  is_enable: true,
  visible: true,
  is_inherited: false,
  sid: 'sid-1',
  ssn: '00001',
  sname: 'Main Toon',
  screatetime: null,
  slastusedtime: null,
  sauthtype: null,
}

const POPULATED_LIST: AccountListResult = {
  accounts: [SERVICE_ACCOUNT],
  amount_limit_notice: { kind: 'none' },
}

const InertDialogStub = defineComponent({
  name: 'InertDialogStub',
  props: { visible: { type: Boolean, default: false } },
  render: () => h('div'),
})

function buildHarness(): {
  router: Router
  mountIt: () => Promise<ReturnType<typeof mount>>
} {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: [
      { path: '/accounts', name: 'accounts', component: AccountList },
      {
        path: '/login',
        name: 'login-region',
        component: defineComponent({ name: 'LoginStub', render: () => h('div') }),
      },
    ],
  })
  const i18n = createAppI18n()
  return {
    router,
    async mountIt() {
      await router.push('/accounts')
      await router.isReady()
      return mount(AccountList, {
        global: {
          plugins: [router, i18n],
          stubs: {
            AddServiceAccount: InertDialogStub,
            ChangeServiceAccountDisplayName: InertDialogStub,
            ServiceAccountInfo: InertDialogStub,
            CopyBox: InertDialogStub,
            GameList: InertDialogStub,
            UnconnectedGameAddAccount: InertDialogStub,
            UnconnectedGameChangePassword: InertDialogStub,
            ToolsDialogStack: InertDialogStub,
          },
        },
      })
    },
  }
}

function installClipboardMock(): { writeText: ReturnType<typeof vi.fn> } {
  const writeText = vi.fn().mockResolvedValue(undefined)
  Object.defineProperty(navigator, 'clipboard', {
    value: { writeText },
    configurable: true,
    writable: true,
  })
  return { writeText }
}

function dispatchEnter(init: Partial<KeyboardEventInit> = {}): void {
  window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true, ...init }))
}

describe('AccountList — Enter hotkey (B6 / WPF parity)', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    vi.spyOn(console, 'error').mockImplementation(() => {})
    vi.spyOn(console, 'warn').mockImplementation(() => {})
    for (const fn of Object.values(commands) as ReturnType<typeof vi.fn>[]) fn.mockReset()
    vi.mocked(ElMessage.error).mockReset()
    vi.mocked(ElMessage.success).mockReset()
    vi.mocked(ElMessage.warning).mockReset()
    vi.mocked(ElMessage.info).mockReset()
    /*
     * Mount-time IPC defaults so `setupGameOnMount` + the D11 balance
     * pre-fetch don't blow up before the case under test gets to
     * assert anything. `getAccounts` is the one callers re-seed
     * per-case because the happy-path fetches need a populated list.
     */
    vi.mocked(commands.setConfig).mockReturnValue(ok(null))
    vi.mocked(commands.getRemainPoint).mockReturnValue(ok(0))
    vi.mocked(commands.listGames).mockReturnValue(ok({ ini: {}, services: [] }))
    vi.mocked(commands.setActiveService).mockReturnValue(ok(null))
    vi.mocked(commands.detectGamePath).mockReturnValue(ok(null))
    vi.mocked(commands.listGameProcesses).mockReturnValue(ok([]))
    vi.mocked(commands.killGameProcesses).mockReturnValue(ok([]))
    vi.mocked(commands.launchGame).mockReturnValue(ok(null))
    vi.mocked(commands.openUrl).mockReturnValue(ok(null))
    installClipboardMock()
  })

  it('selected row + Enter → fires handleGetOtp via commands.getOtp', async () => {
    vi.mocked(commands.getAccounts).mockReturnValueOnce(ok(POPULATED_LIST))
    vi.mocked(commands.getOtp).mockReturnValueOnce(ok('OTP-ENTER'))

    const wrapper = await buildHarness().mountIt()
    await flushPromises()

    useAccountStore().selectedSid = 'sid-1'

    dispatchEnter()
    await flushPromises()

    expect(commands.getOtp).toHaveBeenCalledTimes(1)
    expect(commands.getOtp).toHaveBeenCalledWith(SERVICE_ACCOUNT)

    wrapper.unmount()
  })

  it('no selection → silent no-op (MsgSelectAccount NOT fired, mirrors B6 "press Enter on empty list")', async () => {
    /*
     * Divergence from the Get-OTP button click, which routes through
     * `handleGetOtp` and surfaces `MsgSelectAccount`. Enter on an
     * empty selection is deliberately silent — keyboard presses that
     * accidentally land on an idle page shouldn't pop a warning.
     */
    vi.mocked(commands.getAccounts).mockReturnValueOnce(ok(POPULATED_LIST))

    const wrapper = await buildHarness().mountIt()
    await flushPromises()

    // Clear auto-selected account to test the "no selection" guard.
    useAccountStore().selectedSid = null

    dispatchEnter()
    await flushPromises()

    expect(commands.getOtp).not.toHaveBeenCalled()
    expect(ElMessage.warning).not.toHaveBeenCalled()

    wrapper.unmount()
  })

  it('ignores IME composition and key repeat', async () => {
    /*
     * CJK IME users commit a candidate with Enter, which fires a
     * keydown with `isComposing: true`. Without this guard every
     * candidate commit inside a dialog input would kick off an OTP
     * fetch. `repeat: true` fires when the user holds Enter — the
     * OTP IPC should fire once per deliberate press, not per repeat.
     */
    vi.mocked(commands.getAccounts).mockReturnValueOnce(ok(POPULATED_LIST))

    const wrapper = await buildHarness().mountIt()
    await flushPromises()

    useAccountStore().selectedSid = 'sid-1'

    dispatchEnter({ isComposing: true })
    dispatchEnter({ repeat: true })
    await flushPromises()

    expect(commands.getOtp).not.toHaveBeenCalled()

    wrapper.unmount()
  })

  it('focus inside an <input> → skipped (lets the input own Enter)', async () => {
    vi.mocked(commands.getAccounts).mockReturnValueOnce(ok(POPULATED_LIST))

    const wrapper = await buildHarness().mountIt()
    await flushPromises()

    useAccountStore().selectedSid = 'sid-1'

    const input = document.createElement('input')
    document.body.appendChild(input)
    input.focus()
    input.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }))
    await flushPromises()

    expect(commands.getOtp).not.toHaveBeenCalled()

    document.body.removeChild(input)
    wrapper.unmount()
  })

  it('focus inside an Element Plus overlay → skipped (modal owns Enter)', async () => {
    /*
     * `ElDialog` / `ElMessageBox` render via `append-to-body` and
     * wrap their content in `.el-overlay`. When a modal is open its
     * confirm button owns Enter; routing the key to `handleGetOtp`
     * would silently kick off OTP fetches behind the modal (e.g.
     * the logout confirm dialog would double as a GetOtp trigger).
     */
    vi.mocked(commands.getAccounts).mockReturnValueOnce(ok(POPULATED_LIST))

    const wrapper = await buildHarness().mountIt()
    await flushPromises()

    useAccountStore().selectedSid = 'sid-1'

    const overlay = document.createElement('div')
    overlay.className = 'el-overlay'
    const modalBtn = document.createElement('button')
    overlay.appendChild(modalBtn)
    document.body.appendChild(overlay)
    modalBtn.focus()

    dispatchEnter()
    await flushPromises()

    expect(commands.getOtp).not.toHaveBeenCalled()

    document.body.removeChild(overlay)
    wrapper.unmount()
  })

  it('onBeforeUnmount removes the listener (no leak after the page closes)', async () => {
    /*
     * Catches the lifecycle regression where a future refactor drops
     * the `onBeforeUnmount` cleanup and lets the listener outlive
     * the component. Asserts by count: after unmount, dispatching
     * Enter with a selection set on a DIFFERENT pinia must NOT fire
     * the mock.
     */
    vi.mocked(commands.getAccounts).mockReturnValueOnce(ok(POPULATED_LIST))

    const wrapper = await buildHarness().mountIt()
    await flushPromises()

    const initialStore = useAccountStore()
    initialStore.selectedSid = 'sid-1'

    wrapper.unmount()

    /* Fresh pinia with a selection → would dispatch if the listener leaked. */
    setActivePinia(createPinia())
    useAccountStore().selectedSid = 'sid-1'
    dispatchEnter()
    await flushPromises()

    expect(commands.getOtp).not.toHaveBeenCalled()
  })
})
