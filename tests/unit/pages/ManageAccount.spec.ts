/**
 * P12.2 D9 — ManageAccount page behaviour.
 *
 * What this spec locks down (matches the D9 scope outlined in
 * `pages/ManageAccount.vue`):
 *
 * 1. Renders the four list states (`loading` / `empty` / `error` /
 *    `ready`) driven by the store's `loadAccounts` round-trip; the
 *    in-list `noSearchResult` placeholder is a sub-state of `ready`.
 * 2. Search filter narrows visible rows by `account_id` +
 *    `account_name` (case-insensitive contains); the `totalAccounts`
 *    stat stays anchored to the unfiltered list count so the user
 *    doesn't see "you have 0 accounts" mid-search.
 * 3. Edit row → `<ChangeAccount>` opens with the row snapshot
 *    forwarded as the `account` prop (verifies the snapshot pattern
 *    docblocked in the SUT).
 * 4. Delete row → `ElMessageBox.confirm`; **Yes** branch invokes
 *    `commands.removeAccount(region, account_id)` (mirror of WPF
 *    `Delete_Button_Click`), **No** branch is a hard no-op.
 * 5. Copy account ID → `navigator.clipboard.writeText(account_id)`
 *    + success toast (mockup-introduced UX, no WPF parity).
 * 6. Import flow happy path → `dialog.open` returns a path →
 *    `ElMessageBox.confirm` (overwrite gate, Q12) confirms →
 *    `commands.importRecords(path)` invoked + success toast.
 * 7. Import flow user cancels the file picker (`dialog.open` returns
 *    `null`) → silent no-op (no IPC, no toast, no overwrite confirm).
 * 8. Export flow happy path → `dialog.save` returns a path →
 *    `commands.exportRecords(path)` invoked + success toast.
 * 9. Toolbar `DataBackup` button (D10.4) opens the AccRecovery
 *    dialog (mirror of WPF `DataBackup_Click` →
 *    `new AccRecovery().Show()`).
 * 10. AccRecovery → `restored` event re-derives the in-page
 *     `loadState` so the post-restore list (which the dialog
 *     already wrote into `account.accounts`) paints immediately,
 *     mirroring WPF's `loginMethodInit()` post-restore call.
 *
 * Out of scope (locked down elsewhere, or deferred):
 * - AddAccount / ChangeAccount internal form behaviour — owned by
 *   `tests/unit/windows/{AddAccount,ChangeAccount}.spec.ts` (D8).
 * - Drag-to-reorder — Q3 = deferred (visual handle only); no runtime
 *   reorder logic exists in D9 to assert.
 * - Multi-select / batch delete — Q9 = single-delete only.
 */

import { beforeEach, afterEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { flushPromises, mount } from '@vue/test-utils'
import { defineComponent, h, type Component } from 'vue'

import type { Account, CommandError, Result } from '../../../src/types/bindings'

const { elMessage, elMessageBoxConfirm } = vi.hoisted(() => ({
  elMessage: { error: vi.fn(), success: vi.fn(), warning: vi.fn(), info: vi.fn() },
  elMessageBoxConfirm: vi.fn(),
}))

const { dialogOpen, dialogSave } = vi.hoisted(() => ({
  dialogOpen: vi.fn(),
  dialogSave: vi.fn(),
}))

vi.mock('@tauri-apps/plugin-dialog', () => ({
  open: dialogOpen,
  save: dialogSave,
}))

/*
 * `useRouter` mock — the SUT calls `router.back()` / `router.push()`
 * inside `handleBack` (P12.4-followup-B-fix F6). Going through a
 * real `createMemoryHistory` router would force every existing test
 * (which never navigates) to also juggle router setup/teardown;
 * mocking the composable keeps the existing harness untouched and
 * lets the new back-navigation cases assert exact call shapes
 * directly on the spies.
 */
const { routerBack, routerPush } = vi.hoisted(() => ({
  routerBack: vi.fn(),
  routerPush: vi.fn(),
}))
vi.mock('vue-router', () => ({
  useRouter: () => ({ back: routerBack, push: routerPush }),
}))

vi.mock('element-plus', async () => {
  const { defineComponent: dc, h: hh } = await import('vue')

  const ElButton = dc({
    name: 'ElButtonStub',
    emits: ['click'],
    setup(_, { slots, emit, attrs }) {
      return () =>
        hh(
          'button',
          {
            ...attrs,
            class: 'el-button-stub',
            onClick: (e: MouseEvent) => emit('click', e),
          },
          slots.default?.(),
        )
    },
  })

  /*
   * Render `ElInput` as a native `<input>` so `setValue('foo')`
   * triggers the same v-model update path the real component uses.
   * `...attrs` (including `data-test`, `disabled`, `placeholder`)
   * spreads onto the **inner input** so the SUT's selectors
   * (`wrapper.get('[data-test="..."]').setValue(...)`) resolve to
   * the form control rather than the wrapper. Slot `prefix` renders
   * as a sibling so the search-icon affordance still mounts; not
   * asserting visual placement.
   */
  const ElInput = dc({
    name: 'ElInputStub',
    inheritAttrs: false,
    props: { modelValue: { type: String, default: '' } },
    emits: ['update:modelValue'],
    setup(props, { emit, attrs, slots }) {
      return () =>
        hh('div', { class: 'el-input-stub-wrap' }, [
          slots.prefix?.(),
          hh('input', {
            ...attrs,
            class: 'el-input-stub',
            value: props.modelValue,
            onInput: (e: Event) => emit('update:modelValue', (e.target as HTMLInputElement).value),
          }),
        ])
    },
  })

  const ElIcon = dc({
    name: 'ElIconStub',
    setup(_, { slots }) {
      return () => hh('span', { class: 'el-icon-stub' }, slots.default?.())
    },
  })

  return {
    ElButton,
    ElInput,
    ElIcon,
    ElMessage: elMessage,
    ElMessageBox: { confirm: elMessageBoxConfirm },
  }
})

vi.mock('@element-plus/icons-vue', () => {
  const stub = (name: string): Component => defineComponent({ name, render: () => h('svg') })
  return {
    /* SUT template icons */
    ArrowLeft: stub('ArrowLeftStub'),
    Delete: stub('DeleteStub'),
    DocumentCopy: stub('DocumentCopyStub'),
    Download: stub('DownloadStub'),
    EditPen: stub('EditPenStub'),
    InfoFilled: stub('InfoFilledStub'),
    Lock: stub('LockStub'),
    Plus: stub('PlusStub'),
    Rank: stub('RankStub'),
    Search: stub('SearchStub'),
    Upload: stub('UploadStub'),
    UserFilled: stub('UserFilledStub'),
    /*
     * AddAccount.vue + ChangeAccount.vue (transitively imported by
     * the SUT) reference the icons below from their own templates.
     * The dialog stubs render nothing so they never paint, but the
     * import side-effect still needs satisfying.
     */
    CircleClose: stub('CircleCloseStub'),
    CirclePlus: stub('CirclePlusStub'),
    Check: stub('CheckStub'),
  }
})

vi.mock('../../../src/types/bindings', () => ({
  commands: {
    loadAccounts: vi.fn(),
    saveAccount: vi.fn(),
    removeAccount: vi.fn(),
    importRecords: vi.fn(),
    exportRecords: vi.fn(),
  },
}))

import { commands } from '../../../src/types/bindings'
import ManageAccount from '../../../src/pages/ManageAccount.vue'
import { createAppI18n } from '../../../src/i18n'
import { FRONTEND_ONLY_MESSAGES } from '../../../src/i18n/messages'

const ok = <T>(data: T): Promise<Result<T, CommandError>> => Promise.resolve({ status: 'ok', data })

/**
 * Stub `<AddAccount>` — the SUT only owns the "click + open" wiring;
 * the dialog's internal form / submit / WPF parity lives in
 * `tests/unit/windows/AddAccount.spec.ts`. Stub forwards `visible`
 * into a `data-visible` attribute so tests can assert toggle state
 * without poking at internal refs.
 */
const AddAccountStub = defineComponent({
  name: 'AddAccount',
  props: { visible: { type: Boolean, default: false } },
  emits: ['update:visible', 'created'],
  setup(props) {
    return () =>
      h('div', {
        class: 'add-account-stub',
        'data-test': 'add-account-stub',
        'data-visible': String(props.visible),
      })
  },
})

/**
 * Stub `<ChangeAccount>` — same SRP rationale as AddAccountStub.
 * Forwards both `visible` and the bound `account` prop so the spec
 * can assert the snapshot pattern (the SUT must seed `editTarget`
 * before flipping `editVisible` so the dialog gets the right row).
 */
const ChangeAccountStub = defineComponent({
  name: 'ChangeAccount',
  props: {
    visible: { type: Boolean, default: false },
    account: { type: Object as () => Account | null, default: null },
  },
  emits: ['update:visible', 'updated'],
  setup(props) {
    return () =>
      h('div', {
        class: 'change-account-stub',
        'data-test': 'change-account-stub',
        'data-visible': String(props.visible),
        'data-account-id': props.account?.account_id ?? '',
        'data-account-region': props.account?.region ?? '',
      })
  },
})

/**
 * Stub `<AccRecovery>` — D10.3 dialog. The page only owns the
 * "click → open" + `@restored → re-derive list state` wiring; the
 * AES backup/restore behaviour itself lives in
 * `tests/unit/windows/AccRecovery.spec.ts`. We expose:
 *
 * - `data-visible` so the open-on-click test can assert the v-model
 *   flipped on click.
 * - A `data-test="acc-recovery-emit-restored"` button that fires
 *   the `restored` event so the refresh test can drive the
 *   parent's post-restore branch without going through the AES IPC.
 */
const AccRecoveryStub = defineComponent({
  name: 'AccRecovery',
  props: { visible: { type: Boolean, default: false } },
  emits: ['update:visible', 'restored'],
  setup(props, { emit }) {
    return () =>
      h(
        'div',
        {
          class: 'acc-recovery-stub',
          'data-test': 'acc-recovery-stub',
          'data-visible': String(props.visible),
        },
        [
          h('button', {
            type: 'button',
            'data-test': 'acc-recovery-emit-restored',
            onClick: () => emit('restored'),
          }),
        ],
      )
  },
})

const ALICE: Account = {
  region: 'TW',
  account_id: 'alice_tw',
  account_name: 'Main',
  password: 'pw',
  verify: '',
  method: 0,
  auto_login: true,
}

const BOB: Account = {
  region: 'HK',
  account_id: 'bob_hk',
  account_name: 'HK Mule',
  password: '',
  verify: '',
  method: 0,
  auto_login: false,
}

const CATHY: Account = {
  region: 'TW',
  account_id: 'cathy_tw',
  account_name: '',
  password: 'pw',
  verify: '',
  method: 0,
  auto_login: false,
}

const POPULATED: Account[] = [ALICE, BOB, CATHY]

/**
 * Standalone harness — direct mount (no router needed; the SUT
 * doesn't navigate). Stubs the two dialog children so dialog
 * internals stay out of this spec's blast radius.
 */
function mountIt() {
  const i18n = createAppI18n()
  return mount(ManageAccount, {
    global: {
      plugins: [i18n],
      stubs: {
        AddAccount: AddAccountStub,
        ChangeAccount: ChangeAccountStub,
        AccRecovery: AccRecoveryStub,
      },
    },
  })
}

/**
 * Per-test clipboard shim — same `Object.defineProperty` write-around
 * as `AccountList.spec.ts::installClipboardMock` because jsdom 29 has
 * `navigator.clipboard` declared as a non-writable getter on the
 * navigator prototype.
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

describe('ManageAccount page', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    vi.spyOn(console, 'error').mockImplementation(() => {})
    vi.spyOn(console, 'warn').mockImplementation(() => {})
    for (const fn of Object.values(commands) as ReturnType<typeof vi.fn>[]) fn.mockReset()
    elMessage.error.mockReset()
    elMessage.success.mockReset()
    elMessage.warning.mockReset()
    elMessage.info.mockReset()
    elMessageBoxConfirm.mockReset()
    dialogOpen.mockReset()
    dialogSave.mockReset()
    routerBack.mockReset()
    routerPush.mockReset()
  })

  afterEach(() => {
    /*
     * Restore a fresh empty clipboard slot per test — leaving the
     * mocked writeText in place would let spy expectations from one
     * case bleed into the next.
     */
    Object.defineProperty(navigator, 'clipboard', {
      value: undefined,
      configurable: true,
      writable: true,
    })
  })

  it('shows the loading placeholder while loadAccounts is in flight', async () => {
    /*
     * Hand-crafted pending promise — never resolves during the test
     * — so the page stays in the `loading` branch long enough to
     * assert. Resolved after the assert so vitest's
     * unhandled-rejection check stays quiet.
     */
    let resolveFetch!: (r: Result<Account[], CommandError>) => void
    const pending = new Promise<Result<Account[], CommandError>>((resolve) => {
      resolveFetch = resolve
    })
    vi.mocked(commands.loadAccounts).mockReturnValueOnce(pending)

    const wrapper = mountIt()

    expect(wrapper.find('[data-test="manage-account-loading"]').exists()).toBe(true)
    expect(wrapper.find('[data-test="manage-account-empty"]').exists()).toBe(false)
    /* Search input is disabled while loading per Q6 / D9.7 wiring. */
    const searchInput = wrapper.get('[data-test="manage-account-search"]').element
    expect((searchInput as HTMLInputElement).disabled).toBe(true)

    resolveFetch({ status: 'ok', data: [] })
    await flushPromises()
  })

  it('renders the empty state when loadAccounts returns no records', async () => {
    vi.mocked(commands.loadAccounts).mockReturnValueOnce(ok([]))

    const wrapper = mountIt()
    await flushPromises()

    expect(wrapper.find('[data-test="manage-account-empty"]').exists()).toBe(true)
    expect(wrapper.find('[data-test="manage-account-loading"]').exists()).toBe(false)
    expect(wrapper.find('[data-test="manage-account-total"]').text()).toBe('0')
  })

  it('renders one row per stored account with avatar + region chip + remark fallback', async () => {
    vi.mocked(commands.loadAccounts).mockReturnValueOnce(ok(POPULATED))

    const wrapper = mountIt()
    await flushPromises()

    expect(wrapper.find('[data-test="manage-account-total"]').text()).toBe('3')
    expect(wrapper.find('[data-test="manage-account-row-TW-alice_tw"]').exists()).toBe(true)
    expect(wrapper.find('[data-test="manage-account-row-HK-bob_hk"]').exists()).toBe(true)
    expect(wrapper.find('[data-test="manage-account-row-TW-cathy_tw"]').exists()).toBe(true)

    /*
     * Cathy has no remark → italic placeholder copy; Alice has a
     * remark → plain text. Asserting the placeholder copy directly
     * verifies the empty-remark branch in the SUT template.
     */
    const cathyRow = wrapper.get('[data-test="manage-account-row-TW-cathy_tw"]')
    expect(cathyRow.text()).toContain(FRONTEND_ONLY_MESSAGES['zh-TW'].manageAccount.remarkEmpty)
    const aliceRow = wrapper.get('[data-test="manage-account-row-TW-alice_tw"]')
    expect(aliceRow.text()).toContain('Main')
  })

  it('search input filters rows by account_id / account_name (case-insensitive); stat stays anchored to total', async () => {
    vi.mocked(commands.loadAccounts).mockReturnValueOnce(ok(POPULATED))

    const wrapper = mountIt()
    await flushPromises()

    /* Type "BOB" (uppercase) — should still match `bob_hk`. */
    await wrapper.get('[data-test="manage-account-search"]').setValue('BOB')
    await flushPromises()

    expect(wrapper.find('[data-test="manage-account-row-HK-bob_hk"]').exists()).toBe(true)
    expect(wrapper.find('[data-test="manage-account-row-TW-alice_tw"]').exists()).toBe(false)
    expect(wrapper.find('[data-test="manage-account-row-TW-cathy_tw"]').exists()).toBe(false)
    /* Stat card stays at 3 — search is a view filter, not a count modifier. */
    expect(wrapper.find('[data-test="manage-account-total"]').text()).toBe('3')

    /* Junk query renders the no-search-result placeholder. */
    await wrapper.get('[data-test="manage-account-search"]').setValue('zzzzzz')
    await flushPromises()
    expect(wrapper.find('[data-test="manage-account-no-search-result"]').exists()).toBe(true)
    expect(wrapper.find('[data-test="manage-account-row-HK-bob_hk"]').exists()).toBe(false)
  })

  it("clicking a row's edit icon opens ChangeAccount with the row snapshot as the account prop", async () => {
    vi.mocked(commands.loadAccounts).mockReturnValueOnce(ok(POPULATED))

    const wrapper = mountIt()
    await flushPromises()

    /* Pre-condition: dialog is closed and unbound. */
    const stubBefore = wrapper.get('[data-test="change-account-stub"]')
    expect(stubBefore.attributes('data-visible')).toBe('false')
    expect(stubBefore.attributes('data-account-id')).toBe('')

    await wrapper.get('[data-test="manage-account-edit-HK-bob_hk"]').trigger('click')
    await flushPromises()

    const stubAfter = wrapper.get('[data-test="change-account-stub"]')
    expect(stubAfter.attributes('data-visible')).toBe('true')
    expect(stubAfter.attributes('data-account-id')).toBe('bob_hk')
    expect(stubAfter.attributes('data-account-region')).toBe('HK')
  })

  it('delete row + confirm Yes → invokes commands.removeAccount; cancel branch is a hard no-op', async () => {
    vi.mocked(commands.loadAccounts).mockReturnValueOnce(ok(POPULATED))
    /*
     * After deleting Alice the store re-loads with a 2-row response.
     * `removeAccount` returns the post-delete list per the IPC
     * contract (`Result<Account[], CommandError>`).
     */
    vi.mocked(commands.removeAccount).mockReturnValueOnce(ok([BOB, CATHY]))

    const wrapper = mountIt()
    await flushPromises()

    /* --------------- Yes branch --------------- */
    elMessageBoxConfirm.mockResolvedValueOnce('confirm')
    await wrapper.get('[data-test="manage-account-delete-TW-alice_tw"]').trigger('click')
    await flushPromises()

    expect(elMessageBoxConfirm).toHaveBeenCalledTimes(1)
    /*
     * Regression guard for the `t('MsgDeleteAccount*', { 0: x })` vs
     * `t('MsgDeleteAccount*', [x])` foot-gun: vue-i18n list-style
     * placeholders (`{0}`) only resolve via array values, so the
     * object form silently leaves the literal `{0}` in the rendered
     * string. Asserting the message body **contains** `account_id`
     * **and** does **not** contain the unresolved `{0}` token
     * exercises both the WPF locale interpolation contract
     * (`MsgDeleteAccountSingle("alice_tw")`) and the outer
     * `MsgDeleteAccountMng(<subject>)` wrap.
     */
    const [confirmMessage, confirmTitle] = elMessageBoxConfirm.mock.calls[0]!
    expect(typeof confirmMessage).toBe('string')
    expect(confirmMessage).toContain('alice_tw')
    expect(confirmMessage).not.toContain('{0}')
    /*
     * Title is the rendered `t('DeleteAccount')` (zh-TW default
     * locale → "移除帳號"), not the raw key literal — assert it's a
     * non-empty string that doesn't accidentally bypass i18n.
     */
    expect(typeof confirmTitle).toBe('string')
    expect((confirmTitle as string).length).toBeGreaterThan(0)
    expect(confirmTitle).not.toBe('DeleteAccount')

    expect(commands.removeAccount).toHaveBeenCalledTimes(1)
    expect(commands.removeAccount).toHaveBeenCalledWith('TW', 'alice_tw')
    expect(wrapper.find('[data-test="manage-account-row-TW-alice_tw"]').exists()).toBe(false)
    expect(wrapper.find('[data-test="manage-account-total"]').text()).toBe('2')

    /* --------------- No branch --------------- */
    elMessageBoxConfirm.mockRejectedValueOnce('cancel')
    await wrapper.get('[data-test="manage-account-delete-HK-bob_hk"]').trigger('click')
    await flushPromises()

    /* Confirm fired again, but no extra IPC call from the cancel. */
    expect(elMessageBoxConfirm).toHaveBeenCalledTimes(2)
    expect(commands.removeAccount).toHaveBeenCalledTimes(1)
    expect(wrapper.find('[data-test="manage-account-row-HK-bob_hk"]').exists()).toBe(true)
  })

  it('copy ID button writes the account_id to the clipboard and toasts success', async () => {
    const clipboard = installClipboardMock()
    vi.mocked(commands.loadAccounts).mockReturnValueOnce(ok(POPULATED))

    const wrapper = mountIt()
    await flushPromises()

    await wrapper.get('[data-test="manage-account-copy-TW-alice_tw"]').trigger('click')
    await flushPromises()

    expect(clipboard.writeText).toHaveBeenCalledTimes(1)
    expect(clipboard.writeText).toHaveBeenCalledWith('alice_tw')
    expect(elMessage.success).toHaveBeenCalledWith(
      FRONTEND_ONLY_MESSAGES['zh-TW'].manageAccount.idCopied,
    )
  })

  it('import flow happy path: file picker → overwrite confirm → commands.importRecords + success toast', async () => {
    vi.mocked(commands.loadAccounts).mockReturnValueOnce(ok(POPULATED))
    /*
     * After import the store overwrites with a new list. The
     * `importRecords` IPC returns the post-import account list per
     * the contract.
     */
    const imported: Account[] = [ALICE]
    vi.mocked(commands.importRecords).mockReturnValueOnce(ok(imported))
    dialogOpen.mockResolvedValueOnce('C:/tmp/seed.json')
    elMessageBoxConfirm.mockResolvedValueOnce('confirm')

    const wrapper = mountIt()
    await flushPromises()

    await wrapper.get('[data-test="manage-account-import"]').trigger('click')
    await flushPromises()

    expect(dialogOpen).toHaveBeenCalledTimes(1)
    /* Q12: overwrite confirm gate must fire before the IPC. */
    expect(elMessageBoxConfirm).toHaveBeenCalledTimes(1)
    expect(commands.importRecords).toHaveBeenCalledTimes(1)
    expect(commands.importRecords).toHaveBeenCalledWith('C:/tmp/seed.json')
    expect(elMessage.success).toHaveBeenCalledWith(
      FRONTEND_ONLY_MESSAGES['zh-TW'].manageAccount.importSuccess,
    )
    /* Post-import list re-rendered. */
    expect(wrapper.find('[data-test="manage-account-row-TW-alice_tw"]').exists()).toBe(true)
    expect(wrapper.find('[data-test="manage-account-row-HK-bob_hk"]').exists()).toBe(false)
    expect(wrapper.find('[data-test="manage-account-total"]').text()).toBe('1')
  })

  it('import flow with cancelled file picker: no IPC, no overwrite confirm, no toast', async () => {
    vi.mocked(commands.loadAccounts).mockReturnValueOnce(ok(POPULATED))
    /* User cancels the native picker → returns `null`. */
    dialogOpen.mockResolvedValueOnce(null)

    const wrapper = mountIt()
    await flushPromises()

    await wrapper.get('[data-test="manage-account-import"]').trigger('click')
    await flushPromises()

    expect(dialogOpen).toHaveBeenCalledTimes(1)
    expect(elMessageBoxConfirm).not.toHaveBeenCalled()
    expect(commands.importRecords).not.toHaveBeenCalled()
    expect(elMessage.success).not.toHaveBeenCalled()
    expect(elMessage.error).not.toHaveBeenCalled()
  })

  it('export flow happy path: file picker → commands.exportRecords + success toast (no overwrite confirm)', async () => {
    vi.mocked(commands.loadAccounts).mockReturnValueOnce(ok(POPULATED))
    vi.mocked(commands.exportRecords).mockReturnValueOnce(ok(null))
    dialogSave.mockResolvedValueOnce('C:/tmp/dump.json')

    const wrapper = mountIt()
    await flushPromises()

    await wrapper.get('[data-test="manage-account-export"]').trigger('click')
    await flushPromises()

    expect(dialogSave).toHaveBeenCalledTimes(1)
    /* Export is non-destructive — no confirm gate. */
    expect(elMessageBoxConfirm).not.toHaveBeenCalled()
    expect(commands.exportRecords).toHaveBeenCalledTimes(1)
    expect(commands.exportRecords).toHaveBeenCalledWith('C:/tmp/dump.json')
    expect(elMessage.success).toHaveBeenCalledWith(
      FRONTEND_ONLY_MESSAGES['zh-TW'].manageAccount.exportSuccess,
    )
  })

  it('Add Account toolbar button toggles the AddAccount dialog visible', async () => {
    vi.mocked(commands.loadAccounts).mockReturnValueOnce(ok([]))

    const wrapper = mountIt()
    await flushPromises()

    /* Pre-condition: dialog stub starts closed. */
    expect(wrapper.get('[data-test="add-account-stub"]').attributes('data-visible')).toBe('false')

    await wrapper.get('[data-test="manage-account-add"]').trigger('click')
    await flushPromises()

    expect(wrapper.get('[data-test="add-account-stub"]').attributes('data-visible')).toBe('true')
  })

  it('DataBackup toolbar button opens the AccRecovery dialog', async () => {
    vi.mocked(commands.loadAccounts).mockReturnValueOnce(ok(POPULATED))

    const wrapper = mountIt()
    await flushPromises()

    /* Pre-condition: dialog stub starts closed. */
    expect(wrapper.get('[data-test="acc-recovery-stub"]').attributes('data-visible')).toBe('false')

    await wrapper.get('[data-test="manage-account-backup"]').trigger('click')
    await flushPromises()

    expect(wrapper.get('[data-test="acc-recovery-stub"]').attributes('data-visible')).toBe('true')
  })

  it('AccRecovery → restored event re-derives loadState from the post-restore store list (mirrors WPF loginMethodInit)', async () => {
    /*
     * Start from `empty` so the `restored` handler must flip the
     * state machine to `ready` to make the test pass — exercises
     * the empty → ready transition the dialog needs after a
     * restore brings new rows into an empty Users.dat.
     */
    vi.mocked(commands.loadAccounts).mockReturnValueOnce(ok([]))

    const wrapper = mountIt()
    await flushPromises()

    expect(wrapper.find('[data-test="manage-account-empty"]').exists()).toBe(true)

    /*
     * Simulate the dialog's successful restore: the dialog itself
     * (in production) writes the post-restore array into
     * `account.accounts` before emitting `restored`. We mirror that
     * here by mutating the store directly so the parent's
     * `handleRestored` sees the new size.
     */
    const { useAccountStore } = await import('../../../src/stores/account')
    const account = useAccountStore()
    account.accounts = POPULATED

    await wrapper.get('[data-test="acc-recovery-emit-restored"]').trigger('click')
    await flushPromises()

    expect(wrapper.find('[data-test="manage-account-empty"]').exists()).toBe(false)
    expect(wrapper.find('[data-test="manage-account-row-TW-alice_tw"]').exists()).toBe(true)
    expect(wrapper.find('[data-test="manage-account-row-HK-bob_hk"]').exists()).toBe(true)
    expect(wrapper.find('[data-test="manage-account-total"]').text()).toBe('3')
  })

  it('export button is disabled when there are no stored accounts (cant export an empty Users.dat)', async () => {
    vi.mocked(commands.loadAccounts).mockReturnValueOnce(ok([]))

    const wrapper = mountIt()
    await flushPromises()

    const exportBtn = wrapper.get('[data-test="manage-account-export"]').element
    expect((exportBtn as HTMLButtonElement).disabled).toBe(true)
  })

  it('renders the error state with retry when loadAccounts fails; retry recovers on second attempt', async () => {
    const error: CommandError = { code: 'storage.io_error', message: 'disk full' }
    vi.mocked(commands.loadAccounts).mockReturnValueOnce(
      Promise.resolve({ status: 'error', error }),
    )

    const wrapper = mountIt()
    await flushPromises()

    expect(wrapper.find('[data-test="manage-account-error"]').exists()).toBe(true)

    /* Recovery: retry → second call succeeds → ready state with rows. */
    vi.mocked(commands.loadAccounts).mockReturnValueOnce(ok(POPULATED))
    await wrapper.get('[data-test="manage-account-retry"]').trigger('click')
    await flushPromises()

    expect(wrapper.find('[data-test="manage-account-error"]').exists()).toBe(false)
    expect(wrapper.find('[data-test="manage-account-row-TW-alice_tw"]').exists()).toBe(true)
  })

  /* --------------- back navigation (P12.4-followup-B-fix F6) --------------- */

  /**
   * Original D9 ship had no back affordance — the page was a
   * dead-end once entered from Settings. F6 wires `handleBack`
   * mirroring `Settings.vue` / `About.vue`.
   *
   * The handler reads `window.history.length` as the "can-go-back"
   * proxy (vue-router exposes no such predicate). We patch it via
   * `Object.defineProperty` rather than `Object.assign` because the
   * `length` getter on `window.history` is non-writable in jsdom.
   */
  function setHistoryLength(value: number): void {
    Object.defineProperty(window.history, 'length', {
      value,
      configurable: true,
      writable: true,
    })
  }

  it('back button calls router.back() when there is prior history (history.length > 1)', async () => {
    vi.mocked(commands.loadAccounts).mockReturnValueOnce(ok(POPULATED))
    setHistoryLength(2)

    const wrapper = mountIt()
    await flushPromises()

    await wrapper.get('[data-test="manage-account-back"]').trigger('click')

    expect(routerBack).toHaveBeenCalledTimes(1)
    expect(routerPush).not.toHaveBeenCalled()
  })

  it('back button falls back to router.push("/settings") when entered via direct hash (history.length === 1)', async () => {
    /*
     * F6 deliberately diverges from `Settings.vue` / `About.vue`
     * (which fall back to `/login`): ManageAccount has exactly one
     * production entry point — `Settings.vue::handleManageAccount`
     * — so the canonical re-entry target is `/settings`. The
     * route is `requiresAuth: true`; falling back to `/login`
     * would either kick the user out unnecessarily or trigger the
     * auth guard's self-loop. Locking the literal here guards
     * against a future copy-paste that drops back to `/login`.
     */
    vi.mocked(commands.loadAccounts).mockReturnValueOnce(ok(POPULATED))
    setHistoryLength(1)

    const wrapper = mountIt()
    await flushPromises()

    await wrapper.get('[data-test="manage-account-back"]').trigger('click')

    expect(routerBack).not.toHaveBeenCalled()
    expect(routerPush).toHaveBeenCalledTimes(1)
    expect(routerPush).toHaveBeenCalledWith('/settings')
  })
})
