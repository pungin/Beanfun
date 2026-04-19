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
    Message: stub('MessageStub'),
    MoreFilled: stub('MoreFilledStub'),
    Operation: stub('OperationStub'),
    Plus: stub('PlusStub'),
    Refresh: stub('RefreshStub'),
    Service: stub('ServiceStub'),
    SwitchButton: stub('SwitchButtonStub'),
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
 * D7: stub `vuedraggable` to a thin reactive wrapper that:
 *
 * 1. Renders the `#item` slot once per `:list` element (in
 *    bound order), exposing the same `{ element, index }` slot
 *    shape vuedraggable@4 ships, so the existing row template
 *    inside AccountList renders unchanged.
 * 2. Forwards extra attrs (e.g. `data-test`, `class`) onto the
 *    rendered tag so the `[data-test="account-list-rows"]`
 *    selector in pre-D7 tests keeps resolving.
 * 3. Lets tests fire `@end` directly via
 *    `wrapper.findComponent(DraggableStub).vm.$emit('end')` after
 *    mutating `:list` (mutating Pinia state has the same effect
 *    as Sortable.js' real splice — both flow through Vue
 *    reactivity to the bound prop).
 *
 * Real Sortable.js DOM behaviour (mouse-down on handle, ghost
 * element, animation timing) is intentionally out of scope for
 * unit tests — those would belong in an E2E suite.
 */
vi.mock('vuedraggable', () => ({
  default: defineComponent({
    name: 'DraggableStub',
    props: {
      list: { type: Array, default: () => [] },
      itemKey: { type: [String, Function], default: '' },
    },
    emits: ['end', 'change'],
    setup(props, { slots, attrs }) {
      return () =>
        h(
          'ul',
          { ...attrs, class: ['draggable-stub', attrs.class] },
          (props.list as Array<Record<string, unknown>>).map((element, index) =>
            slots.item ? slots.item({ element, index }) : null,
          ),
        )
    },
  }),
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
  },
}))

import { ElMessage } from 'element-plus'
import { commands } from '../../../src/types/bindings'
import AccountList from '../../../src/pages/AccountList.vue'
import { useAccountStore } from '../../../src/stores/account'
import { useAuthStore } from '../../../src/stores/auth'
import { useConfigStore } from '../../../src/stores/config'
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
    expect(rows[0].text()).toContain('sid-1')
    expect(rows[1].text()).toContain('Mule Account')
    expect(rows[2].text()).toContain('Suspended User')
    /*
     * The banned row swaps the ID line for the localized
     * "Disabled" copy — proves the conditional branch fires.
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
    expect(account.selectedSid).toBeNull()

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

    expect(account.selectedSid).toBeNull()
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
     */
    vi.mocked(commands.getAccounts).mockReturnValueOnce(ok(EMPTY_LIST))

    const ctx = buildHarness()
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
    expect(account.selectedSid).toBeNull()

    await wrapper.get('[data-test="account-row-change-alias-sid-2"]').trigger('click')
    await flushPromises()

    expect(account.selectedSid).toBeNull()
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
    expect(account.selectedSid).toBeNull()

    await wrapper.get('[data-test="account-row-info-sid-2"]').trigger('click')
    await flushPromises()

    expect(account.selectedSid).toBeNull()
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

  it('Tools toolbar button is wired to its own stub handler (NOT the change-game stub)', async () => {
    /*
     * WPF parity guard: WPF `btn_Tools_Click`
     * (`AccountList.xaml.cs` L237-249) is a per-game tools
     * window launcher — it opens MapleTools / KartTools by
     * gameCode and is conditionally Visible only for those 3
     * codes (`MainWindow.xaml.cs` L1710-1713). It is NOT the
     * game switcher.
     *
     * Pre-D10.5 the SPA mistakenly bound the Tools button click
     * to the change-game stub, conflating two distinct WPF
     * surfaces and making the eventual P12.3 wire-up ambiguous.
     * This case locks the button to its own dedicated stub
     * marker so a future regression that swaps the binding back
     * fails loudly.
     */
    vi.mocked(commands.getAccounts).mockReturnValueOnce(ok(POPULATED_LIST))
    const consoleWarn = vi.spyOn(console, 'warn').mockImplementation(() => {})

    const ctx = buildHarness()
    const wrapper = await ctx.mountIt()
    await flushPromises()

    consoleWarn.mockClear()
    await wrapper.get('[data-test="account-list-tools"]').trigger('click')
    await flushPromises()

    expect(consoleWarn).toHaveBeenCalledTimes(1)
    /*
     * The stub marker text is the SUT contract — asserting it
     * pins the binding to the Tools-specific handler rather than
     * the Change Game one (which would log "Change Game ..." and
     * fail this matcher).
     */
    const [logged] = consoleWarn.mock.calls[0] ?? []
    expect(typeof logged).toBe('string')
    expect(logged as string).toContain('[AccountList]')
    expect(logged as string).toContain('Tools')
    expect(logged as string).not.toContain('Change Game')
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

    /* No row selected after the initial fetch. */
    const account = useAccountStore()
    expect(account.selectedSid).toBeNull()

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
   * after Vuedraggable has already mutated the bound `:list` in
   * place (we simulate that by writing to `account.serviceAccounts`
   * directly — same Pinia reactivity flow), the @end handler runs
   * its (a) canonical store-action funnel and (b) silent persist.
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
     * The page's @end handler reads the (already-mutated) sids from
     * `account.serviceAccounts.map(a => a.sid)`, so we simulate
     * Sortable.js' splice by writing to that array directly and
     * then firing the @end emit on the DraggableStub.
     */
    vi.mocked(commands.getAccounts).mockReturnValueOnce(ok(POPULATED_LIST))

    const ctx = buildHarness()
    useAuthStore().session = FAKE_SESSION
    const wrapper = await ctx.mountIt()
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
     * Sortable.js' real onEnd flow first splices the bound array
     * in place, then fires the event. Reproduce that order so the
     * page's @end handler reads the post-drag order from the
     * store, not the pre-drag order.
     */
    account.serviceAccounts = [SECOND_SA, BANNED_SA, SERVICE_ACCOUNT]

    const draggable = wrapper.findComponent({ name: 'DraggableStub' })
    expect(draggable.exists()).toBe(true)
    draggable.vm.$emit('end')
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
    vi.mocked(commands.getAccounts).mockReturnValueOnce(ok(POPULATED_LIST))
    vi.mocked(commands.setConfig).mockReturnValueOnce(
      err({ code: 'config.write_failed', message: 'disk full', details: null }),
    )

    const ctx = buildHarness()
    useAuthStore().session = FAKE_SESSION
    const wrapper = await ctx.mountIt()
    await flushPromises()

    const account = useAccountStore()
    account.serviceAccounts = [SECOND_SA, SERVICE_ACCOUNT, BANNED_SA]

    const draggable = wrapper.findComponent({ name: 'DraggableStub' })
    draggable.vm.$emit('end')
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
})
