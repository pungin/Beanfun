/**
 * P12.2 D1 — AccountList page behaviour.
 *
 * What this spec locks down (matches the D1 scope outlined in
 * `pages/AccountList.vue`):
 *
 * 1. Renders the four list states (`loading` → `ready` /
 *    `empty` / `error`) driven entirely by the
 *    `useAccountStore.getServiceAccounts` round-trip — no fake data,
 *    no special-case branches the user can't reach.
 * 2. Retry button on the error state re-runs the fetch and recovers
 *    when the second attempt succeeds.
 * 3. Account count badge mirrors `serviceAccounts.length`.
 * 4. Click on an enabled row arms `account.selectedSid`; click on a
 *    disabled (`is_enable === false`) row is a hard no-op (matches
 *    WPF `lstViewAccount_SelectionChanged` ignoring banned items).
 * 5. Logout button confirm → `auth.logout()` →
 *    `account.clearSessionData()` → `router.push('/login')`. Mirrors
 *    the WPF `MainWindow.Logout_Click` confirm-then-act flow plus
 *    the SPA addition of clearing the non-auth Pinia caches (the
 *    same wipe the D10 router-guard `clearAccountSession` callback
 *    fires on `auth.session_required`).
 *
 * The chrome stubs (Start Game / Change Game / Refresh Balance /
 * Member Center / Customer Service / Add Account / Get OTP / Copy
 * OTP / row context menu) are **not** asserted here — each gets its
 * own spec when its real D-step lands and the stub gets replaced.
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
  SessionInfo,
} from '../../../src/types/bindings'

const { elMessageBoxConfirm } = vi.hoisted(() => ({
  elMessageBoxConfirm: vi.fn(),
}))

vi.mock('element-plus', () => {
  /*
   * D3 note: ElDialog / ElForm / ElFormItem / ElInput are added to
   * the mock surface only because `AddServiceAccount.vue` (imported
   * by the SUT) lists them in its template. The dialog itself is
   * never rendered by AccountList (it lives behind a stubbed child
   * component — see the `AddServiceAccount` stub passed via
   * `global.stubs`), so these mocks just need to *exist* to satisfy
   * the import side-effect. The semantics of the dialog are tested
   * in `tests/unit/windows/AddServiceAccount.spec.ts` instead.
   *
   * D4 note: same reasoning extended to
   * `ChangeServiceAccountDisplayName.vue` — it is also replaced
   * with a stub via `global.stubs`. ElDropdown / ElDropdownMenu /
   * ElDropdownItem are however **rendered** by AccountList (the
   * row context menu trigger), so those stubs need to expose the
   * dropdown slot synchronously (not behind a click trigger) so
   * tests can drive the menu item without Element Plus's real
   * popper-overlay machinery.
   */
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
      setup(_, { slots, emit, attrs }) {
        return () =>
          h(
            'button',
            {
              ...attrs,
              class: 'el-button-stub',
              onClick: (e: MouseEvent) => emit('click', e),
            },
            slots.default?.(),
          )
      },
    }),
    ElCheckbox: defineComponent({
      name: 'ElCheckboxStub',
      props: { modelValue: { type: Boolean, default: false } },
      /*
       * Mirrors Element Plus's real `<el-checkbox>` event surface:
       *   - `update:modelValue` → `boolean` (for `v-model`)
       *   - `change`            → `boolean` (the new state, also payload)
       * D5 needs the component-level `change` event because
       * AccountList wires the auto-paste preference via
       * `:model-value + @change` rather than `v-model`. Without an
       * explicit `change` emit, Vue's `@change="..."` binding would
       * fall through to a native bubbled DOM `change` event whose
       * payload is the DOM `Event` object — `Boolean(Event)` is
       * always `true`, breaking the toggle-to-false branch silently.
       */
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
            h('span', { class: 'el-checkbox-stub__label' }, slots.default?.()),
          ])
      },
    }),
    ElIcon: defineComponent({
      name: 'ElIconStub',
      setup(_, { slots }) {
        return () => h('span', { class: 'el-icon-stub' }, slots.default?.())
      },
    }),
    /*
     * Render the trigger (default slot) AND the menu (`#dropdown`
     * slot) inline, so the tests don't need to fight Element
     * Plus's real popper-overlay behaviour. Real-world UX —
     * click trigger → menu pops — is verified manually / in E2E;
     * unit specs only need to assert that selecting the menu
     * item invokes the right handler.
     */
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
    /*
     * D5 added `info` (AutoPasteTip first-time toggle) on top of the
     * existing surface. Object-shape mock keeps the spec pin-pointed
     * at the toast surface AccountList actually consumes — adding
     * extra methods here is a deliberate flag for "the SUT now uses
     * a new toast variant".
     */
    ElMessage: { error: vi.fn(), success: vi.fn(), warning: vi.fn(), info: vi.fn() },
    ElMessageBox: { confirm: elMessageBoxConfirm },
  }
})

vi.mock('@element-plus/icons-vue', () => {
  const stub = (name: string): Component => defineComponent({ name, render: () => h('svg') })
  return {
    /* AccountList template icons */
    DocumentCopy: stub('DocumentCopyStub'),
    EditPen: stub('EditPenStub'),
    InfoFilled: stub('InfoFilledStub'),
    Key: stub('KeyStub'),
    Close: stub('CloseStub'),
    Grid: stub('GridStub'),
    Lock: stub('LockStub'),
    Message: stub('MessageStub'),
    Minus: stub('MinusStub'),
    Monitor: stub('MonitorStub'),
    MoreFilled: stub('MoreFilledStub'),
    Operation: stub('OperationStub'),
    Plus: stub('PlusStub'),
    Promotion: stub('PromotionStub'),
    Refresh: stub('RefreshStub'),
    Service: stub('ServiceStub'),
    Setting: stub('SettingStub'),
    Iphone: stub('IphoneStub'),
    SwitchButton: stub('SwitchButtonStub'),
    Wallet: stub('WalletStub'),
    User: stub('UserStub'),
    VideoPlay: stub('VideoPlayStub'),
    /*
     * AddServiceAccount.vue + ChangeServiceAccountDisplayName.vue
     * + ServiceAccountInfo.vue (transitively imported via the SUT)
     * pull the icons below; the dialog stubs render nothing so they
     * never paint, but the import side-effect still needs satisfying.
     */
    Check: stub('CheckStub'),
    CircleClose: stub('CircleCloseStub'),
    CirclePlus: stub('CirclePlusStub'),
    CopyDocument: stub('CopyDocumentStub'),
    Document: stub('DocumentStub'),
  }
})

/**
 * D7: mock `sortablejs`. The page uses `Sortable.create()` directly
 * on a native `<ul>` ref. We capture the `onEnd` callback passed to
 * `Sortable.create` so tests can invoke it with a synthetic
 * `{ oldIndex, newIndex }` event to exercise the drag-end handler.
 *
 * Real Sortable.js DOM behaviour (mouse-down on handle, ghost
 * element, animation timing) is intentionally out of scope for
 * unit tests — those would belong in an E2E suite.
 */
let capturedOnEnd:
  | ((event: {
      oldIndex?: number
      newIndex?: number
      item: HTMLElement
      from: HTMLElement
    }) => void)
  | null = null

vi.mock('sortablejs', () => ({
  default: {
    create: vi.fn((_el: HTMLElement, options: { onEnd?: typeof capturedOnEnd }) => {
      capturedOnEnd = options?.onEnd ?? null
      return { destroy: vi.fn() }
    }),
  },
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
    /*
     * D5 added autoPaste IPC + Config.xml read/write IPC. Mocking
     * `setConfig` / `getAllConfig` here (rather than at the
     * `useConfigStore` level) preserves the same wrapping-flow
     * everywhere in the spec — store actions still go through
     * `wrapCommand`, which still routes through these mocks.
     */
    autoPaste: vi.fn(),
    setConfig: vi.fn(),
    getAllConfig: vi.fn(),
    /*
     * D8 added the game-catalogue IPC + Start Game pipeline IPCs +
     * the new `set_active_service` mutator. Mocked here so the
     * page-level `setupGameOnMount` (which fires on every mount)
     * doesn't blow up when the SUT-under-test doesn't otherwise
     * exercise the game flow. Defaults are seeded in `beforeEach`
     * so existing D1-D11 tests keep passing without per-test wire
     * (see the "D8 mount-time pipeline" section in beforeEach).
     */
    listGames: vi.fn(),
    setActiveService: vi.fn(),
    detectGamePath: vi.fn(),
    listGameProcesses: vi.fn(),
    killGameProcesses: vi.fn(),
    closeMaplePlayWindow: vi.fn(),
    checkAndKillMaplePatcher: vi.fn(),
    launchGame: vi.fn(),
    openUrl: vi.fn(),
    /*
     * P12.4-followup-B-fix F8/F9 added the in-app browser entry
     * points used by the Customer Service / Member Center
     * buttons. `openInAppBrowser` is invoked through the
     * `useInAppBrowser` composable (Customer Service path);
     * `openMemberCenterBrowser` is invoked directly by the
     * page (Member Center path — the URL embeds the session's
     * server-side `web_token` and is built backend-side).
     */
    openInAppBrowser: vi.fn(),
    openMemberCenterBrowser: vi.fn(),
  },
}))

import { ElMessage } from 'element-plus'
import { commands } from '../../../src/types/bindings'
import AccountList from '../../../src/pages/AccountList.vue'
import { useAccountStore } from '../../../src/stores/account'
import { useAuthStore } from '../../../src/stores/auth'
import { useConfigStore } from '../../../src/stores/config'
import { useGameStore } from '../../../src/stores/game'
import { createAppI18n, i18nMessages } from '../../../src/i18n'

const ok = <T>(data: T): Promise<Result<T, CommandError>> => Promise.resolve({ status: 'ok', data })

const err = (error: CommandError): Promise<Result<never, CommandError>> =>
  Promise.resolve({ status: 'error', error })

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

const SECOND_SA: ServiceAccount = {
  ...SERVICE_ACCOUNT,
  sid: 'sid-2',
  ssn: '00002',
  sname: 'Mule Account',
}

const BANNED_SA: ServiceAccount = {
  ...SERVICE_ACCOUNT,
  sid: 'sid-3',
  ssn: '00003',
  sname: 'Suspended User',
  is_enable: false,
}

const POPULATED_LIST: AccountListResult = {
  accounts: [SERVICE_ACCOUNT, SECOND_SA, BANNED_SA],
  amount_limit_notice: { kind: 'none' },
}

const EMPTY_LIST: AccountListResult = {
  accounts: [],
  amount_limit_notice: { kind: 'none' },
}

const FAKE_SESSION: SessionInfo = {
  region: 'TW',
  account_id: 'alice',
  service_code: '610074',
  service_region: 'T9',
}

/* ---------------------------------------------------------------- */
/*  D8 — game catalogue fixtures                                     */
/* ---------------------------------------------------------------- */

/*
 * MapleStory TW — the canonical "tools-bearing" connected game
 * (`610074_T9` ∈ TOOLS_GAME_CODES). Used by the Tools-button
 * visibility tests, the Add Account connected branch, and the
 * post-OTP auto-paste `specialClick` derivation (TW MapleStory →
 * SEA pre-click sequence).
 */
const MAPLESTORY_TW: import('../../../src/types/bindings').GameService = {
  name: 'MapleStory TW',
  service_code: '610074',
  service_region: 'T9',
  website_url: 'https://maplestory.beanfun.com',
  xlarge_image_name: '610074_xlarge.jpg',
  large_image_name: '610074_large.jpg',
  small_image_name: '610074_small.jpg',
  download_url: 'https://maplestory.beanfun.com/download',
}

/*
 * KartRider Rush+ — a non-tools connected game used to assert
 * the Tools button stays hidden for codes outside the
 * `TOOLS_GAME_CODES` whitelist.
 */
const KARTRIDER_TW: import('../../../src/types/bindings').GameService = {
  name: 'KartRider TW',
  service_code: '610099',
  service_region: 'T9',
  website_url: 'https://kart.beanfun.com',
  xlarge_image_name: '610099_xlarge.jpg',
  large_image_name: '610099_large.jpg',
  small_image_name: '610099_small.jpg',
  download_url: '',
}

/*
 * "新瑪奇" (Mabinogi) — the canonical unconnected game
 * (`610153_TN` ∈ UNCONNECTED_GAME_CODES, see `stores/game.ts`).
 * Used by the Add Account / Change Password unconnected-branch
 * tests below.
 */
const MABINOGI_TN: import('../../../src/types/bindings').GameService = {
  name: 'Mabinogi',
  service_code: '610153',
  service_region: 'TN',
  website_url: 'https://mabinogi.beanfun.com',
  xlarge_image_name: '610153_xlarge.jpg',
  large_image_name: '610153_large.jpg',
  small_image_name: '610153_small.jpg',
  download_url: 'https://mabinogi.beanfun.com/download',
}

/*
 * INI fixture for MapleStory TW. `login_action_type=8` is WPF's
 * default (the "OTP first, no auto-launch" branch); D8f tests
 * override this per-case to exercise the direct-launch (`0` / `1`
 * + tradLogin) and OTP+launch (`1` + !tradLogin) chains.
 */
const MAPLESTORY_TW_INI: import('../../../src/types/bindings').GameIniEntry = {
  exe: 'C:\\Beanfun\\MapleStory.exe',
  login_action_type: '8',
  win_class_name: 'MapleStoryClass',
  dir_value_name: 'ExecPath',
  dir_reg: 'SOFTWARE\\Gamania\\MapleStory',
}

const KARTRIDER_TW_INI: import('../../../src/types/bindings').GameIniEntry = {
  exe: 'C:\\Beanfun\\KartRider.exe',
  login_action_type: '8',
  win_class_name: 'KartRiderClass',
  dir_value_name: 'ExecPath',
  dir_reg: 'SOFTWARE\\Gamania\\KartRider',
}

const MABINOGI_TN_INI: import('../../../src/types/bindings').GameIniEntry = {
  exe: 'C:\\Beanfun\\Mabinogi.exe',
  login_action_type: '8',
  win_class_name: 'MabinogiClass',
  dir_value_name: 'ExecPath',
  dir_reg: 'SOFTWARE\\Gamania\\Mabinogi',
}

/**
 * Seed the {@link useGameStore} catalogue + active selection in
 * one shot, matching the post-`setupGameOnMount` shape the page
 * sees once both the game catalogue and the saved-loginGame
 * restore have settled.
 *
 * Tests that need to bypass the mount-time picker auto-open and
 * jump straight into the "game already chosen" surface use this
 * helper — the mount path itself is exercised by the dedicated
 * D8c cases below.
 */
function seedActiveGame(
  service: import('../../../src/types/bindings').GameService,
  ini: import('../../../src/types/bindings').GameIniEntry,
): void {
  const gameStore = useGameStore()
  gameStore.services = [service]
  gameStore.ini = { [`${service.service_code}_${service.service_region}`]: ini }
  gameStore.selectedGameCode = `${service.service_code}_${service.service_region}`
  gameStore.loadState = 'loaded'
}

/**
 * Stub for the `<AddServiceAccount />` modal — keeps the AccountList
 * spec focused on the "open / close" wiring it actually owns. The
 * dialog's internal form / submit / contract-preview behaviour
 * lives in `tests/unit/windows/AddServiceAccount.spec.ts`; mixing
 * those concerns here would make this file fail for unrelated
 * reasons every time D3+ refactors land.
 */
const AddServiceAccountStub = defineComponent({
  name: 'AddServiceAccount',
  props: { visible: { type: Boolean, default: false } },
  emits: ['update:visible', 'created'],
  setup(props) {
    return () =>
      h('div', {
        class: 'add-service-account-stub',
        'data-test': 'add-service-account-stub',
        'data-visible': String(props.visible),
      })
  },
})

/**
 * Stub for the `<ChangeServiceAccountDisplayName />` modal — same
 * SRP rationale as the AddServiceAccountStub above. The page only
 * owns the "row context-menu opens dialog with the right account"
 * wiring; the dialog's form / submit / WPF parity is locked down
 * in `tests/unit/windows/ChangeServiceAccountDisplayName.spec.ts`.
 *
 * The stub forwards the `account` prop into a `data-account-sid`
 * attribute so tests can assert the correct row was wired into
 * the dialog without poking at internal refs.
 */
const ChangeServiceAccountDisplayNameStub = defineComponent({
  name: 'ChangeServiceAccountDisplayName',
  props: {
    visible: { type: Boolean, default: false },
    account: { type: Object as () => ServiceAccount | null, default: null },
  },
  emits: ['update:visible', 'updated'],
  setup(props) {
    return () =>
      h('div', {
        class: 'change-display-name-stub',
        'data-test': 'change-display-name-stub',
        'data-visible': String(props.visible),
        'data-account-sid': props.account?.sid ?? '',
      })
  },
})

/**
 * Stub for the `<ServiceAccountInfo />` modal — same SRP rationale
 * as the dialog stubs above. The page only owns the "row context-menu
 * opens dialog with the right account" wiring; the dialog's internal
 * field rendering / day-count math / null-shell handling lives in
 * `tests/unit/windows/ServiceAccountInfo.spec.ts`.
 */
const ServiceAccountInfoStub = defineComponent({
  name: 'ServiceAccountInfo',
  props: {
    visible: { type: Boolean, default: false },
    account: { type: Object as () => ServiceAccount | null, default: null },
  },
  emits: ['update:visible'],
  setup(props) {
    return () =>
      h('div', {
        class: 'service-account-info-stub',
        'data-test': 'service-account-info-stub',
        'data-visible': String(props.visible),
        'data-account-sid': props.account?.sid ?? '',
      })
  },
})

/**
 * Stub for the `<CopyBox />` dialog (D10.1). Exposes the bound
 * `title` / `value` props as `data-*` attributes so D10.5 specs
 * can assert the page wired the right WPF-locale title and the
 * right `commands.getEmail` payload before flipping `visible`.
 * The dialog's own clipboard / toast / close behaviour lives in
 * `tests/unit/windows/CopyBox.spec.ts`.
 */
const CopyBoxStub = defineComponent({
  name: 'CopyBox',
  props: {
    visible: { type: Boolean, default: false },
    title: { type: String, default: '' },
    value: { type: String, default: '' },
  },
  emits: ['update:visible'],
  setup(props) {
    return () =>
      h('div', {
        class: 'copy-box-stub',
        'data-test': 'copy-box-stub',
        'data-visible': String(props.visible),
        'data-title': props.title,
        'data-value': props.value,
      })
  },
})

/**
 * D8 — stub for the `<GameList />` picker. Same SRP rationale as
 * the dialog stubs above: the page only owns the open / select
 * wiring; the dialog's catalogue rendering / loading states /
 * card click handling lives in `tests/unit/windows/GameList.spec.ts`.
 *
 * Forwards `region` into a `data-region` attribute so D8c specs
 * can assert the page passed `auth.session.region` correctly,
 * and exposes a manual `select` button so tests can synthesise
 * the user's selection without poking at internal refs.
 */
const GameListStub = defineComponent({
  name: 'GameList',
  props: {
    visible: { type: Boolean, default: false },
    region: { type: String, default: '' },
  },
  emits: ['update:visible', 'select'],
  setup(props) {
    return () =>
      h('div', {
        class: 'game-list-stub',
        'data-test': 'game-list-stub',
        'data-visible': String(props.visible),
        'data-region': props.region,
      })
  },
})

/**
 * D8g — stub for the `<UnconnectedGameAddAccount />` dialog. Same
 * SRP rationale as the regular `AddServiceAccount` stub above —
 * the page only owns the open / created wiring, the dialog's
 * own form / submit / contract-preview behaviour is covered by
 * `tests/unit/windows/UnconnectedGame_AddAccount.spec.ts`.
 */
const UnconnectedGameAddAccountStub = defineComponent({
  name: 'UnconnectedGameAddAccount',
  props: { visible: { type: Boolean, default: false } },
  emits: ['update:visible', 'created'],
  setup(props) {
    return () =>
      h('div', {
        class: 'unconnected-add-stub',
        'data-test': 'unconnected-add-stub',
        'data-visible': String(props.visible),
      })
  },
})

/**
 * D8h — stub for the `<UnconnectedGameChangePassword />` dialog.
 * Forwards `accountIndex` so D8h specs can assert the page wired
 * the correct row index (mirrors WPF
 * `accountList.list_Account.SelectedIndex` argument).
 */
const UnconnectedGameChangePasswordStub = defineComponent({
  name: 'UnconnectedGameChangePassword',
  props: {
    visible: { type: Boolean, default: false },
    accountIndex: { type: Number, default: -1 },
  },
  emits: ['update:visible', 'verify-code-sent'],
  setup(props) {
    return () =>
      h('div', {
        class: 'unconnected-changepw-stub',
        'data-test': 'unconnected-changepw-stub',
        'data-visible': String(props.visible),
        'data-account-index': String(props.accountIndex),
      })
  },
})

/**
 * P12.5 D7 — stub for the `<ToolsDialogStack />` wrapper. The
 * page only owns the click → `openForGame(gameCode)` wiring; the
 * wrapper's own dispatch / dialog mounting / event chain is
 * locked down in `tests/unit/windows/ToolsDialogStack.spec.ts`.
 *
 * Exposes an `openForGame` Vitest mock function by returning it
 * from `setup()` (rather than via `ctx.expose()`), because the
 * AccountList template binds the wrapper as `<ToolsDialogStack
 * ref="toolsDialogRef" />` and reads `toolsDialogRef.value
 * .openForGame(...)`. With Composition API, a setup-returned
 * value is reachable from the parent's `ref` AND surfaces on
 * `wrapper.vm.openForGame` for direct assertion in tests —
 * `ctx.expose({ ... })` empirically does not surface through
 * vue-test-utils' `vm` proxy in our Vitest 4 / @vue/test-utils
 * 2 setup, so the simpler return path is the reliable choice.
 *
 * Each mount creates a fresh `vi.fn` instance (setup() runs once
 * per mount), keeping per-test state isolated without an
 * explicit reset.
 */
const ToolsDialogStackStub = defineComponent({
  name: 'ToolsDialogStack',
  setup() {
    const openForGame = vi.fn().mockResolvedValue(undefined)
    return { openForGame }
  },
  render() {
    return h('div', {
      class: 'tools-dialog-stack-stub',
      'data-test': 'tools-dialog-stack-stub',
    })
  },
})

/**
 * Standalone harness — memory-history router with the page mounted at
 * `/accounts` plus a stub `/login` so the post-logout `router.push`
 * resolves cleanly. Mirrors the per-page sandbox pattern from
 * `LoginRegionSelection.spec.ts` / `IdPassForm.spec.ts`.
 */
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
            AddServiceAccount: AddServiceAccountStub,
            ChangeServiceAccountDisplayName: ChangeServiceAccountDisplayNameStub,
            ServiceAccountInfo: ServiceAccountInfoStub,
            CopyBox: CopyBoxStub,
            GameList: GameListStub,
            UnconnectedGameAddAccount: UnconnectedGameAddAccountStub,
            UnconnectedGameChangePassword: UnconnectedGameChangePasswordStub,
            ToolsDialogStack: ToolsDialogStackStub,
          },
        },
      })
    },
  }
}

/**
 * Fresh `navigator.clipboard` mock per test. `vi.spyOn` against the
 * existing `navigator.clipboard.writeText` would only work if jsdom
 * already shipped one — empirically jsdom 23 stubs the property as
 * `undefined`, so we install a writable shim per-test and restore it
 * in `afterEach`. Using `Object.defineProperty` (rather than direct
 * assignment) is required because `navigator.clipboard` is declared
 * as a non-writable getter on the navigator prototype.
 */
function installClipboardMock(): { writeText: ReturnType<typeof vi.fn> } {
  const writeText = vi.fn().mockResolvedValue(undefined)
  Object.defineProperty(navigator, 'clipboard', {
    value: { writeText },
    configurable: true,
    writable: true,
  })
  return { writeText }
}

describe('AccountList page', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    vi.spyOn(console, 'error').mockImplementation(() => {})
    vi.spyOn(console, 'warn').mockImplementation(() => {})
    for (const fn of Object.values(commands) as ReturnType<typeof vi.fn>[]) fn.mockReset()
    elMessageBoxConfirm.mockReset()
    /*
     * Reset the toast mocks per-test so D5 cases that count
     * `info` / `success` / `warning` invocations are independent of
     * whatever fired in earlier cases. The `ElMessage` import resolves
     * to the mock object declared inside `vi.mock('element-plus')`
     * above, so the same `vi.fn` instances live for the whole module.
     */
    vi.mocked(ElMessage.error).mockReset()
    vi.mocked(ElMessage.success).mockReset()
    vi.mocked(ElMessage.warning).mockReset()
    vi.mocked(ElMessage.info).mockReset()
    /*
     * D5 store dependencies: `useConfigStore` reads via `commands.getAllConfig`
     * during `loadAll()`, but AccountList never calls `configStore.loadAll()`
     * directly (App.vue does, at boot). The store starts uninitialised here,
     * so `getOr('autoPaste', 'false')` resolves to the default and the
     * checkbox starts off. `setConfig` returns ok by default so the
     * lazy-write path doesn't toast in unrelated tests.
     */
    vi.mocked(commands.setConfig).mockReturnValue(ok(null))
    /*
     * D11: AccountList now lazy-fetches the Gash balance on mount
     * (`account.getRemainPoint()`). Default the mock to a successful
     * `0` so unrelated tests don't trip the `wrapCommand` error toast
     * path and so their `ElMessage.error` assertions remain accurate.
     * Tests that exercise the D11 surface override this with the
     * specific value they need (TW 1234 / HK 1234 / HK 0 / etc.).
     */
    vi.mocked(commands.getRemainPoint).mockReturnValue(ok(0))
    /*
     * D8 mount-time pipeline defaults:
     *
     * - `listGames` returns an empty bundle so `setupGameOnMount`
     *   succeeds (no error fallback) but finds nothing to restore.
     *   With no `loginGame` in Config.xml either, the picker is
     *   auto-opened (the stub renders a div either way) AND
     *   `loadList()` fires once — same single-fetch cadence the
     *   pre-D8 specs expected.
     * - `setActiveService` returns ok so any test that does seed a
     *   matching `loginGame` doesn't blow up on the backend swap.
     * - `detectGamePath` returns ok-with-empty so the Start Game
     *   pipeline takes the `MsgCantFindGame` branch by default
     *   (only the D8f explicit cases override this).
     * - `listGameProcesses` returns an empty array so the
     *   `MsgGameAlreadyRun` branch is opt-in.
     * - `launchGame` / `openUrl` / `killGameProcesses` all default
     *   to ok so the full Start Game chain can be exercised
     *   end-to-end without per-test re-mocking.
     */
    vi.mocked(commands.listGames).mockReturnValue(ok({ ini: {}, services: [] }))
    vi.mocked(commands.setActiveService).mockReturnValue(ok(null))
    vi.mocked(commands.detectGamePath).mockReturnValue(ok(null))
    vi.mocked(commands.listGameProcesses).mockReturnValue(ok([]))
    vi.mocked(commands.killGameProcesses).mockReturnValue(ok([]))
    vi.mocked(commands.closeMaplePlayWindow).mockReturnValue(ok(false))
    vi.mocked(commands.checkAndKillMaplePatcher).mockReturnValue(ok([]))
    vi.mocked(commands.launchGame).mockReturnValue(ok(null))
    vi.mocked(commands.openUrl).mockReturnValue(ok(null))
  })

  it('shows the loading state while getAccounts is in flight', async () => {
    /*
     * Hand-crafted pending promise — never resolves during the
     * test — so the page stays in the `loading` branch long enough
     * for the assertion. Resolved after the assert so vitest's
     * unhandled-rejection check stays quiet.
     */
    let resolveFetch!: (r: Result<AccountListResult, CommandError>) => void
    const pending = new Promise<Result<AccountListResult, CommandError>>((resolve) => {
      resolveFetch = resolve
    })
    vi.mocked(commands.getAccounts).mockReturnValueOnce(pending)

    const ctx = buildHarness()
    const wrapper = await ctx.mountIt()

    expect(wrapper.find('[data-test="account-list-loading"]').exists()).toBe(true)
    expect(wrapper.find('[data-test="account-list-rows"]').exists()).toBe(false)

    resolveFetch({ status: 'ok', data: EMPTY_LIST })
    await flushPromises()
  })

  it('renders the empty state when getAccounts returns an empty list', async () => {
    vi.mocked(commands.getAccounts).mockReturnValueOnce(ok(EMPTY_LIST))

    const ctx = buildHarness()
    const wrapper = await ctx.mountIt()
    await flushPromises()

    expect(wrapper.find('[data-test="account-list-empty"]').exists()).toBe(true)
    expect(wrapper.find('[data-test="account-list-rows"]').exists()).toBe(false)
    expect(wrapper.find('[data-test="account-list-count"]').text()).toContain('0')
  })

  it('renders the populated state with one row per service account', async () => {
    vi.mocked(commands.getAccounts).mockReturnValueOnce(ok(POPULATED_LIST))

    const ctx = buildHarness()
    const wrapper = await ctx.mountIt()
    await flushPromises()

    const rowsContainer = wrapper.find('[data-test="account-list-rows"]')
    expect(rowsContainer.exists()).toBe(true)

    const rows = wrapper.findAll('.account-list__row')
    expect(rows).toHaveLength(3)
    expect(rows[0].text()).toContain('Main Toon')
    expect(rows[1].text()).toContain('Mule Account')
    expect(rows[2].text()).toContain('Suspended User')
    /*
     * The banned row shows the localized "Disabled" copy as a
     * subtitle — proves the conditional branch fires. Enabled
     * rows show only the display name (no ID subtitle).
     */
    expect(rows[2].text()).toContain(i18nMessages['zh-TW'].accountList.statusBanned)
    expect(rows[2].classes()).toContain('account-list__row--banned')

    expect(wrapper.find('[data-test="account-list-count"]').text()).toContain('3')
  })

  it('renders the error state with retry, and recovers when the retry succeeds', async () => {
    vi.mocked(commands.getAccounts)
      .mockReturnValueOnce(err({ code: 'beanfun.transport', message: 'boom', details: null }))
      .mockReturnValueOnce(ok(POPULATED_LIST))

    const ctx = buildHarness()
    const wrapper = await ctx.mountIt()
    await flushPromises()

    expect(wrapper.find('[data-test="account-list-error"]').exists()).toBe(true)
    expect(wrapper.find('[data-test="account-list-error"]').text()).toContain('boom')

    await wrapper.get('[data-test="account-list-retry"]').trigger('click')
    await flushPromises()

    expect(wrapper.find('[data-test="account-list-error"]').exists()).toBe(false)
    expect(wrapper.find('[data-test="account-list-rows"]').exists()).toBe(true)
    expect(commands.getAccounts).toHaveBeenCalledTimes(2)
  })

  it('clicking an enabled row arms account.selectedSid', async () => {
    vi.mocked(commands.getAccounts).mockReturnValueOnce(ok(POPULATED_LIST))

    const ctx = buildHarness()
    const wrapper = await ctx.mountIt()
    await flushPromises()

    const account = useAccountStore()
    expect(account.selectedSid).toBe('sid-1')

    await wrapper.get('[data-test="account-row-sid-2"]').trigger('click')

    expect(account.selectedSid).toBe('sid-2')
    expect(wrapper.get('[data-test="account-row-sid-2"]').classes()).toContain(
      'account-list__row--selected',
    )
  })

  it('clicking a banned row is a no-op (WPF parity)', async () => {
    vi.mocked(commands.getAccounts).mockReturnValueOnce(ok(POPULATED_LIST))

    const ctx = buildHarness()
    const wrapper = await ctx.mountIt()
    await flushPromises()

    const account = useAccountStore()
    await wrapper.get('[data-test="account-row-sid-3"]').trigger('click')

    expect(account.selectedSid).toBe('sid-1')
  })

  it('double-clicking an enabled row copies sid to clipboard (WPF parity, issue #239)', async () => {
    /*
     * WPF `lstViewAccount_MouseDoubleClick` only called
     * `Clipboard.SetText(selected.sid)` — no row-arm, no OTP fetch.
     * Mirror that here: the dblclick must hit `clipboard.writeText`
     * with the row's sid and surface a generic `CopyFinished` toast,
     * without touching `account.selectedSid` or `commands.getOtp`
     * (those belong to the single-click + Enter path; auto-select
     * from PR #245 is what arms the initial selection on mount).
     */
    vi.mocked(commands.getAccounts).mockReturnValueOnce(ok(POPULATED_LIST))
    const clipboard = installClipboardMock()

    const ctx = buildHarness()
    const wrapper = await ctx.mountIt()
    await flushPromises()

    const account = useAccountStore()
    /*
     * Snapshot whatever the auto-select from PR #245 armed on
     * mount; the dblclick on sid-2 must NOT mutate it (otherwise
     * the OTP / Start Game UI would silently flip to point at the
     * just-copied row, which the WPF user never saw).
     */
    const selectedBeforeDblClick = account.selectedSid

    await wrapper.get('[data-test="account-row-sid-2"]').trigger('dblclick')
    await flushPromises()

    expect(clipboard.writeText).toHaveBeenCalledTimes(1)
    expect(clipboard.writeText).toHaveBeenCalledWith('sid-2')
    expect(ElMessage.success).toHaveBeenCalledWith(i18nMessages['zh-TW'].CopyFinished)
    expect(commands.getOtp).not.toHaveBeenCalled()
    expect(account.selectedSid).toBe(selectedBeforeDblClick)
  })

  it('double-clicking a banned row is a no-op (mirrors the single-click guard)', async () => {
    vi.mocked(commands.getAccounts).mockReturnValueOnce(ok(POPULATED_LIST))
    const clipboard = installClipboardMock()

    const ctx = buildHarness()
    const wrapper = await ctx.mountIt()
    await flushPromises()

    const account = useAccountStore()
    const selectedBeforeDblClick = account.selectedSid

    await wrapper.get('[data-test="account-row-sid-3"]').trigger('dblclick')
    await flushPromises()

    expect(clipboard.writeText).not.toHaveBeenCalled()
    expect(commands.getOtp).not.toHaveBeenCalled()
    expect(account.selectedSid).toBe(selectedBeforeDblClick)
  })

  it('logout: confirm → auth.logout → account.clearSessionData → /login', async () => {
    vi.mocked(commands.getAccounts).mockReturnValueOnce(ok(POPULATED_LIST))
    vi.mocked(commands.logout).mockReturnValueOnce(ok(null))
    elMessageBoxConfirm.mockResolvedValueOnce('confirm')

    const ctx = buildHarness()
    const wrapper = await ctx.mountIt()
    await flushPromises()

    /*
     * Seed the auth store with a session + the account store with
     * a primed cache so the post-logout wipe is observable. The
     * spec asserts both wipes (auth + account) fire, in addition
     * to the navigation, so a regression that drops either side
     * of the orchestration trips a red test.
     */
    const auth = useAuthStore()
    auth.session = FAKE_SESSION
    const account = useAccountStore()
    account.selectedSid = 'sid-1'

    await wrapper.get('[data-test="account-list-logout"]').trigger('click')
    await flushPromises()

    expect(elMessageBoxConfirm).toHaveBeenCalledTimes(1)
    expect(commands.logout).toHaveBeenCalledTimes(1)
    expect(auth.session).toBeNull()
    expect(account.serviceAccounts).toEqual([])
    expect(account.selectedSid).toBeNull()
    expect(ctx.router.currentRoute.value.path).toBe('/login')
  })

  it('Add Service Account button toggles the modal visibility (D3 wiring)', async () => {
    /*
     * D3 replaced the previous `makeStub('Add Service Account')`
     * console.warn with a real modal opener. This spec locks down
     * the surface AccountList owns: the button click flips the
     * `addAccountVisible` ref, which propagates to the
     * `<AddServiceAccount v-model:visible>` binding. The dialog's
     * own form / submit / contract preview behaviour is covered by
     * `tests/unit/windows/AddServiceAccount.spec.ts`.
     *
     * D8g extended the button handler to branch between connected
     * and unconnected games on `game.isUnconnectedGame`. The
     * connected-game branch is the original D3 surface — seed
     * MapleStory TW (a connected game) so the click takes the
     * `addAccountVisible` flip rather than the new
     * `unconnectedAddVisible` flip (the latter has its own D8g
     * case below).
     */
    vi.mocked(commands.getAccounts).mockReturnValueOnce(ok(EMPTY_LIST))

    const ctx = buildHarness()
    seedActiveGame(MAPLESTORY_TW, MAPLESTORY_TW_INI)
    const wrapper = await ctx.mountIt()
    await flushPromises()

    const stub = wrapper.get('[data-test="add-service-account-stub"]')
    expect(stub.attributes('data-visible')).toBe('false')

    await wrapper.get('[data-test="account-list-add"]').trigger('click')
    await flushPromises()

    expect(stub.attributes('data-visible')).toBe('true')

    /*
     * Closing via the dialog's `update:visible(false)` (i.e. the
     * Cancel button or the success path) flips it back. We
     * synthesise that emit on the stub directly because the page
     * is the consumer being tested here, not the dialog itself.
     */
    const stubComponent = wrapper.findComponent(AddServiceAccountStub)
    stubComponent.vm.$emit('update:visible', false)
    await flushPromises()

    expect(stub.attributes('data-visible')).toBe('false')
  })

  it('row context menu Change Alias → opens dialog with the row account (D4 wiring)', async () => {
    /*
     * D4 replaced the previous `makeStub('Row context menu')`
     * console.warn with a real `<el-dropdown>` popover holding a
     * `Change Alias` item. This spec locks down the surface
     * AccountList owns: clicking the menu item flips
     * `changeAliasVisible` AND threads the row's account into
     * `changeAliasTarget`, both of which propagate into the
     * `<ChangeServiceAccountDisplayName>` props.
     *
     * The dialog's own form / submit / WPF parity behaviour is
     * covered by `tests/unit/windows/ChangeServiceAccountDisplayName.spec.ts`.
     */
    vi.mocked(commands.getAccounts).mockReturnValueOnce(ok(POPULATED_LIST))

    const ctx = buildHarness()
    const wrapper = await ctx.mountIt()
    await flushPromises()

    const stub = wrapper.get('[data-test="change-display-name-stub"]')
    expect(stub.attributes('data-visible')).toBe('false')
    expect(stub.attributes('data-account-sid')).toBe('')

    /*
     * Click Change Alias on row 2 (the Mule Account row). The
     * dropdown stub renders the `#dropdown` slot inline so we
     * can hit the menu item directly without simulating the
     * trigger click first.
     */
    await wrapper.get('[data-test="account-row-change-alias-sid-2"]').trigger('click')
    await flushPromises()

    expect(stub.attributes('data-visible')).toBe('true')
    expect(stub.attributes('data-account-sid')).toBe('sid-2')

    /*
     * Closing the dialog (cancel / submit / Esc → emits
     * update:visible(false)) must clear both the visibility
     * flag AND the target snapshot — leaking the previous
     * account ref into the next open would let the user rename
     * the wrong row.
     */
    const dialog = wrapper.findComponent(ChangeServiceAccountDisplayNameStub)
    dialog.vm.$emit('update:visible', false)
    await flushPromises()

    expect(stub.attributes('data-visible')).toBe('false')
    expect(stub.attributes('data-account-sid')).toBe('')
  })

  it('row context menu Change Alias does not arm selectedSid (no row-select side effect)', async () => {
    /*
     * The `more_vert` button + dropdown menu live inside the row
     * `<li>`. Without `@click.stop` on both the trigger and the
     * menu item, a click would bubble up and trigger
     * `selectRow(a)` as a side effect — silently arming the
     * wrong account for OTP / Start Game on top of opening the
     * rename dialog. This spec pins down that the menu item is
     * a clean affordance.
     */
    vi.mocked(commands.getAccounts).mockReturnValueOnce(ok(POPULATED_LIST))

    const ctx = buildHarness()
    const wrapper = await ctx.mountIt()
    await flushPromises()

    const account = useAccountStore()
    expect(account.selectedSid).toBe('sid-1')

    await wrapper.get('[data-test="account-row-change-alias-sid-2"]').trigger('click')
    await flushPromises()

    expect(account.selectedSid).toBe('sid-1')
  })

  it('row context menu Account Info → opens dialog with the row account (D6 wiring)', async () => {
    /*
     * D6 added a second `<el-dropdown-item>` to the per-row menu:
     * Account Info opens the `<ServiceAccountInfo>` read-only
     * modal pre-loaded with the row's account. This spec locks
     * down what AccountList owns: clicking the menu item flips
     * `accountInfoVisible` AND threads the row's account into
     * `accountInfoTarget`, both of which propagate as props.
     *
     * The dialog's own field rendering / day-count math / null-
     * shell handling are covered by
     * `tests/unit/windows/ServiceAccountInfo.spec.ts`.
     */
    vi.mocked(commands.getAccounts).mockReturnValueOnce(ok(POPULATED_LIST))

    const ctx = buildHarness()
    const wrapper = await ctx.mountIt()
    await flushPromises()

    const stub = wrapper.get('[data-test="service-account-info-stub"]')
    expect(stub.attributes('data-visible')).toBe('false')
    expect(stub.attributes('data-account-sid')).toBe('')

    /* Click Account Info on row 3 (the banned `Suspended User` row). */
    await wrapper.get('[data-test="account-row-info-sid-3"]').trigger('click')
    await flushPromises()

    expect(stub.attributes('data-visible')).toBe('true')
    expect(stub.attributes('data-account-sid')).toBe('sid-3')

    /*
     * Closing the dialog (cancel / Esc → emits update:visible(false))
     * must clear both the visibility flag AND the target snapshot —
     * leaking the previous account ref into the next open would
     * surface stale data on the next Account Info click.
     */
    const dialog = wrapper.findComponent(ServiceAccountInfoStub)
    dialog.vm.$emit('update:visible', false)
    await flushPromises()

    expect(stub.attributes('data-visible')).toBe('false')
    expect(stub.attributes('data-account-sid')).toBe('')
  })

  it('row context menu Account Info does not arm selectedSid (no row-select side effect)', async () => {
    /*
     * Same `@click.stop` invariant as the Change Alias menu item:
     * opening the read-only info dialog must not silently swap
     * the selected account that drives OTP / Start Game.
     */
    vi.mocked(commands.getAccounts).mockReturnValueOnce(ok(POPULATED_LIST))

    const ctx = buildHarness()
    const wrapper = await ctx.mountIt()
    await flushPromises()

    const account = useAccountStore()
    expect(account.selectedSid).toBe('sid-1')

    await wrapper.get('[data-test="account-row-info-sid-2"]').trigger('click')
    await flushPromises()

    expect(account.selectedSid).toBe('sid-1')
  })

  /* ---------------------------------------------------------------- */
  /*  D10.5 — row context-menu Get Email + Tools button stub split    */
  /* ---------------------------------------------------------------- */

  it('row context menu Get Email: commands.getEmail ok → CopyBox opens with AuthEmail title + email payload', async () => {
    /*
     * 1:1 mirror of WPF `m_GetEmail_Click`
     * (`Beanfun/Pages/AccountList.xaml.cs` L204-209):
     *
     *   new CopyBox(
     *     TryFindResource("AuthEmail") as string,
     *     App.MainWnd.bfClient.getEmail()
     *   ).ShowDialog();
     *
     * The IPC payload becomes the dialog's `value`; the WPF
     * locale key `AuthEmail` becomes the dialog's `title`. The
     * page wraps the call in `wrapCommand` so the standard
     * session-expired hook + `errors.{code}` toast pipeline
     * apply uniformly with every other IPC in this file.
     */
    vi.mocked(commands.getAccounts).mockReturnValueOnce(ok(POPULATED_LIST))
    vi.mocked(commands.getEmail).mockReturnValueOnce(ok('verified@beanfun.example'))

    const ctx = buildHarness()
    const wrapper = await ctx.mountIt()
    await flushPromises()

    const stub = wrapper.get('[data-test="copy-box-stub"]')
    expect(stub.attributes('data-visible')).toBe('false')
    expect(stub.attributes('data-value')).toBe('')

    await wrapper.get('[data-test="account-row-get-email-sid-2"]').trigger('click')
    await flushPromises()

    expect(commands.getEmail).toHaveBeenCalledTimes(1)
    expect(stub.attributes('data-visible')).toBe('true')
    expect(stub.attributes('data-value')).toBe('verified@beanfun.example')
    /*
     * Title resolves through the i18n harness — assert the
     * rendered translated value is the WPF-locale `AuthEmail`
     * resource (zh-TW: 「認證信箱」). Using the locale fixture
     * directly keeps the assertion stable if the WPF locale ever
     * gets re-translated.
     */
    expect(stub.attributes('data-title')).toBe(i18nMessages['zh-TW'].AuthEmail)
    /* No error toast on the happy path. */
    expect(ElMessage.error).not.toHaveBeenCalled()
  })

  it('row context menu Get Email: commands.getEmail error → wrapCommand toasts + CopyBox stays closed', async () => {
    /*
     * Match WPF behaviour: if the IPC throws, no CopyBox is
     * shown. `wrapCommand` already produces the user-visible
     * error toast + console log; the page must not flip
     * `copyBoxVisible` so the user is not staring at an empty
     * dialog while the toast describes the cause.
     */
    vi.mocked(commands.getAccounts).mockReturnValueOnce(ok(POPULATED_LIST))
    vi.mocked(commands.getEmail).mockReturnValueOnce(
      err({ code: 'beanfun.email_unavailable', message: 'no email on file' }),
    )

    const ctx = buildHarness()
    const wrapper = await ctx.mountIt()
    await flushPromises()

    await wrapper.get('[data-test="account-row-get-email-sid-1"]').trigger('click')
    await flushPromises()

    expect(commands.getEmail).toHaveBeenCalledTimes(1)
    /* wrapCommand fired its toast pipeline. */
    expect(ElMessage.error).toHaveBeenCalled()
    /* Dialog must NOT have opened — assert visibility + payload stay pristine. */
    const stub = wrapper.get('[data-test="copy-box-stub"]')
    expect(stub.attributes('data-visible')).toBe('false')
    expect(stub.attributes('data-value')).toBe('')
    expect(stub.attributes('data-title')).toBe('')
  })

  it('P12.5 D7: Tools button click → ToolsDialogStack.openForGame(selectedGameCode)', async () => {
    /*
     * WPF parity guard: WPF `btn_Tools_Click`
     * (`AccountList.xaml.cs` L237-249) is a per-game tools
     * window launcher — it switches on `gameCode` and opens
     * MapleTools / KartTools accordingly. The SPA centralises
     * that dispatch in `windows/ToolsDialogStack.vue` and the
     * page just calls `toolsDialogRef.value?.openForGame(gameCode)`.
     *
     * This test pins:
     * 1. The click is wired to the Tools-specific handler (not
     *    the Change Game stub — which would never call
     *    `openForGame`).
     * 2. The handler passes the LIVE `selectedGameCode` from the
     *    store, not a hard-coded literal (a future regression that
     *    accidentally hard-codes `'610074_T9'` would silently
     *    break MapleStory M / KartRider routing).
     *
     * The wrapper's own dispatch correctness lives in
     * `tests/unit/windows/ToolsDialogStack.spec.ts`.
     */
    vi.mocked(commands.getAccounts).mockReturnValueOnce(ok(POPULATED_LIST))

    const ctx = buildHarness()
    seedActiveGame(MAPLESTORY_TW, MAPLESTORY_TW_INI)
    const wrapper = await ctx.mountIt()
    await flushPromises()

    const stackStub = wrapper.findComponent(ToolsDialogStackStub)
    expect(stackStub.exists()).toBe(true)
    const openForGame = (stackStub.vm as unknown as { openForGame: ReturnType<typeof vi.fn> })
      .openForGame
    expect(openForGame).not.toHaveBeenCalled()

    await wrapper.get('[data-test="account-list-tools"]').trigger('click')
    await flushPromises()

    expect(openForGame).toHaveBeenCalledTimes(1)
    expect(openForGame).toHaveBeenCalledWith('610074_T9')
  })

  it('logout: dismissing the confirm dialog is a hard cancel', async () => {
    vi.mocked(commands.getAccounts).mockReturnValueOnce(ok(POPULATED_LIST))
    /*
     * `ElMessageBox.confirm` rejects with `'cancel'` (or `'close'`)
     * when the user dismisses; the page must treat a rejection as
     * "user said no" and skip the entire post-confirm flow.
     */
    elMessageBoxConfirm.mockRejectedValueOnce('cancel')

    const ctx = buildHarness()
    const wrapper = await ctx.mountIt()
    await flushPromises()

    const auth = useAuthStore()
    auth.session = FAKE_SESSION

    await wrapper.get('[data-test="account-list-logout"]').trigger('click')
    await flushPromises()

    expect(commands.logout).not.toHaveBeenCalled()
    expect(auth.session).toEqual(FAKE_SESSION)
    expect(ctx.router.currentRoute.value.path).toBe('/accounts')
  })

  /* ---------------------------------------------------------------- */
  /*  D5 — Get OTP / clipboard / auto-paste flow                       */
  /* ---------------------------------------------------------------- */

  /*
   * Each D5 case exercises one branch of the WPF
   * `getOtpWorker_RunWorkerCompleted` decision tree (plus the SPA
   * additions documented in the D5 decision table). Cases are
   * deliberately granular — one branch per `it` block — so a
   * regression in any single branch fails one obviously-named test
   * rather than a fat catch-all.
   */

  it('D5: clicking Get OTP without a row selection shows MsgSelectAccount and skips the IPC', async () => {
    /*
     * Mirrors WPF `btnGetOtp_Click` L84-87: with `SelectedIndex < 0`
     * the worker never starts and the user sees a warning. The SPA
     * uses `ElMessage.warning` instead of MessageBox, but the
     * effect — block the IPC, surface a guidance message — is
     * identical.
     */
    vi.mocked(commands.getAccounts).mockReturnValueOnce(ok(POPULATED_LIST))
    installClipboardMock()

    const ctx = buildHarness()
    const wrapper = await ctx.mountIt()
    await flushPromises()

    /* Auto-select picks the first enabled row; clear it to test the
     * "no selection" guard. */
    const account = useAccountStore()
    account.selectedSid = null

    await wrapper.get('[data-test="account-list-otp-get"]').trigger('click')
    await flushPromises()

    expect(commands.getOtp).not.toHaveBeenCalled()
    expect(ElMessage.warning).toHaveBeenCalledTimes(1)
    /*
     * Use the canonical zh-TW string to verify the right key fired —
     * loose `toHaveBeenCalled()` would let a future refactor that
     * swaps the key sneak past unnoticed.
     */
    expect(ElMessage.warning).toHaveBeenCalledWith(i18nMessages['zh-TW'].MsgSelectAccount)
  })

  it('D5: Get OTP with auto-paste OFF copies to clipboard and surfaces GetOtpSuccessAndCopy', async () => {
    /*
     * WPF L2169-2174 path A: window-not-found OR autoPaste unchecked
     * → `Clipboard.SetText(otp)` + MessageBox `GetOtpSuccessAndCopy`.
     * Here we reach the same code path via the autoPaste-off branch
     * (the window-not-found branch is exercised in the next case).
     */
    vi.mocked(commands.getAccounts).mockReturnValueOnce(ok(POPULATED_LIST))
    vi.mocked(commands.getOtp).mockReturnValueOnce(ok('OTP-123'))
    const clipboard = installClipboardMock()

    const ctx = buildHarness()
    const wrapper = await ctx.mountIt()
    await flushPromises()

    /* Arm a row, leave autoPaste off (the default). */
    const account = useAccountStore()
    account.selectedSid = 'sid-2'

    await wrapper.get('[data-test="account-list-otp-get"]').trigger('click')
    await flushPromises()

    /* OTP IPC fired with the snapshot of the selected row. */
    expect(commands.getOtp).toHaveBeenCalledTimes(1)
    expect(commands.getOtp).toHaveBeenCalledWith(SECOND_SA)

    /* Auto-paste IPC was NOT fired (preference is off). */
    expect(commands.autoPaste).not.toHaveBeenCalled()

    /* OTP visible + copied to clipboard + WPF success toast. */
    expect(wrapper.find('[data-test="account-list-otp-field"]').attributes('value')).toBe('OTP-123')
    expect(clipboard.writeText).toHaveBeenCalledTimes(1)
    expect(clipboard.writeText).toHaveBeenCalledWith('OTP-123')
    expect(ElMessage.success).toHaveBeenCalledTimes(1)
    expect(ElMessage.success).toHaveBeenCalledWith(i18nMessages['zh-TW'].GetOtpSuccessAndCopy)
  })

  it('D5: Get OTP with auto-paste ON delegates to commands.autoPaste and stays silent on success', async () => {
    /*
     * WPF L2178-2237 path C (the happy path): autoPaste is checked
     * AND the launcher window is present → PostString the credentials
     * silently, no MsgBox. We assert (a) the IPC fires with the right
     * shape including `specialClick` for TW MapleStory (610074/T9),
     * (b) clipboard is NOT touched (the OTP went straight into the
     * game), (c) zero toasts (silent on success — see the D5 Q4
     * decision table).
     */
    vi.mocked(commands.getAccounts).mockReturnValueOnce(ok(POPULATED_LIST))
    vi.mocked(commands.getOtp).mockReturnValueOnce(ok('OTP-XYZ'))
    vi.mocked(commands.autoPaste).mockReturnValueOnce(ok(null))
    const clipboard = installClipboardMock()

    const ctx = buildHarness()
    const wrapper = await ctx.mountIt()
    await flushPromises()

    /*
     * Seed the auth session (the page reads `service_code` /
     * `service_region` to compute `specialClick`) and arm a row.
     */
    useAuthStore().session = FAKE_SESSION
    useAccountStore().selectedSid = 'sid-1'

    /* Toggle autoPaste on via the user-gesture path (mirrors the real flow). */
    await wrapper.get('[data-test="account-list-auto-paste"] input').setValue(true)
    await flushPromises()
    /*
     * The toggle path also fires the AutoPasteTip on the first-ever
     * toggle — clear those toast mocks so the post-OTP assertions
     * count only the OTP-flow toasts.
     */
    vi.mocked(ElMessage.info).mockClear()
    vi.mocked(ElMessage.success).mockClear()

    await wrapper.get('[data-test="account-list-otp-get"]').trigger('click')
    await flushPromises()

    expect(commands.autoPaste).toHaveBeenCalledTimes(1)
    expect(commands.autoPaste).toHaveBeenCalledWith({
      className: 'MapleStoryClass',
      account: 'sid-1',
      password: 'OTP-XYZ',
      /* TW MapleStory → SEA pre-click sequence enabled. */
      specialClick: true,
    })
    /* Clipboard not touched — the OTP went directly into the game. */
    expect(clipboard.writeText).not.toHaveBeenCalled()
    /* Silent on success — mirrors WPF L2235-2237. */
    expect(ElMessage.success).not.toHaveBeenCalled()
    expect(ElMessage.error).not.toHaveBeenCalled()
  })

  it('D5: Get OTP with auto-paste ON falls back to clipboard when the launcher window is missing', async () => {
    /*
     * WPF L2169-2174 path B: autoPaste is checked BUT
     * `FindWindow(win_class_name) == IntPtr.Zero` → fall back to
     * Clipboard + MessageBox `GetOtpSuccessAndCopy`. The Rust port
     * surfaces the missing window as `process.window_not_found`,
     * which we branch on inside `handleGetOtp` to reach the same
     * fallback. `wrapCommand` must NOT toast on this branch — the
     * outcome is, from the user's POV, a successful flow.
     */
    vi.mocked(commands.getAccounts).mockReturnValueOnce(ok(POPULATED_LIST))
    vi.mocked(commands.getOtp).mockReturnValueOnce(ok('OTP-FALLBACK'))
    vi.mocked(commands.autoPaste).mockReturnValueOnce(
      err({ code: 'process.window_not_found', message: 'no launcher window', details: null }),
    )
    const clipboard = installClipboardMock()

    const ctx = buildHarness()
    const wrapper = await ctx.mountIt()
    await flushPromises()

    useAuthStore().session = FAKE_SESSION
    useAccountStore().selectedSid = 'sid-1'

    await wrapper.get('[data-test="account-list-auto-paste"] input').setValue(true)
    await flushPromises()
    vi.mocked(ElMessage.info).mockClear()
    vi.mocked(ElMessage.success).mockClear()

    await wrapper.get('[data-test="account-list-otp-get"]').trigger('click')
    await flushPromises()

    expect(commands.autoPaste).toHaveBeenCalledTimes(1)
    /* Fallback fired: clipboard write + WPF success toast. */
    expect(clipboard.writeText).toHaveBeenCalledTimes(1)
    expect(clipboard.writeText).toHaveBeenCalledWith('OTP-FALLBACK')
    expect(ElMessage.success).toHaveBeenCalledTimes(1)
    expect(ElMessage.success).toHaveBeenCalledWith(i18nMessages['zh-TW'].GetOtpSuccessAndCopy)
    /*
     * window_not_found is treated as a successful fallback, not a
     * user-visible error — no error toast should fire.
     */
    expect(ElMessage.error).not.toHaveBeenCalled()
  })

  it('D5: changing the selected row clears the OTP value (no cross-row OTP leak)', async () => {
    /*
     * The OTP is bound to whichever row was selected at fetch time —
     * leaving it visible after the user picks a different row would
     * be a misbinding hazard (the user might paste row A's OTP
     * thinking it belonged to row B). The watch on `selectedSid`
     * resets `otpValue` on every selection change.
     */
    vi.mocked(commands.getAccounts).mockReturnValueOnce(ok(POPULATED_LIST))
    vi.mocked(commands.getOtp).mockReturnValueOnce(ok('OTP-RESET-ME'))
    installClipboardMock()

    const ctx = buildHarness()
    const wrapper = await ctx.mountIt()
    await flushPromises()

    const account = useAccountStore()
    account.selectedSid = 'sid-1'

    await wrapper.get('[data-test="account-list-otp-get"]').trigger('click')
    await flushPromises()
    expect(wrapper.find('[data-test="account-list-otp-field"]').attributes('value')).toBe(
      'OTP-RESET-ME',
    )

    /* Switch to a different enabled row → OTP must clear. */
    await wrapper.get('[data-test="account-row-sid-2"]').trigger('click')
    await flushPromises()

    expect(wrapper.find('[data-test="account-list-otp-field"]').attributes('value')).toBe('')
  })

  it("#300: switching sub-accounts restores each account's cached OTP", async () => {
    /*
     * Issue #300 feature request: an OTP fetched for one sub-account
     * must persist when the user toggles to another account and back,
     * instead of blanking. We fetch for sid-1, hop to sid-2 (no cached
     * OTP → blank), then return to sid-1 and expect the original OTP
     * to reappear — with NO second `getOtp` IPC (it came from cache).
     */
    vi.mocked(commands.getAccounts).mockReturnValueOnce(ok(POPULATED_LIST))
    vi.mocked(commands.getOtp).mockReturnValueOnce(ok('OTP-CACHED-1'))
    installClipboardMock()

    const ctx = buildHarness()
    const wrapper = await ctx.mountIt()
    await flushPromises()

    const account = useAccountStore()
    account.selectedSid = 'sid-1'

    await wrapper.get('[data-test="account-list-otp-get"]').trigger('click')
    await flushPromises()
    expect(wrapper.find('[data-test="account-list-otp-field"]').attributes('value')).toBe(
      'OTP-CACHED-1',
    )

    /* Hop to sid-2 — no cached OTP for it yet → field blanks. */
    await wrapper.get('[data-test="account-row-sid-2"]').trigger('click')
    await flushPromises()
    expect(wrapper.find('[data-test="account-list-otp-field"]').attributes('value')).toBe('')

    /* Back to sid-1 — cached OTP restored without a fresh fetch. */
    await wrapper.get('[data-test="account-row-sid-1"]').trigger('click')
    await flushPromises()
    expect(wrapper.find('[data-test="account-list-otp-field"]').attributes('value')).toBe(
      'OTP-CACHED-1',
    )
    expect(commands.getOtp).toHaveBeenCalledTimes(1)
  })

  it('#300: double-clicking the OTP field copies it and surfaces GetOtpSuccessAndCopy', async () => {
    /*
     * Issue #300 regression: up to 5.9.2 double-clicking the generated
     * OTP auto-copied it. Restore that gesture — dblclick on the field
     * must hit `clipboard.writeText` with the current OTP and toast
     * the WPF success string (the double-click has no persistent
     * affordance, so explicit feedback is what confirms the copy).
     */
    vi.mocked(commands.getAccounts).mockReturnValueOnce(ok(POPULATED_LIST))
    vi.mocked(commands.getOtp).mockReturnValueOnce(ok('OTP-DBL'))
    const clipboard = installClipboardMock()

    const ctx = buildHarness()
    const wrapper = await ctx.mountIt()
    await flushPromises()

    useAccountStore().selectedSid = 'sid-1'

    await wrapper.get('[data-test="account-list-otp-get"]').trigger('click')
    await flushPromises()
    /* Clear the copy + toast from the Get OTP (auto-paste-off) flow so
     * the assertions below count only the double-click's effects. */
    clipboard.writeText.mockClear()
    vi.mocked(ElMessage.success).mockClear()

    await wrapper.get('[data-test="account-list-otp-field"]').trigger('dblclick')
    await flushPromises()

    expect(clipboard.writeText).toHaveBeenCalledTimes(1)
    expect(clipboard.writeText).toHaveBeenCalledWith('OTP-DBL')
    expect(ElMessage.success).toHaveBeenCalledTimes(1)
    expect(ElMessage.success).toHaveBeenCalledWith(i18nMessages['zh-TW'].GetOtpSuccessAndCopy)
  })

  it('D5: first auto-paste toggle shows AutoPasteTip + persists; subsequent toggles persist silently', async () => {
    /*
     * Mirrors WPF L73-79 `autoPaste_CheckedChanged`: the very first
     * toggle (when `GetValue("autoPaste", "")` returns "" — i.e. the
     * key has never been written) shows the AutoPasteTip MessageBox
     * before persisting. We use `ElMessage.info` instead of MessageBox
     * for the SPA equivalent, but the branch — first toggle ⇒ tip ⇒
     * always persist — is identical.
     */
    vi.mocked(commands.getAccounts).mockReturnValueOnce(ok(EMPTY_LIST))
    installClipboardMock()

    const ctx = buildHarness()
    const wrapper = await ctx.mountIt()
    await flushPromises()

    const configStore = useConfigStore()
    expect(configStore.get('autoPaste')).toBeUndefined()

    /* First toggle — should fire the tip + persist `'true'`. */
    await wrapper.get('[data-test="account-list-auto-paste"] input').setValue(true)
    await flushPromises()

    expect(ElMessage.info).toHaveBeenCalledTimes(1)
    expect(ElMessage.info).toHaveBeenCalledWith(
      expect.objectContaining({
        message: i18nMessages['zh-TW'].accountList.autoPasteTip,
      }),
    )
    expect(commands.setConfig).toHaveBeenCalledTimes(1)
    expect(commands.setConfig).toHaveBeenCalledWith('autoPaste', 'true')
    expect(configStore.get('autoPaste')).toBe('true')

    /* Second toggle — no tip, just persist. */
    await wrapper.get('[data-test="account-list-auto-paste"] input').setValue(false)
    await flushPromises()

    expect(ElMessage.info).toHaveBeenCalledTimes(1) /* unchanged */
    expect(commands.setConfig).toHaveBeenCalledTimes(2)
    expect(commands.setConfig).toHaveBeenLastCalledWith('autoPaste', 'false')
    expect(configStore.get('autoPaste')).toBe('false')
  })

  /*
   * Enter-hotkey coverage lives in its own spec file
   * (`AccountList.EnterHotkey.spec.ts`) because the window-level
   * `keydown` listener that mirrors WPF's "select row + press Enter
   * → Get OTP" behaviour interacts with jsdom's shared `window`
   * across test cases — the mounts in this file intentionally
   * never call `unmount()` (see the D1 stylistic convention above),
   * so their listeners accumulate and would flake the Enter assertions
   * here. Vitest's per-file jsdom isolation gives the Enter tests a
   * fresh `window`, keeping each hotkey guard deterministic.
   */

  /* ---------------------------------------------------------------- */
  /*  D7 — drag-and-drop reorder + per-game persistence                */
  /* ---------------------------------------------------------------- */

  /*
   * Each D7 case exercises one branch of the WPF "Drag and Drop
   * Reorder" region (`AccountList.xaml.cs` L257-451) +
   * `SaveAccountOrder` (L477-487) + `ApplyAccountOrder` (L489-531)
   * + the `BeanfunClient.Account.cs` L137-139 call site that runs
   * ApplyAccountOrder right after the server-side `OrderBy(ssn)`.
   *
   * Sortable.js DOM mechanics (mouse-down on handle, ghost element,
   * animation) are intentionally out of scope here — they belong in
   * an E2E suite. The unit boundary is the page's @end handler:
   * the @end handler receives `{ oldIndex, newIndex }` from the
   * event, splices the Pinia array in place, then runs its
   * (a) canonical store-action funnel and (b) silent persist.
   */

  it('D7: applies the saved per-game order to the server-sorted list on load', async () => {
    /*
     * Mirrors WPF `BeanfunClient.Account.cs` L137-139 — right after
     * `GetAccounts` finishes the `OrderBy(ssn)` sort, the page-level
     * `ApplyAccountOrder` reshuffles the list to the user's saved
     * preference. Saved CSV `"sid-2,sid-1"` reorders the first two
     * rows; sid-3 (banned) isn't mentioned and must append to the
     * tail (the WPF L522-526 invariant).
     */
    vi.mocked(commands.getAccounts).mockReturnValueOnce(ok(POPULATED_LIST))

    const ctx = buildHarness()
    /*
     * Seed BEFORE mount so that loadList's
     * `applyServiceAccountOrderFromSavedCsv` call inside the SUT
     * reads the saved order on the very first paint.
     *
     * Pinia is created fresh in `beforeEach` via `createPinia` +
     * `setActivePinia`, so calling `useAuthStore()` / `useConfigStore()`
     * here resolves to the same instances the SUT will see — no
     * extra wiring needed.
     */
    useAuthStore().session = FAKE_SESSION
    useConfigStore().entries['AccountOrder_610074_T9'] = 'sid-2,sid-1'

    const wrapper = await ctx.mountIt()
    await flushPromises()

    /* Saved order applied: sid-2 first, sid-1 second, sid-3 (unmentioned) appended. */
    const account = useAccountStore()
    expect(account.serviceAccounts.map((a) => a.sid)).toEqual(['sid-2', 'sid-1', 'sid-3'])

    /*
     * Visible row order matches the store — proves the apply
     * happened before the ready-state paint, not after.
     */
    const rowSids = wrapper.findAll('.account-list__row').map((row) => row.attributes('data-test'))
    expect(rowSids).toEqual(['account-row-sid-2', 'account-row-sid-1', 'account-row-sid-3'])
  })

  it('D7: drag end → setServiceAccountOrder + persist commands.setConfig under per-game key', async () => {
    /*
     * Mirrors WPF `Drop` handler (L420-451) + `SaveAccountOrder`
     * (L477-487): once the user releases the drag, the new sid CSV
     * gets persisted under `AccountOrder_<service_code>_<service_region>`.
     *
     * The page's @end handler receives `{ oldIndex, newIndex }`
     * and splices the Pinia array in place before persisting.
     */
    vi.mocked(commands.getAccounts).mockReturnValueOnce(ok(structuredClone(POPULATED_LIST)))

    const ctx = buildHarness()
    useAuthStore().session = FAKE_SESSION
    await ctx.mountIt()
    await flushPromises()

    const account = useAccountStore()

    /*
     * Spy on the store action AFTER the page mounted (so the
     * page captured the un-spied reference at setup time, but
     * the spy still intercepts because Pinia setup-store actions
     * are looked up on the proxy at call time, not bound at setup).
     */
    const setOrderSpy = vi.spyOn(account, 'setServiceAccountOrder')

    /*
     * Simulate dragging item at index 0 (SERVICE_ACCOUNT / sid-1)
     * to index 2. The handler splices the Pinia array in place,
     * producing [sid-2, sid-3, sid-1].
     */
    expect(capturedOnEnd).not.toBeNull()
    const fakeFrom = document.createElement('ul')
    for (let i = 0; i < 3; i++) fakeFrom.appendChild(document.createElement('li'))
    capturedOnEnd!({
      oldIndex: 0,
      newIndex: 2,
      item: fakeFrom.children[0] as HTMLElement,
      from: fakeFrom,
    })
    await flushPromises()

    /*
     * (a) Store action invoked with the post-drag sid list — the
     *     canonical funnel for any future invariants attached to
     *     setServiceAccountOrder (analytics, dedup, etc.).
     */
    expect(setOrderSpy).toHaveBeenCalledTimes(1)
    expect(setOrderSpy).toHaveBeenCalledWith(['sid-2', 'sid-3', 'sid-1'])

    /*
     * (b) Persist write fired under the per-game key with the
     *     CSV-joined sids — mirrors WPF L485-486
     *     `ConfigAppSettings.SetValue("AccountOrder_" + gameCode, csv)`.
     */
    expect(commands.setConfig).toHaveBeenCalledWith('AccountOrder_610074_T9', 'sid-2,sid-3,sid-1')

    /*
     * Cache stays in sync with the disk write so the next load
     * (without a refresh round-trip) sees the same order. Mirrors
     * the inline `entries.value[key] = csv` in `persistAccountOrder`.
     */
    const configStore = useConfigStore()
    expect(configStore.get('AccountOrder_610074_T9')).toBe('sid-2,sid-3,sid-1')
  })

  it('D7: persist failure on drag end is silent (no toast, mirrors WPF SetValue)', async () => {
    /*
     * Mirrors WPF L482-487 `ConfigAppSettings.SetValue` which
     * swallows IO errors silently. A toast on every transient
     * Config.xml write failure mid-drag would be jarring; the
     * next refresh / drag will reconcile, and the operation is
     * non-destructive (only ordering is at stake, not account
     * data). The SPA equivalent uses `safeInvoke` directly
     * (bypassing `configStore.set` → `wrapCommand` → toast) so
     * the user sees nothing.
     */
    vi.mocked(commands.getAccounts).mockReturnValueOnce(ok(structuredClone(POPULATED_LIST)))
    vi.mocked(commands.setConfig).mockReturnValueOnce(
      err({ code: 'config.write_failed', message: 'disk full', details: null }),
    )

    const ctx = buildHarness()
    useAuthStore().session = FAKE_SESSION
    await ctx.mountIt()
    await flushPromises()

    /*
     * Simulate dragging item at index 1 (SECOND_SA / sid-2) to
     * index 0. The handler splices the array to [sid-2, sid-1, sid-3].
     */
    expect(capturedOnEnd).not.toBeNull()
    const fakeFrom = document.createElement('ul')
    for (let i = 0; i < 3; i++) fakeFrom.appendChild(document.createElement('li'))
    capturedOnEnd!({
      oldIndex: 1,
      newIndex: 0,
      item: fakeFrom.children[1] as HTMLElement,
      from: fakeFrom,
    })
    await flushPromises()

    /* The IPC fired (we asked the backend to persist). */
    expect(commands.setConfig).toHaveBeenCalledWith('AccountOrder_610074_T9', 'sid-2,sid-1,sid-3')
    /* But the failure stayed silent — no error toast surfaced. */
    expect(ElMessage.error).not.toHaveBeenCalled()
    /*
     * The local order is intact (the page does not revert on
     * persist failure — the WPF behaviour is identical, the user
     * keeps the drag result and the next refresh reconciles).
     */
    const account = useAccountStore()
    expect(account.serviceAccounts.map((a) => a.sid)).toEqual(['sid-2', 'sid-1', 'sid-3'])
    /*
     * Cache was NOT mutated since the IPC failed — mirrors the
     * `if (result.ok)` guard in `persistAccountOrder`. This pins
     * down that on retry, the cached value still reflects disk
     * (here: undefined / never-written), avoiding a stale-cache
     * trap if the user reorders again before refreshing.
     */
    const configStore = useConfigStore()
    expect(configStore.get('AccountOrder_610074_T9')).toBeUndefined()
  })

  /* ---------------------------------------------------------------- */
  /*  D11 — Gash balance value + refresh                              */
  /* ---------------------------------------------------------------- */

  /*
   * D11 cases pin down the WPF parity for the Gash balance card:
   *
   * - Initial mount auto-fetches once (matches WPF login flow which
   *   pre-populates `bfClient.remainPoint`).
   * - Display format mirrors `MainWindow.updateRemainPoint`
   *   (L1716-1721) including the HK in-game suffix branch and the
   *   `remainPoint == 0` carve-out that suppresses the suffix.
   * - Refresh button forces an IPC re-round-trip (matches WPF
   *   `m_UpdatePoint_Click` L137-140 which never caches).
   * - Mount-fetch failures are silently swallowed (the standard
   *   `wrapCommand` toast still fires; the page itself does not
   *   surface an inline banner — the visible refresh button is the
   *   recovery affordance).
   *
   * Each case overrides the per-test `commands.getRemainPoint` mock
   * to the specific value/status it needs; the `beforeEach` default
   * `ok(0)` only applies to non-D11 tests that inadvertently trigger
   * the mount auto-fetch.
   */

  it('D11: TW session — mount auto-fetches and renders "樂豆: 1234 點" (no in-game suffix)', async () => {
    /*
     * WPF parity: TW branch in `updateRemainPoint` (L1716-1721)
     * skips the `GashRemainInGame` suffix entirely (the ternary
     * collapses to empty string). Format key reads:
     *   GashRemain.format("1234") → "樂豆: 1234 點"
     * which is the entire visible header text in WPF.
     */
    vi.mocked(commands.getAccounts).mockReturnValueOnce(ok(POPULATED_LIST))
    vi.mocked(commands.getRemainPoint).mockReturnValueOnce(ok(1234))

    const ctx = buildHarness()
    useAuthStore().session = { ...FAKE_SESSION, region: 'TW' }
    const wrapper = await ctx.mountIt()
    await flushPromises()

    expect(commands.getRemainPoint).toHaveBeenCalledTimes(1)
    expect(wrapper.get('[data-test="account-list-balance-value"]').text()).toBe('樂豆: 1234 點')
  })

  it('D11: HK session, positive balance — appends "(遊戲內 floor(value/2.5))" suffix', async () => {
    /*
     * WPF L1716-1721:
     *   remainPoint=1234, region=HK → suffix = GashRemainInGame.format(floor(1234/2.5)) = " (遊戲內 493)"
     * Outer GashRemain wraps the concatenation:
     *   "樂豆: 1234 (遊戲內 493) 點"
     * The inner space before "(" is part of the `GashRemainInGame`
     * locale string itself (" (遊戲內 {0})") — assert verbatim so
     * a future locale-tree edit that drops the space would fail.
     */
    vi.mocked(commands.getAccounts).mockReturnValueOnce(ok(POPULATED_LIST))
    vi.mocked(commands.getRemainPoint).mockReturnValueOnce(ok(1234))

    const ctx = buildHarness()
    useAuthStore().session = { ...FAKE_SESSION, region: 'HK' }
    const wrapper = await ctx.mountIt()
    await flushPromises()

    expect(commands.getRemainPoint).toHaveBeenCalledTimes(1)
    expect(wrapper.get('[data-test="account-list-balance-value"]').text()).toBe(
      '樂豆: 1234 (遊戲內 493) 點',
    )
  })

  it('D11: HK session, zero balance — suppresses in-game suffix (matches WPF carve-out)', async () => {
    /*
     * WPF L1716-1721 carve-out: `remainPoint == 0` short-circuits
     * the in-game suffix even on HK. This avoids showing "(遊戲內
     * 0)" which is meaningless. The SPA mirrors the same guard.
     */
    vi.mocked(commands.getAccounts).mockReturnValueOnce(ok(POPULATED_LIST))
    vi.mocked(commands.getRemainPoint).mockReturnValueOnce(ok(0))

    const ctx = buildHarness()
    useAuthStore().session = { ...FAKE_SESSION, region: 'HK' }
    const wrapper = await ctx.mountIt()
    await flushPromises()

    expect(wrapper.get('[data-test="account-list-balance-value"]').text()).toBe('樂豆: 0 點')
  })

  it('D11: refresh button click forces a second IPC round-trip and locks the button while in flight', async () => {
    /*
     * WPF `m_UpdatePoint_Click` L137-140 calls
     * `bfClient.getRemainPoint()` directly every click — no cache
     * check. We assert the SPA mirrors that by passing
     * `force = true` to the store wrapper, which always re-IPCs.
     *
     * Also pins the in-flight UX (disabled + spinning class) so a
     * future regression that drops the loading state — leaving the
     * user unable to tell whether their click did anything — fails
     * loudly.
     */
    vi.mocked(commands.getAccounts).mockReturnValueOnce(ok(POPULATED_LIST))
    /* Mount fetch returns 100; refresh fetch returns 200. */
    vi.mocked(commands.getRemainPoint).mockReturnValueOnce(ok(100))

    const ctx = buildHarness()
    useAuthStore().session = { ...FAKE_SESSION, region: 'TW' }
    const wrapper = await ctx.mountIt()
    await flushPromises()

    expect(commands.getRemainPoint).toHaveBeenCalledTimes(1)
    expect(wrapper.get('[data-test="account-list-balance-value"]').text()).toBe('樂豆: 100 點')

    /* Hand-crafted pending Promise so we can observe the in-flight state. */
    let resolveRefresh!: (r: Result<number, CommandError>) => void
    const pending = new Promise<Result<number, CommandError>>((resolve) => {
      resolveRefresh = resolve
    })
    vi.mocked(commands.getRemainPoint).mockReturnValueOnce(pending)

    const button = wrapper.get('[data-test="account-list-refresh-balance"]')
    await button.trigger('click')
    /*
     * After the click but before the IPC resolves: button is
     * disabled (no double-click foot-gun) and the spinning class
     * is applied to the icon container.
     */
    expect((button.element as HTMLButtonElement).disabled).toBe(true)
    expect(button.classes()).toContain('account-list__balance-refresh--spinning')

    resolveRefresh({ status: 'ok', data: 200 })
    await flushPromises()

    expect(commands.getRemainPoint).toHaveBeenCalledTimes(2)
    expect((button.element as HTMLButtonElement).disabled).toBe(false)
    expect(button.classes()).not.toContain('account-list__balance-refresh--spinning')
    expect(wrapper.get('[data-test="account-list-balance-value"]').text()).toBe('樂豆: 200 點')
  })

  it('D11: mount auto-fetch failure leaves the placeholder visible without crashing the page', async () => {
    /*
     * The page must not propagate a balance-fetch failure to the
     * page-level error banner — that banner is reserved for
     * `loadList()` failures that prevent the account list from
     * rendering. `wrapCommand` (inside `account.getRemainPoint`)
     * still toasts the error so the user knows something went
     * wrong, but the visible state collapses to the WPF parity
     * placeholder ("—") since `account.remainPoint` stays null.
     *
     * The visible refresh button remains the recovery affordance —
     * a follow-up click can succeed independently.
     */
    vi.mocked(commands.getAccounts).mockReturnValueOnce(ok(POPULATED_LIST))
    vi.mocked(commands.getRemainPoint).mockReturnValueOnce(
      err({ code: 'beanfun.balance_unavailable', message: 'flaky upstream' }),
    )

    const ctx = buildHarness()
    useAuthStore().session = { ...FAKE_SESSION, region: 'TW' }
    const wrapper = await ctx.mountIt()
    await flushPromises()

    expect(commands.getRemainPoint).toHaveBeenCalledTimes(1)
    /* `wrapCommand` fired its toast pipeline. */
    expect(ElMessage.error).toHaveBeenCalled()
    /* Account list still rendered (page-level state is `ready`). */
    expect(wrapper.find('[data-test="account-list-rows"]').exists()).toBe(true)
    /* Balance display collapsed to the WPF-parity placeholder. */
    expect(wrapper.get('[data-test="account-list-balance-value"]').text()).toBe('—')
  })

  /* ---------------------------------------------------------------- */
  /*  D8c — mount-time game setup pipeline                             */
  /* ---------------------------------------------------------------- */

  /*
   * D8c locks down the mount-time `setupGameOnMount` orchestration —
   * load catalogue → restore `loginGame` from Config.xml → either
   * delegate to `selectActiveGame` (saved value valid) or auto-open
   * the picker + paint the default-session account list.
   *
   * The four cases below cover every branch of the flow chart in
   * `setupGameOnMount`'s docblock (`pages/AccountList.vue`):
   *
   * 1. Catalogue load fails              → fall through to loadList
   * 2. No saved loginGame                → loadList + open picker
   * 3. Saved loginGame matches session   → selectActiveGame skips
   *                                        setActiveService IPC
   * 4. Saved loginGame differs from sess → selectActiveGame fires
   *                                        setActiveService + clears
   *                                        stale account selection
   */

  it('D8c: catalogue load fails → falls through to loadList with default session (no picker)', async () => {
    /*
     * Mirrors `setupGameOnMount` step 2: when `game.loadGames()`
     * leaves the store in `loadState === 'error'`, the page bails
     * out to the default-session `loadList()` so the user still
     * sees the account list (with the previously-selected game
     * the backend session is pinned to). The picker stays closed
     * because surfacing it on top of an error state would be a UX
     * regression — `gameStore.loadGames` already toasted the
     * structured cause.
     */
    vi.mocked(commands.listGames).mockReturnValueOnce(
      err({ code: 'beanfun.transport', message: 'catalogue down', details: null }),
    )
    vi.mocked(commands.getAccounts).mockReturnValueOnce(ok(POPULATED_LIST))

    const ctx = buildHarness()
    useAuthStore().session = FAKE_SESSION
    const wrapper = await ctx.mountIt()
    await flushPromises()

    expect(commands.listGames).toHaveBeenCalledTimes(1)
    /* loadList ran with the default session — account list rendered. */
    expect(commands.getAccounts).toHaveBeenCalledTimes(1)
    expect(wrapper.find('[data-test="account-list-rows"]').exists()).toBe(true)
    /* Picker stayed closed (the GameList stub mounts behind v-if="auth.session"
     * — when present, its `data-visible` attribute reads "false"). */
    const picker = wrapper.find('[data-test="game-list-stub"]')
    expect(picker.exists()).toBe(true)
    expect(picker.attributes('data-visible')).toBe('false')
    /* setActiveService was NOT called — error path doesn't reach selectActiveGame. */
    expect(commands.setActiveService).not.toHaveBeenCalled()
  })

  it('D8c: no saved loginGame → opens picker AND fires loadList in parallel', async () => {
    /*
     * Mirrors the "step 5" branch — `loginGame` config key absent,
     * so the page can't restore a previous selection. WPF would
     * fall through to the default item; the SPA equivalent shows
     * the picker so the user can pick explicitly while still
     * painting the default-session account list behind it (no
     * blank screen during the picker decision).
     */
    vi.mocked(commands.listGames).mockReturnValueOnce(
      ok({
        ini: { '610074_T9': MAPLESTORY_TW_INI },
        services: [MAPLESTORY_TW],
      }),
    )
    vi.mocked(commands.getAccounts).mockReturnValueOnce(ok(POPULATED_LIST))

    const ctx = buildHarness()
    useAuthStore().session = FAKE_SESSION
    /* Config has NO `loginGame` entry — the default state. */
    const wrapper = await ctx.mountIt()
    await flushPromises()

    /* Catalogue loaded + account list painted. */
    expect(commands.listGames).toHaveBeenCalledTimes(1)
    expect(commands.getAccounts).toHaveBeenCalledTimes(1)
    /* Picker auto-opened. */
    const picker = wrapper.get('[data-test="game-list-stub"]')
    expect(picker.attributes('data-visible')).toBe('true')
    /* GameList received `region` prop from `auth.session.region`. */
    expect(picker.attributes('data-region')).toBe('TW')
    /* setActiveService NOT called — no saved value to restore. */
    expect(commands.setActiveService).not.toHaveBeenCalled()
  })

  it('D8c: saved loginGame matches session → loads accounts WITHOUT setActiveService IPC', async () => {
    /*
     * Cold-mount optimisation guard. When the saved `loginGame`
     * resolves to the same `(service_code, service_region)` pair
     * the backend session is already pinned to (the common case
     * for a returning user who logged into the same game), the
     * `setActiveService` IPC is a wasted round-trip. The page's
     * `sameAsSession` branch in `selectActiveGame` skips the IPC
     * but still runs `loadList` so the first paint always shows
     * data (or the empty state).
     */
    vi.mocked(commands.listGames).mockReturnValueOnce(
      ok({
        ini: { '610074_T9': MAPLESTORY_TW_INI },
        services: [MAPLESTORY_TW],
      }),
    )
    vi.mocked(commands.getAccounts).mockReturnValueOnce(ok(POPULATED_LIST))

    const ctx = buildHarness()
    /* Session pinned to the same gameCode the saved value points at. */
    useAuthStore().session = { ...FAKE_SESSION, service_code: '610074', service_region: 'T9' }
    useConfigStore().entries['loginGame'] = '610074_T9'

    const wrapper = await ctx.mountIt()
    await flushPromises()

    expect(commands.listGames).toHaveBeenCalledTimes(1)
    /* No backend swap needed. */
    expect(commands.setActiveService).not.toHaveBeenCalled()
    /* Account list still loaded (selectActiveGame's `refresh: true`). */
    expect(commands.getAccounts).toHaveBeenCalledTimes(1)
    expect(wrapper.find('[data-test="account-list-rows"]').exists()).toBe(true)
    /* Picker stays closed — the saved value resolved cleanly. */
    expect(wrapper.get('[data-test="game-list-stub"]').attributes('data-visible')).toBe('false')
  })

  it('D8c: saved loginGame differs from session → fires setActiveService + clears stale selection + reloads', async () => {
    /*
     * The "user switched games last session" path. Saved
     * `loginGame=610099_T9` (KartRider) but the session is still
     * pinned to MapleStory's `610074_T9` (the post-login default).
     * `selectActiveGame` must:
     *   1. Persist `loginGame` (idempotent here).
     *   2. Call `setActiveService` so the backend session matches
     *      the saved selection (the next account-list IPC needs
     *      the right pair).
     *   3. Clear any stale `account.selectedSid`.
     *   4. Run `loadList` so the account list reflects the new
     *      game.
     */
    vi.mocked(commands.listGames).mockReturnValueOnce(
      ok({
        ini: {
          '610074_T9': MAPLESTORY_TW_INI,
          '610099_T9': KARTRIDER_TW_INI,
        },
        services: [MAPLESTORY_TW, KARTRIDER_TW],
      }),
    )
    vi.mocked(commands.getAccounts).mockReturnValueOnce(ok(POPULATED_LIST))

    const ctx = buildHarness()
    useAuthStore().session = { ...FAKE_SESSION, service_code: '610074', service_region: 'T9' }
    useConfigStore().entries['loginGame'] = '610099_T9'
    /* Stale selection from the previous session. */
    useAccountStore().selectedSid = 'sid-stale'

    const wrapper = await ctx.mountIt()
    await flushPromises()

    /* (a) Backend session updated to the saved selection. */
    expect(commands.setActiveService).toHaveBeenCalledTimes(1)
    expect(commands.setActiveService).toHaveBeenCalledWith('610099', 'T9')
    /* (b) Account list reloaded for the new game. */
    expect(commands.getAccounts).toHaveBeenCalledTimes(1)
    /* (c) Selection auto-picks first enabled account after reload. */
    expect(useAccountStore().selectedSid).toBe('sid-1')
    /* (d) Game switch reflected in the gameStore. */
    expect(useGameStore().selectedGameCode).toBe('610099_T9')
    /* Picker stays closed — the saved value resolved cleanly. */
    expect(wrapper.get('[data-test="game-list-stub"]').attributes('data-visible')).toBe('false')
  })

  /* ---------------------------------------------------------------- */
  /*  Mount fast path — no re-fetch when returning from Settings/About */
  /* ---------------------------------------------------------------- */

  /**
   * Regression for the user-reported bug: every visit to Settings
   * (or About) triggered an account-list re-fetch on return,
   * making custom drag-sort orders appear to "reset" briefly
   * before being re-applied from `Config.xml`.
   *
   * The fast-path predicate in `setupGameOnMount` is "store
   * already has accounts AND session is non-null". The two tests
   * below pin both halves:
   *
   * 1. Predicate holds (store seeded, session live) → the page
   *    must NOT fire `listGames`, `setActiveService`, or
   *    `getAccounts` on mount.
   * 2. Predicate fails (store empty) → the full D8c bootstrap
   *    runs (covered by the D8c specs above; this case re-asserts
   *    the existing baseline so a future skip-by-default
   *    regression would fail loudly here too).
   */
  it('skips re-fetch on remount when serviceAccounts already cached for current session', async () => {
    /*
     * Seed BEFORE mount: this is the "navigate back from
     * Settings/About" shape — Pinia stores survive the route
     * change, AccountList is the component that gets unmounted
     * + remounted, and the cache predicate is what protects the
     * user from the spinner-flash + sort-reset UX bug.
     */
    const auth = useAuthStore()
    auth.session = FAKE_SESSION
    const accountStore = useAccountStore()
    accountStore.serviceAccounts = POPULATED_LIST.accounts
    /*
     * Selected sid survives the round-trip too — this is the
     * "user picked an account, opened Settings, came back"
     * flow where the OTP button must remain primed.
     */
    accountStore.selectedSid = POPULATED_LIST.accounts[0]!.sid

    const ctx = buildHarness()
    const wrapper = await ctx.mountIt()
    await flushPromises()

    /* Fast path took effect: no IPC calls fired from setupGameOnMount. */
    expect(commands.listGames).not.toHaveBeenCalled()
    expect(commands.getAccounts).not.toHaveBeenCalled()
    expect(commands.setActiveService).not.toHaveBeenCalled()

    /* Account list still rendered (loadState flipped to 'ready' on the skip). */
    expect(wrapper.find('[data-test="account-list-rows"]').exists()).toBe(true)
    /* Selection preserved across the remount. */
    expect(useAccountStore().selectedSid).toBe(POPULATED_LIST.accounts[0]!.sid)
  })

  it('does not fast-path cached accounts when selected game differs from session', async () => {
    /*
     * Regression for issue #274: the UI could retain the previous
     * selected game (e.g. Mabinogi icon/name) while the authenticated
     * backend session and cached account list still belonged to the
     * login-default game (e.g. MapleStory). The old remount fast-path
     * only checked "accounts exist", skipped `setActiveService`, and
     * left users looking at MapleStory accounts under a Mabinogi
     * header until they manually switched away and back.
     */
    vi.mocked(commands.listGames).mockReturnValueOnce(
      ok({
        ini: {
          '610074_T9': MAPLESTORY_TW_INI,
          '610153_TN': MABINOGI_TN_INI,
        },
        services: [MAPLESTORY_TW, MABINOGI_TN],
      }),
    )
    vi.mocked(commands.getAccounts).mockReturnValueOnce(ok(POPULATED_LIST))

    useAuthStore().session = { ...FAKE_SESSION, service_code: '610074', service_region: 'T9' }
    useConfigStore().entries['loginGame'] = '610153_TN'

    const gameStore = useGameStore()
    gameStore.selectedGameCode = '610153_TN'

    const accountStore = useAccountStore()
    accountStore.serviceAccounts = POPULATED_LIST.accounts
    accountStore.selectedSid = 'sid-stale'

    const ctx = buildHarness()
    await ctx.mountIt()
    await flushPromises()

    expect(commands.listGames).toHaveBeenCalledTimes(1)
    expect(commands.setActiveService).toHaveBeenCalledTimes(1)
    expect(commands.setActiveService).toHaveBeenCalledWith('610153', 'TN')
    expect(commands.getAccounts).toHaveBeenCalledTimes(1)
    expect(useAuthStore().session?.service_code).toBe('610153')
    expect(useAuthStore().session?.service_region).toBe('TN')
    expect(useGameStore().selectedGameCode).toBe('610153_TN')
    expect(useAccountStore().selectedSid).toBe('sid-1')
  })

  it('still re-fetches on remount when serviceAccounts cache is empty (cold mount baseline)', async () => {
    /*
     * Sanity-check the negative branch: empty store + live session
     * must continue to run the full D8c bootstrap so the very
     * first paint after login still works. If somebody ever
     * widens the skip predicate to "session non-null only" this
     * baseline would catch it.
     */
    vi.mocked(commands.getAccounts).mockReturnValueOnce(ok(POPULATED_LIST))
    useAuthStore().session = FAKE_SESSION

    const ctx = buildHarness()
    await ctx.mountIt()
    await flushPromises()

    expect(commands.listGames).toHaveBeenCalledTimes(1)
    expect(commands.getAccounts).toHaveBeenCalledTimes(1)
  })

  /* ---------------------------------------------------------------- */
  /*  P12.4 followup-A D5 — lastSelectedIni persistence on selection   */
  /* ---------------------------------------------------------------- */

  /**
   * P12.4 followup-A D5 — `selectActiveGame` must persist a
   * JSON snapshot of the active `GameIniEntry` to Config.xml
   * under `lastSelectedIni` so the LoginPage `GameStart` button
   * (`useGameLauncher` → `game.restoreLastSelected(config)`)
   * can re-launch the same game after a logout that empties the
   * in-memory game store.
   *
   * WPF parity rationale: `MainWindow.runGame` reads the launch
   * subset (`game_exe` / `dir_value_name` / `dir_reg` /
   * `win_class_name`) from the live `MainWindow` instance, which
   * survives logout because it lives on the singleton window.
   * Pinia's `clearGameData` (called on `auth.logout`) wipes the
   * SPA equivalent, so we mirror WPF lifetime by parking the
   * snapshot in Config.xml — the only persistence layer that
   * spans both the authenticated and unauthenticated halves of
   * the SPA.
   *
   * Asserts the call is paired with the existing `loginGame`
   * write (gameCode → loginGame, INI → lastSelectedIni) so a
   * future refactor can't silently drop one half of the pair
   * without taking the test with it.
   */
  it('D5: selectActiveGame persists lastSelectedIni JSON alongside loginGame', async () => {
    vi.mocked(commands.listGames).mockReturnValueOnce(
      ok({
        ini: { '610074_T9': MAPLESTORY_TW_INI },
        services: [MAPLESTORY_TW],
      }),
    )
    vi.mocked(commands.getAccounts).mockReturnValueOnce(ok(POPULATED_LIST))
    vi.mocked(commands.setConfig).mockReturnValue(ok(null))

    const ctx = buildHarness()
    useAuthStore().session = { ...FAKE_SESSION, service_code: '610074', service_region: 'T9' }
    /*
     * Saved loginGame matches the session → cold-mount path runs
     * `selectActiveGame('610074', 'T9', true)` via
     * `setupGameOnMount`'s "saved value valid" branch (D8c
     * "matches session" specs above lock down that orchestration).
     * That call is what should fire the two setConfig writes
     * we're asserting on here.
     */
    useConfigStore().entries['loginGame'] = '610074_T9'

    await ctx.mountIt()
    await flushPromises()

    expect(commands.setConfig).toHaveBeenCalledWith('loginGame', '610074_T9')
    expect(commands.setConfig).toHaveBeenCalledWith(
      'lastSelectedIni',
      JSON.stringify(MAPLESTORY_TW_INI),
    )
  })

  /* ---------------------------------------------------------------- */
  /*  D8d — game info bar real name + image                            */
  /* ---------------------------------------------------------------- */

  it('D8d: game info bar shows the active game name + region-aware banner image', async () => {
    /*
     * Mirrors WPF `MainWindow.gameName.Content = selectedGame.name`
     * (L662) plus the `gameImage.Source = imageBase + small_image_name`
     * binding. The SPA derives both via the `gameNameDisplay` /
     * `gameImageUrl` computeds — assert the rendered DOM matches
     * the gameStore selection rather than the placeholder
     * (`accountList.gamePlaceholder`).
     */
    vi.mocked(commands.getAccounts).mockReturnValueOnce(ok(POPULATED_LIST))

    const ctx = buildHarness()
    useAuthStore().session = FAKE_SESSION
    seedActiveGame(MAPLESTORY_TW, MAPLESTORY_TW_INI)
    const wrapper = await ctx.mountIt()
    await flushPromises()

    expect(wrapper.get('[data-test="account-list-game-name"]').text()).toBe('MapleStory TW')

    const img = wrapper.get('[data-test="account-list-game-image"]')
    /*
     * TW base URL + small_image_name — must match `imageUrl` exactly
     * so a future protocol / base-URL change in `stores/game.ts`
     * surfaces here as a red test rather than silently 404'ing in
     * the WebView.
     */
    expect(img.attributes('src')).toBe('https://images.beanfun.com/GameZone/610074_small.jpg')
    expect(img.attributes('alt')).toBe('MapleStory TW')
  })

  it('D8d: no selected game → falls back to placeholder name + generic icon (no <img>)', async () => {
    /*
     * Cold-mount surface: gameStore is empty, the page renders the
     * `accountList.gamePlaceholder` copy and the generic VideoPlay
     * glyph instead of an `<img>`. Lock this so a regression that
     * accidentally renders an empty-src `<img>` (which would
     * 404-spam the WebView console) trips the test.
     */
    vi.mocked(commands.getAccounts).mockReturnValueOnce(ok(POPULATED_LIST))

    const ctx = buildHarness()
    useAuthStore().session = FAKE_SESSION
    /* No `seedActiveGame` — gameStore stays empty. */
    const wrapper = await ctx.mountIt()
    await flushPromises()

    expect(wrapper.get('[data-test="account-list-game-name"]').text()).toBe(
      i18nMessages['zh-TW'].accountList.gamePlaceholder,
    )
    expect(wrapper.find('[data-test="account-list-game-image"]').exists()).toBe(false)
  })

  /* ---------------------------------------------------------------- */
  /*  D8e — Tools button conditional visibility                        */
  /* ---------------------------------------------------------------- */

  it('D8e: Tools button hidden for a game outside TOOLS_GAME_CODES (KartRider)', async () => {
    /*
     * KartRider's `610099_T9` is NOT in the WPF whitelist
     * (`MainWindow.xaml.cs` L1710-1713 — only MapleStory TW /
     * MapleStory M / MapleStory R have tools windows). The
     * `v-if="showToolsButton"` gate must keep the button out of
     * the DOM entirely.
     */
    vi.mocked(commands.getAccounts).mockReturnValueOnce(ok(POPULATED_LIST))

    const ctx = buildHarness()
    useAuthStore().session = { ...FAKE_SESSION, service_code: '610099', service_region: 'T9' }
    seedActiveGame(KARTRIDER_TW, KARTRIDER_TW_INI)
    const wrapper = await ctx.mountIt()
    await flushPromises()

    expect(wrapper.find('[data-test="account-list-tools"]').exists()).toBe(false)
  })

  it('D8e: Tools button visible for a game inside TOOLS_GAME_CODES (MapleStory TW)', async () => {
    /*
     * `610074_T9` IS in the WPF whitelist — button must render
     * and be reachable. Pairs with the negative case above to
     * lock the gate from both sides.
     */
    vi.mocked(commands.getAccounts).mockReturnValueOnce(ok(POPULATED_LIST))

    const ctx = buildHarness()
    useAuthStore().session = FAKE_SESSION
    seedActiveGame(MAPLESTORY_TW, MAPLESTORY_TW_INI)
    const wrapper = await ctx.mountIt()
    await flushPromises()

    expect(wrapper.find('[data-test="account-list-tools"]').exists()).toBe(true)
  })

  /* ---------------------------------------------------------------- */
  /*  D8f — Start Game pipeline (direct vs OTP+launch chain)           */
  /* ---------------------------------------------------------------- */

  it('D8f: Start Game direct branch (login_action_type=0) → runGame() with empty creds', async () => {
    /*
     * Mirrors WPF `Pages/AccountList.xaml.cs::Button_Click` L57-63:
     *
     *   if ((tradLogin && login_action_type == 1) || login_action_type == 0)
     *       runGame();
     *
     * `login_action_type=0` always takes the direct branch
     * regardless of `tradLogin`. Assert (a) `getOtp` is NOT
     * called, (b) `launchGame` IS called with empty account/
     * password (the game's own launcher prompts the user), and
     * (c) the path resolution + Start Mode pipeline ran.
     */
    vi.mocked(commands.getAccounts).mockReturnValueOnce(ok(EMPTY_LIST))
    vi.mocked(commands.detectGamePath).mockReturnValueOnce(ok('C:\\Beanfun\\MapleStory.exe'))

    const ctx = buildHarness()
    useAuthStore().session = FAKE_SESSION
    seedActiveGame(MAPLESTORY_TW, { ...MAPLESTORY_TW_INI, login_action_type: '0' })
    const wrapper = await ctx.mountIt()
    await flushPromises()

    await wrapper.get('[data-test="account-list-start"]').trigger('click')
    await flushPromises()

    /* OTP IPC not fired — direct branch skips it. */
    expect(commands.getOtp).not.toHaveBeenCalled()
    /* Path detection ran. */
    expect(commands.detectGamePath).toHaveBeenCalledTimes(1)
    expect(commands.detectGamePath).toHaveBeenCalledWith(
      '610074_T9',
      MAPLESTORY_TW_INI.dir_value_name,
      MAPLESTORY_TW_INI.dir_reg,
      MAPLESTORY_TW_INI.exe,
    )
    /* launchGame called with empty creds + Auto mode (default config). */
    expect(commands.launchGame).toHaveBeenCalledTimes(1)
    expect(commands.launchGame).toHaveBeenCalledWith(
      'C:\\Beanfun\\MapleStory.exe',
      'Auto',
      MAPLESTORY_TW_INI.exe,
      '',
      '',
    )
  })

  it('D8f: traditional login keeps login_action_type=1 on direct empty-credential launch', async () => {
    /*
     * WPF direct branch has two arms:
     *
     *   (tradLogin && login_action_type == 1) || login_action_type == 0
     *
     * The previous case pins the `login_action_type=0` arm; this
     * one pins the traditional-login MapleStory arm so it cannot
     * accidentally be routed through the OTP+command-line branch.
     */
    vi.mocked(commands.getAccounts).mockReturnValueOnce(ok(POPULATED_LIST))
    vi.mocked(commands.detectGamePath).mockReturnValueOnce(ok('C:\\Beanfun\\MapleStory.exe'))

    const ctx = buildHarness()
    useAuthStore().session = FAKE_SESSION
    seedActiveGame(MAPLESTORY_TW, { ...MAPLESTORY_TW_INI, login_action_type: '1' })
    useConfigStore().entries['tradLogin'] = 'true'
    useAccountStore().selectedSid = 'sid-1'

    const wrapper = await ctx.mountIt()
    await flushPromises()

    await wrapper.get('[data-test="account-list-start"]').trigger('click')
    await flushPromises()

    expect(commands.getOtp).not.toHaveBeenCalled()
    expect(commands.launchGame).toHaveBeenCalledWith(
      'C:\\Beanfun\\MapleStory.exe',
      'Auto',
      MAPLESTORY_TW_INI.exe,
      '',
      '',
    )
  })

  it('D8f: Start Game OTP+launch chain (login_action_type=1, tradLogin=false) → getOtp → runGame(account, otp)', async () => {
    /*
     * Mirrors WPF `MainWindow.xaml.cs::getOtpWorker_RunWorkerCompleted`
     * L2152-2155 (`!tradLogin && login_action_type == 1`). The
     * Start Game button routes through `handleGetOtp`, which
     * after the OTP IPC succeeds chains directly into `runGame`
     * with the account + OTP — auto-paste and clipboard branches
     * are skipped because the launcher binary itself receives
     * the credentials via `command_line` substitution.
     */
    vi.mocked(commands.getAccounts).mockReturnValueOnce(ok(POPULATED_LIST))
    vi.mocked(commands.getOtp).mockReturnValueOnce(ok('OTP-CHAIN-123'))
    vi.mocked(commands.detectGamePath).mockReturnValueOnce(ok('C:\\Beanfun\\MapleStory.exe'))

    const ctx = buildHarness()
    useAuthStore().session = FAKE_SESSION
    seedActiveGame(MAPLESTORY_TW, { ...MAPLESTORY_TW_INI, login_action_type: '1' })
    /* Disable tradLogin so the OTP+launch chain is the active branch. */
    useConfigStore().entries['tradLogin'] = 'false'
    useAccountStore().selectedSid = 'sid-1'

    const wrapper = await ctx.mountIt()
    await flushPromises()

    await wrapper.get('[data-test="account-list-start"]').trigger('click')
    await flushPromises()

    /* getOtp fired with the selected account snapshot. */
    expect(commands.getOtp).toHaveBeenCalledTimes(1)
    expect(commands.getOtp).toHaveBeenCalledWith(SERVICE_ACCOUNT)
    /* autoPaste NOT called — chain bypasses it for command-line substitution. */
    expect(commands.autoPaste).not.toHaveBeenCalled()
    /* launchGame called with account + OTP. */
    expect(commands.launchGame).toHaveBeenCalledTimes(1)
    expect(commands.launchGame).toHaveBeenCalledWith(
      'C:\\Beanfun\\MapleStory.exe',
      'Auto',
      MAPLESTORY_TW_INI.exe,
      'sid-1',
      'OTP-CHAIN-123',
    )
  })

  it('D8f: Start Game button is disabled when no game is selected (UI guard)', async () => {
    /*
     * Mirrors WPF UI binding (Start Game button disabled until
     * `selectedGameChanged()` populates the gameCode). The SPA
     * keeps the button always-rendered (the layout would jump on
     * appear/disappear) but the `:disabled="startGameDisabled"`
     * binding makes the affordance unreachable when no game is
     * active. Ensuring the button is disabled — rather than the
     * click handler's runtime fallback — pins the surface the
     * user actually sees: a greyed-out button with cursor:
     * not-allowed, no toast spam from accidental clicks. The
     * runtime `GameSelected` guard inside `handleStartGame` is
     * a belt-and-braces defence for any future caller that
     * bypasses the binding (programmatic click, e2e harness,
     * etc.); it doesn't need its own unit case because the
     * default `setupGameOnMount` no-game cold path already
     * exercises every other no-game guard in the page.
     */
    vi.mocked(commands.getAccounts).mockReturnValueOnce(ok(POPULATED_LIST))

    const ctx = buildHarness()
    useAuthStore().session = FAKE_SESSION
    /* No `seedActiveGame` — gameStore stays empty. */
    const wrapper = await ctx.mountIt()
    await flushPromises()

    const startBtn = wrapper.get('[data-test="account-list-start"]')
    expect((startBtn.element as HTMLButtonElement).disabled).toBe(true)
    /*
     * No IPC is fired even though Pinia is in its initial state —
     * proves the disabled binding is the only thing keeping the
     * pipeline gated, with no spurious mount-time launch attempt.
     */
    expect(commands.getOtp).not.toHaveBeenCalled()
    expect(commands.detectGamePath).not.toHaveBeenCalled()
    expect(commands.launchGame).not.toHaveBeenCalled()
  })

  /* ---------------------------------------------------------------- */
  /*  D8g — Add Account branching (connected vs unconnected)           */
  /* ---------------------------------------------------------------- */

  it('D8g: Add Account on an unconnected game opens UnconnectedGameAddAccount (NOT AddServiceAccount)', async () => {
    /*
     * Mirrors WPF `btnAddServiceAccount_Click`
     * (`AccountList.xaml.cs` L117-135) which forks on
     * `UnconnectedGame` (the same predicate driving
     * `useGameStore.isUnconnectedGame`). The connected-game
     * branch is exercised by the existing D3 wiring test above
     * (now seeded with MapleStory TW); this case is the dual.
     */
    vi.mocked(commands.getAccounts).mockReturnValueOnce(ok(EMPTY_LIST))

    const ctx = buildHarness()
    useAuthStore().session = { ...FAKE_SESSION, service_code: '610153', service_region: 'TN' }
    seedActiveGame(MABINOGI_TN, MABINOGI_TN_INI)
    const wrapper = await ctx.mountIt()
    await flushPromises()

    /* Both stubs start hidden. */
    expect(wrapper.get('[data-test="add-service-account-stub"]').attributes('data-visible')).toBe(
      'false',
    )
    expect(wrapper.get('[data-test="unconnected-add-stub"]').attributes('data-visible')).toBe(
      'false',
    )

    await wrapper.get('[data-test="account-list-add"]').trigger('click')
    await flushPromises()

    /* Unconnected dialog opened, connected one stayed closed. */
    expect(wrapper.get('[data-test="unconnected-add-stub"]').attributes('data-visible')).toBe(
      'true',
    )
    expect(wrapper.get('[data-test="add-service-account-stub"]').attributes('data-visible')).toBe(
      'false',
    )
  })

  it('D8g: Add Account with no selected game → MsgSelectGame warning, no dialog opens', async () => {
    /*
     * Same defensive guard as Start Game above. The mockup keeps
     * the button always-visible so the layout doesn't jump; the
     * runtime gate surfaces the WPF-locale warning rather than
     * silently no-op'ing.
     */
    vi.mocked(commands.getAccounts).mockReturnValueOnce(ok(EMPTY_LIST))

    const ctx = buildHarness()
    useAuthStore().session = FAKE_SESSION
    /* No `seedActiveGame` — gameStore stays empty. */
    const wrapper = await ctx.mountIt()
    await flushPromises()

    await wrapper.get('[data-test="account-list-add"]').trigger('click')
    await flushPromises()

    expect(wrapper.get('[data-test="add-service-account-stub"]').attributes('data-visible')).toBe(
      'false',
    )
    expect(wrapper.get('[data-test="unconnected-add-stub"]').attributes('data-visible')).toBe(
      'false',
    )
    expect(ElMessage.warning).toHaveBeenCalledWith(i18nMessages['zh-TW'].GameSelected)
  })

  /* ---------------------------------------------------------------- */
  /*  D8h — per-row Change Password (unconnected only)                 */
  /* ---------------------------------------------------------------- */

  it('D8h: Change Password row item is hidden for connected games (MapleStory TW)', async () => {
    /*
     * Mirrors WPF `m_ChangePassword.Visibility` (toggled by
     * `selectedGameChanged()` on the same `UnconnectedGame`
     * predicate). Connected games delegate password changes to
     * the Beanfun member centre web flow opened from the page-
     * level chrome — the per-row affordance only exists for
     * unconnected games.
     */
    vi.mocked(commands.getAccounts).mockReturnValueOnce(ok(POPULATED_LIST))

    const ctx = buildHarness()
    useAuthStore().session = FAKE_SESSION
    seedActiveGame(MAPLESTORY_TW, MAPLESTORY_TW_INI)
    const wrapper = await ctx.mountIt()
    await flushPromises()

    /* Other menu items still rendered — only the change-password one is gated. */
    expect(wrapper.find('[data-test="account-row-change-alias-sid-1"]').exists()).toBe(true)
    expect(wrapper.find('[data-test="account-row-change-password-sid-1"]').exists()).toBe(false)
  })

  it('D8h: Change Password row item visible for unconnected games + opens dialog with the row index', async () => {
    /*
     * Mirrors WPF `m_ChangePassword_Click` (`AccountList.xaml.cs`
     * L227-235): `UnconnectedGame_ChangePassword(this,
     * list_Account.SelectedIndex)`. The SPA captures the row's
     * 0-based index in `account.serviceAccounts` at menu-trigger
     * time so a downstream selection / reorder can't misroute
     * the change-password POST.
     *
     * Pick row 2 (sid-2 → index 1) so the assertion catches a
     * regression that hard-codes index 0 instead of using
     * `findIndex(sid)`.
     */
    vi.mocked(commands.getAccounts).mockReturnValueOnce(ok(POPULATED_LIST))

    const ctx = buildHarness()
    useAuthStore().session = { ...FAKE_SESSION, service_code: '610153', service_region: 'TN' }
    seedActiveGame(MABINOGI_TN, MABINOGI_TN_INI)
    const wrapper = await ctx.mountIt()
    await flushPromises()

    const stub = wrapper.get('[data-test="unconnected-changepw-stub"]')
    expect(stub.attributes('data-visible')).toBe('false')
    expect(stub.attributes('data-account-index')).toBe('-1')

    /* Menu item exists (unconnected game) — click it. */
    await wrapper.get('[data-test="account-row-change-password-sid-2"]').trigger('click')
    await flushPromises()

    expect(stub.attributes('data-visible')).toBe('true')
    /* sid-2 is index 1 in POPULATED_LIST. */
    expect(stub.attributes('data-account-index')).toBe('1')
  })

  /* ---------------------------------------------------------------- */
  /*  P12.4-followup-B-fix F8 — Customer Service quick link            */
  /* ---------------------------------------------------------------- */

  /**
   * Mirrors WPF `Pages/AccountList.xaml.cs::btn_Customerservice_Click`
   * (L190-202) — the per-region static URL is dispatched through
   * the in-app webview window (no `web_token` needed on either
   * side). Region is read from `auth.session.region`; if the
   * route guard ever leaks through with no session, the click
   * surfaces a generic toast instead of a runtime crash.
   *
   * The assertions hit the IPC mock directly rather than the
   * composable internals — same observable contract as a real
   * smoke test (the composable is itself unit-tested in
   * `tests/unit/composables/useInAppBrowser.spec.ts`).
   */
  describe('P12.4-followup-B-fix F8 — Customer Service link', () => {
    it('TW session → openInAppBrowser called with tw.beanfun.com URL', async () => {
      vi.mocked(commands.getAccounts).mockReturnValueOnce(ok(EMPTY_LIST))
      vi.mocked(commands.openInAppBrowser).mockReturnValueOnce(ok(null))

      const ctx = buildHarness()
      useAuthStore().session = FAKE_SESSION
      const wrapper = await ctx.mountIt()
      await flushPromises()

      await wrapper.get('[data-test="account-list-customer-service"]').trigger('click')
      await flushPromises()

      expect(commands.openInAppBrowser).toHaveBeenCalledTimes(1)
      expect(commands.openInAppBrowser).toHaveBeenCalledWith(
        'https://tw.beanfun.com/customerservice/www/main.aspx',
      )
    })

    it('HK session → openInAppBrowser called with bfweb.hk.beanfun.com URL', async () => {
      vi.mocked(commands.getAccounts).mockReturnValueOnce(ok(EMPTY_LIST))
      vi.mocked(commands.openInAppBrowser).mockReturnValueOnce(ok(null))

      const ctx = buildHarness()
      useAuthStore().session = { ...FAKE_SESSION, region: 'HK' }
      const wrapper = await ctx.mountIt()
      await flushPromises()

      await wrapper.get('[data-test="account-list-customer-service"]').trigger('click')
      await flushPromises()

      expect(commands.openInAppBrowser).toHaveBeenCalledTimes(1)
      expect(commands.openInAppBrowser).toHaveBeenCalledWith(
        'https://bfweb.hk.beanfun.com/newfaq/service_newBF.aspx',
      )
    })

    it('no session → generic error toast, IPC never invoked', async () => {
      /*
       * Defence in depth — the AccountList page lives behind the
       * auth route guard so this branch should be unreachable in
       * production. Locking the behaviour anyway so a future
       * route-guard regression surfaces a toast (visible failure)
       * rather than a silent no-op or a TypeError on
       * `undefined.region`.
       */
      vi.mocked(commands.getAccounts).mockReturnValueOnce(ok(EMPTY_LIST))

      const ctx = buildHarness()
      useAuthStore().session = FAKE_SESSION
      const wrapper = await ctx.mountIt()
      await flushPromises()

      /* Strip the session AFTER mount so the route guard's normal
       * pre-mount check is bypassed and we hit the in-handler
       * defensive branch. */
      useAuthStore().session = null

      await wrapper.get('[data-test="account-list-customer-service"]').trigger('click')
      await flushPromises()

      expect(commands.openInAppBrowser).not.toHaveBeenCalled()
      expect(ElMessage.error).toHaveBeenCalledWith(i18nMessages['zh-TW'].inAppBrowser.openFailed)
    })
  })

  /* ---------------------------------------------------------------- */
  /*  P12.4-followup-B-fix F9 — Member Center quick link               */
  /* ---------------------------------------------------------------- */

  /**
   * Mirrors WPF `Pages/AccountList.xaml.cs::BF_btnMember_Click`
   * (L167-188). The URL is built backend-side because it embeds
   * the session's `web_token` (server-side secret per
   * `commands::dto`'s sentinel test). The frontend therefore
   * dispatches the dedicated `openMemberCenterBrowser` IPC
   * command with no arguments — no `web_token` ever crosses the
   * IPC boundary.
   *
   * The URL-shape contract for both regions is locked down by
   * Rust unit tests in
   * `commands::web_browser::build_member_center_url_*` (TW and
   * HK byte-for-byte WPF mirrors); these tests just lock the
   * dispatch + error-path contract.
   */
  describe('P12.4-followup-B-fix F9 — Member Center link', () => {
    it('click → openMemberCenterBrowser invoked with no arguments', async () => {
      vi.mocked(commands.getAccounts).mockReturnValueOnce(ok(EMPTY_LIST))
      vi.mocked(commands.openMemberCenterBrowser).mockReturnValueOnce(ok(null))

      const ctx = buildHarness()
      useAuthStore().session = FAKE_SESSION
      const wrapper = await ctx.mountIt()
      await flushPromises()

      await wrapper.get('[data-test="account-list-member-center"]').trigger('click')
      await flushPromises()

      expect(commands.openMemberCenterBrowser).toHaveBeenCalledTimes(1)
      expect(commands.openMemberCenterBrowser).toHaveBeenCalledWith()
      /*
       * Crucially, the legacy frontend in-app browser command must
       * NOT be invoked — a regression that routes Member Center
       * through `openInAppBrowser` would smuggle the URL (and
       * therefore the `web_token` query parameter) across IPC.
       */
      expect(commands.openInAppBrowser).not.toHaveBeenCalled()
    })

    it('backend error → toast surfaces error message, no second invoke', async () => {
      vi.mocked(commands.getAccounts).mockReturnValueOnce(ok(EMPTY_LIST))
      vi.mocked(commands.openMemberCenterBrowser).mockReturnValueOnce(
        err({ code: 'ui.window_create_failed', message: 'WebView2 unavailable', details: null }),
      )

      const ctx = buildHarness()
      useAuthStore().session = FAKE_SESSION
      const wrapper = await ctx.mountIt()
      await flushPromises()

      await wrapper.get('[data-test="account-list-member-center"]').trigger('click')
      await flushPromises()

      expect(commands.openMemberCenterBrowser).toHaveBeenCalledTimes(1)
      expect(ElMessage.error).toHaveBeenCalledWith('WebView2 unavailable')
    })

    it('backend error with empty message → fallback to inAppBrowser.openFailed toast', async () => {
      vi.mocked(commands.getAccounts).mockReturnValueOnce(ok(EMPTY_LIST))
      vi.mocked(commands.openMemberCenterBrowser).mockReturnValueOnce(
        err({ code: 'auth.session_required', message: '', details: null }),
      )

      const ctx = buildHarness()
      useAuthStore().session = FAKE_SESSION
      const wrapper = await ctx.mountIt()
      await flushPromises()

      await wrapper.get('[data-test="account-list-member-center"]').trigger('click')
      await flushPromises()

      expect(ElMessage.error).toHaveBeenCalledWith(i18nMessages['zh-TW'].inAppBrowser.openFailed)
    })
  })
})
