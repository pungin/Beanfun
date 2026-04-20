import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'

import type {
  Account,
  AccountListResult,
  CommandError,
  Result,
  ServiceAccount,
} from '../../../src/types/bindings'

vi.mock('element-plus', () => ({ ElMessage: { error: vi.fn() } }))

vi.mock('../../../src/types/bindings', () => ({
  commands: {
    loadAccounts: vi.fn(),
    saveAccount: vi.fn(),
    removeAccount: vi.fn(),
    importRecords: vi.fn(),
    exportRecords: vi.fn(),
    getAccounts: vi.fn(),
    refresh: vi.fn(),
    addServiceAccount: vi.fn(),
    changeDisplayName: vi.fn(),
    getOtp: vi.fn(),
    getEmail: vi.fn(),
    getRemainPoint: vi.fn(),
    getContract: vi.fn(),
  },
}))

import { commands } from '../../../src/types/bindings'
import { useAccountStore } from '../../../src/stores/account'

const ok = <T>(data: T): Promise<Result<T, CommandError>> => Promise.resolve({ status: 'ok', data })

const ACCOUNT: Account = {
  region: 'TW',
  account_id: 'alice',
  account_name: 'Alice',
  password: 'pw',
  verify: '',
  method: 0,
  auto_login: false,
}

const SERVICE_ACCOUNT: ServiceAccount = {
  is_enable: true,
  visible: true,
  is_inherited: false,
  sid: 'sid-1',
  ssn: '00001',
  sname: 'Toon',
  screatetime: null,
  slastusedtime: null,
  sauthtype: null,
}

const SECOND_SA: ServiceAccount = { ...SERVICE_ACCOUNT, sid: 'sid-2', ssn: '00002', sname: 'Two' }

const ACCOUNT_LIST: AccountListResult = {
  accounts: [SERVICE_ACCOUNT, SECOND_SA],
  amount_limit_notice: { kind: 'none' },
}

const REFRESH_LIST: AccountListResult = {
  accounts: [SERVICE_ACCOUNT, SECOND_SA, { ...SERVICE_ACCOUNT, sid: 'sid-3' }],
  amount_limit_notice: { kind: 'none' },
}

describe('useAccountStore — stored Beanfun credentials', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    vi.spyOn(console, 'error').mockImplementation(() => {})
    for (const fn of Object.values(commands) as ReturnType<typeof vi.fn>[]) fn.mockReset()
  })

  it('loadAccounts populates the cache from Users.dat', async () => {
    vi.mocked(commands.loadAccounts).mockReturnValueOnce(ok([ACCOUNT]))
    const store = useAccountStore()
    const result = await store.loadAccounts()
    expect(result).toEqual([ACCOUNT])
    expect(store.accounts).toEqual([ACCOUNT])
  })

  it('saveAccount overwrites the cache with the backend response', async () => {
    vi.mocked(commands.saveAccount).mockReturnValueOnce(
      ok([ACCOUNT, { ...ACCOUNT, account_id: 'bob' }]),
    )
    const store = useAccountStore()
    await store.saveAccount(ACCOUNT)
    expect(store.accounts).toHaveLength(2)
  })

  it('removeAccount overwrites the cache with the backend response', async () => {
    vi.mocked(commands.removeAccount).mockReturnValueOnce(ok([]))
    const store = useAccountStore()
    store.accounts = [ACCOUNT]
    await store.removeAccount('TW', 'alice')
    expect(store.accounts).toEqual([])
    expect(commands.removeAccount).toHaveBeenCalledWith('TW', 'alice')
  })

  it('importRecords replaces the cache with the imported list', async () => {
    vi.mocked(commands.importRecords).mockReturnValueOnce(ok([ACCOUNT]))
    const store = useAccountStore()
    await store.importRecords('C:\\backup.json')
    expect(commands.importRecords).toHaveBeenCalledWith('C:\\backup.json')
    expect(store.accounts).toEqual([ACCOUNT])
  })

  it('exportRecords does not mutate the cache', async () => {
    vi.mocked(commands.exportRecords).mockReturnValueOnce(ok(null))
    const store = useAccountStore()
    store.accounts = [ACCOUNT]
    await store.exportRecords('C:\\backup.json')
    expect(store.accounts).toEqual([ACCOUNT])
  })

  it('findStoredAccount returns the matching record', () => {
    const store = useAccountStore()
    store.accounts = [ACCOUNT, { ...ACCOUNT, region: 'HK', account_id: 'bob' }]
    expect(store.findStoredAccount('TW', 'alice')).toEqual(ACCOUNT)
  })

  it('findStoredAccount returns undefined when no row matches', () => {
    const store = useAccountStore()
    store.accounts = [ACCOUNT]
    expect(store.findStoredAccount('TW', 'missing')).toBeUndefined()
    expect(store.findStoredAccount('HK', 'alice')).toBeUndefined()
  })
})

describe('useAccountStore — saveLoginCredentials (WPF SaveLoginCredentials parity)', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    vi.spyOn(console, 'error').mockImplementation(() => {})
    for (const fn of Object.values(commands) as ReturnType<typeof vi.fn>[]) fn.mockReset()
  })

  const BASE_INPUT = {
    region: 'TW' as const,
    accountId: 'alice',
    password: 'plaintext-pw',
    rememberPassword: true,
    verify: '',
    rememberVerify: false,
    method: 0,
    autoLogin: false,
  }

  it('TW Regular + remember writes the password verbatim', async () => {
    vi.mocked(commands.saveAccount).mockReturnValueOnce(ok([]))
    const store = useAccountStore()
    await store.saveLoginCredentials(BASE_INPUT)
    expect(commands.saveAccount).toHaveBeenCalledTimes(1)
    const written = vi.mocked(commands.saveAccount).mock.calls[0]![0] as Account
    expect(written).toEqual({
      region: 'TW',
      account_id: 'alice',
      account_name: '',
      password: 'plaintext-pw',
      verify: '',
      method: 0,
      auto_login: false,
    })
  })

  it('TW Regular + remember off writes empty password', async () => {
    vi.mocked(commands.saveAccount).mockReturnValueOnce(ok([]))
    const store = useAccountStore()
    await store.saveLoginCredentials({ ...BASE_INPUT, rememberPassword: false })
    const written = vi.mocked(commands.saveAccount).mock.calls[0]![0] as Account
    expect(written.password).toBe('')
  })

  it('TW QR is skipped (no IPC)', async () => {
    const store = useAccountStore()
    await store.saveLoginCredentials({ ...BASE_INPUT, method: 1 })
    expect(commands.saveAccount).not.toHaveBeenCalled()
  })

  it('HK QR is skipped (Q1 = B fix vs WPF garbage record)', async () => {
    const store = useAccountStore()
    await store.saveLoginCredentials({ ...BASE_INPUT, region: 'HK', method: 1 })
    expect(commands.saveAccount).not.toHaveBeenCalled()
  })

  it('GamePass is skipped (no IPC) — WPF GamePassLoginCompleted bypass parity', async () => {
    const store = useAccountStore()
    await store.saveLoginCredentials({ ...BASE_INPUT, method: 2 })
    expect(commands.saveAccount).not.toHaveBeenCalled()
  })

  it('HK Regular + rememberVerify writes verify field', async () => {
    vi.mocked(commands.saveAccount).mockReturnValueOnce(ok([]))
    const store = useAccountStore()
    await store.saveLoginCredentials({
      ...BASE_INPUT,
      region: 'HK',
      verify: 'V123',
      rememberVerify: true,
    })
    const written = vi.mocked(commands.saveAccount).mock.calls[0]![0] as Account
    expect(written.region).toBe('HK')
    expect(written.verify).toBe('V123')
  })

  it('rememberVerify off writes empty verify even when verify code provided', async () => {
    vi.mocked(commands.saveAccount).mockReturnValueOnce(ok([]))
    const store = useAccountStore()
    await store.saveLoginCredentials({
      ...BASE_INPUT,
      verify: 'V123',
      rememberVerify: false,
    })
    const written = vi.mocked(commands.saveAccount).mock.calls[0]![0] as Account
    expect(written.verify).toBe('')
  })

  it('account_name is always written as "" (WPF L1352 quirk parity)', async () => {
    vi.mocked(commands.saveAccount).mockReturnValueOnce(ok([]))
    const store = useAccountStore()
    store.accounts = [{ ...ACCOUNT, account_name: 'My Existing Alias' }]
    await store.saveLoginCredentials(BASE_INPUT)
    const written = vi.mocked(commands.saveAccount).mock.calls[0]![0] as Account
    expect(written.account_name).toBe('')
  })
})

describe('useAccountStore — service accounts', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    vi.spyOn(console, 'error').mockImplementation(() => {})
    for (const fn of Object.values(commands) as ReturnType<typeof vi.fn>[]) fn.mockReset()
  })

  it('getServiceAccounts populates list + amount_limit_notice', async () => {
    vi.mocked(commands.getAccounts).mockReturnValueOnce(ok(ACCOUNT_LIST))
    const store = useAccountStore()
    await store.getServiceAccounts()
    expect(store.serviceAccounts).toEqual(ACCOUNT_LIST.accounts)
    expect(store.amountLimitNotice).toEqual({ kind: 'none' })
  })

  it('refreshServiceAccounts re-applies the result over the cache', async () => {
    vi.mocked(commands.getAccounts).mockReturnValueOnce(ok(ACCOUNT_LIST))
    vi.mocked(commands.refresh).mockReturnValueOnce(ok(REFRESH_LIST))
    const store = useAccountStore()
    await store.getServiceAccounts()
    await store.refreshServiceAccounts()
    expect(store.serviceAccounts).toHaveLength(3)
  })

  it('selectedServiceAccount resolves selectedSid against the cache', async () => {
    vi.mocked(commands.getAccounts).mockReturnValueOnce(ok(ACCOUNT_LIST))
    const store = useAccountStore()
    await store.getServiceAccounts()
    expect(store.selectedServiceAccount).toBeNull()
    store.selectedSid = 'sid-2'
    expect(store.selectedServiceAccount).toEqual(SECOND_SA)
  })

  it('addServiceAccount triggers a refresh on success', async () => {
    vi.mocked(commands.addServiceAccount).mockReturnValueOnce(ok(true))
    vi.mocked(commands.refresh).mockReturnValueOnce(ok(REFRESH_LIST))
    const store = useAccountStore()
    const ok2 = await store.addServiceAccount('NewToon')
    expect(ok2).toBe(true)
    expect(commands.refresh).toHaveBeenCalledTimes(1)
    expect(store.serviceAccounts).toHaveLength(3)
  })

  it('addServiceAccount does NOT refresh when backend reports failure', async () => {
    vi.mocked(commands.addServiceAccount).mockReturnValueOnce(ok(false))
    const store = useAccountStore()
    const ok2 = await store.addServiceAccount('NewToon')
    expect(ok2).toBe(false)
    expect(commands.refresh).not.toHaveBeenCalled()
  })

  it('changeServiceAccountName triggers a refresh on success', async () => {
    vi.mocked(commands.changeDisplayName).mockReturnValueOnce(ok(true))
    vi.mocked(commands.refresh).mockReturnValueOnce(ok(REFRESH_LIST))
    const store = useAccountStore()
    await store.changeServiceAccountName('Renamed', SERVICE_ACCOUNT)
    expect(commands.changeDisplayName).toHaveBeenCalledWith('Renamed', SERVICE_ACCOUNT)
    expect(commands.refresh).toHaveBeenCalledTimes(1)
  })
})

/* ------------------------------------------------------------------ */
/* D7 — Service-account ordering (WPF ApplyAccountOrder parity)        */
/* ------------------------------------------------------------------ */

/**
 * Exercises the two ordering actions added in P12.2 D7:
 *
 * - {@link useAccountStore.setServiceAccountOrder} — explicit
 *   sid-list reorder; the page calls this after a SortableJS
 *   `@end` event.
 * - {@link useAccountStore.applyServiceAccountOrderFromSavedCsv} —
 *   convenience wrapper that parses the CSV WPF persists under
 *   `Config.xml::AccountOrder_<gameCode>`; the page calls this
 *   after `getServiceAccounts` / `refreshServiceAccounts` to apply
 *   the user's saved order on top of the server-sorted list.
 *
 * The invariants under test (every case asserts at least one) come
 * from the WPF `ApplyAccountOrder` (L489-531) implementation:
 *
 * 1. Sids in the input list **that match a current account** are
 *    emitted in input order.
 * 2. Sids in the input list **that don't match** any current
 *    account (e.g. stale rows since deleted upstream) are silently
 *    dropped — never inserted, never thrown over.
 * 3. Accounts in the store **not mentioned** by the input list
 *    (e.g. fresh add-account rows) are appended in the store's
 *    pre-existing relative order so they never disappear from the
 *    UI.
 *
 * Together these ensure the result is **always a permutation of
 * the same set** — no subset, no superset.
 */
describe('useAccountStore — service-account ordering (D7)', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    vi.spyOn(console, 'error').mockImplementation(() => {})
    for (const fn of Object.values(commands) as ReturnType<typeof vi.fn>[]) fn.mockReset()
  })

  const THIRD_SA: ServiceAccount = {
    ...SERVICE_ACCOUNT,
    sid: 'sid-3',
    ssn: '00003',
    sname: 'Three',
  }

  it('setServiceAccountOrder reorders to match the supplied sid list', async () => {
    vi.mocked(commands.getAccounts).mockReturnValueOnce(
      ok({
        accounts: [SERVICE_ACCOUNT, SECOND_SA, THIRD_SA],
        amount_limit_notice: { kind: 'none' },
      }),
    )
    const store = useAccountStore()
    await store.getServiceAccounts()

    const next = store.setServiceAccountOrder(['sid-2', 'sid-3', 'sid-1'])

    expect(next.map((a) => a.sid)).toEqual(['sid-2', 'sid-3', 'sid-1'])
    expect(store.serviceAccounts.map((a) => a.sid)).toEqual(['sid-2', 'sid-3', 'sid-1'])
  })

  it('setServiceAccountOrder silently skips unknown sids in the input list', async () => {
    /*
     * WPF L515 `if (accountDict.ContainsKey(sid))` — sids in the
     * saved order that no longer match a live account (e.g. row
     * removed via Beanfun web) are skipped, never inserted as a
     * placeholder.
     */
    vi.mocked(commands.getAccounts).mockReturnValueOnce(
      ok({
        accounts: [SERVICE_ACCOUNT, SECOND_SA],
        amount_limit_notice: { kind: 'none' },
      }),
    )
    const store = useAccountStore()
    await store.getServiceAccounts()

    const next = store.setServiceAccountOrder(['sid-99', 'sid-2', 'sid-missing', 'sid-1'])

    expect(next.map((a) => a.sid)).toEqual(['sid-2', 'sid-1'])
    expect(next).toHaveLength(2)
  })

  it('setServiceAccountOrder appends accounts missing from the sid list (no silent drop)', async () => {
    /*
     * WPF L522-526 `foreach (var account in accountDict.Values)
     * orderedList.Add(account)` — fresh accounts the saved order
     * doesn't yet know about must reach the UI. Critical
     * invariant: the result is a permutation of the input set,
     * never a subset.
     */
    vi.mocked(commands.getAccounts).mockReturnValueOnce(
      ok({
        accounts: [SERVICE_ACCOUNT, SECOND_SA, THIRD_SA],
        amount_limit_notice: { kind: 'none' },
      }),
    )
    const store = useAccountStore()
    await store.getServiceAccounts()

    /* Only mention sid-3; sid-1 and sid-2 should append in their original order. */
    const next = store.setServiceAccountOrder(['sid-3'])

    expect(next.map((a) => a.sid)).toEqual(['sid-3', 'sid-1', 'sid-2'])
  })

  it('setServiceAccountOrder is a no-op when the store is empty', async () => {
    const store = useAccountStore()
    expect(store.serviceAccounts).toEqual([])

    const next = store.setServiceAccountOrder(['sid-1', 'sid-2'])

    expect(next).toEqual([])
    expect(store.serviceAccounts).toEqual([])
  })

  it('applyServiceAccountOrderFromSavedCsv returns the existing list when csv is undefined', async () => {
    vi.mocked(commands.getAccounts).mockReturnValueOnce(
      ok({
        accounts: [SERVICE_ACCOUNT, SECOND_SA],
        amount_limit_notice: { kind: 'none' },
      }),
    )
    const store = useAccountStore()
    await store.getServiceAccounts()
    const before = store.serviceAccounts.map((a) => a.sid)

    const next = store.applyServiceAccountOrderFromSavedCsv(undefined)

    expect(next.map((a) => a.sid)).toEqual(before)
  })

  it('applyServiceAccountOrderFromSavedCsv treats empty / whitespace csv as no-op', async () => {
    /*
     * WPF L497-499 `if (string.IsNullOrEmpty(orderString)) return;`
     * — the saved key may exist with a zero-length value when the
     * user has never reordered for this gameCode.
     */
    vi.mocked(commands.getAccounts).mockReturnValueOnce(
      ok({
        accounts: [SERVICE_ACCOUNT, SECOND_SA],
        amount_limit_notice: { kind: 'none' },
      }),
    )
    const store = useAccountStore()
    await store.getServiceAccounts()
    const before = store.serviceAccounts.map((a) => a.sid)

    expect(store.applyServiceAccountOrderFromSavedCsv('').map((a) => a.sid)).toEqual(before)
    expect(store.applyServiceAccountOrderFromSavedCsv('   ').map((a) => a.sid)).toEqual(before)
  })

  it('applyServiceAccountOrderFromSavedCsv parses csv + skips unknown + appends missing', async () => {
    /*
     * Composite case covering the three WPF invariants in one
     * realistic input — the saved csv mentions a stale sid
     * (`sid-99`), reorders two known sids, and forgets a third
     * (`sid-3`). Expected result: stale dropped, mentioned in
     * order, forgotten appended.
     */
    vi.mocked(commands.getAccounts).mockReturnValueOnce(
      ok({
        accounts: [SERVICE_ACCOUNT, SECOND_SA, THIRD_SA],
        amount_limit_notice: { kind: 'none' },
      }),
    )
    const store = useAccountStore()
    await store.getServiceAccounts()

    const next = store.applyServiceAccountOrderFromSavedCsv('sid-2,sid-99,sid-1')

    expect(next.map((a) => a.sid)).toEqual(['sid-2', 'sid-1', 'sid-3'])
    expect(store.serviceAccounts.map((a) => a.sid)).toEqual(['sid-2', 'sid-1', 'sid-3'])
  })
})

describe('useAccountStore — session-scoped lookups', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    vi.spyOn(console, 'error').mockImplementation(() => {})
    for (const fn of Object.values(commands) as ReturnType<typeof vi.fn>[]) fn.mockReset()
  })

  it('getEmail caches the result; force=true bypasses the cache', async () => {
    vi.mocked(commands.getEmail)
      .mockReturnValueOnce(ok('a@example.com'))
      .mockReturnValueOnce(ok('b@example.com'))
    const store = useAccountStore()
    expect(await store.getEmail()).toBe('a@example.com')
    expect(await store.getEmail()).toBe('a@example.com')
    expect(commands.getEmail).toHaveBeenCalledTimes(1)
    expect(await store.getEmail(true)).toBe('b@example.com')
    expect(commands.getEmail).toHaveBeenCalledTimes(2)
  })

  it('getRemainPoint caches numeric zero correctly', async () => {
    vi.mocked(commands.getRemainPoint).mockReturnValueOnce(ok(0))
    const store = useAccountStore()
    expect(await store.getRemainPoint()).toBe(0)
    expect(await store.getRemainPoint()).toBe(0)
    expect(commands.getRemainPoint).toHaveBeenCalledTimes(1)
  })

  it('getContract caches the result', async () => {
    vi.mocked(commands.getContract).mockReturnValueOnce(ok('contract-text'))
    const store = useAccountStore()
    expect(await store.getContract()).toBe('contract-text')
    expect(await store.getContract()).toBe('contract-text')
    expect(commands.getContract).toHaveBeenCalledTimes(1)
  })

  it('clearSessionData wipes the lookup caches and the service-account list', async () => {
    vi.mocked(commands.getAccounts).mockReturnValueOnce(ok(ACCOUNT_LIST))
    vi.mocked(commands.getEmail).mockReturnValueOnce(ok('a@example.com'))
    const store = useAccountStore()
    await store.getServiceAccounts()
    await store.getEmail()
    store.selectedSid = 'sid-1'

    store.clearSessionData()
    expect(store.serviceAccounts).toEqual([])
    expect(store.amountLimitNotice).toEqual({ kind: 'none' })
    expect(store.selectedSid).toBeNull()
    expect(store.email).toBeNull()
    expect(store.remainPoint).toBeNull()
    expect(store.contract).toBeNull()
  })

  it('getOtp passes through to commands.getOtp without cache', async () => {
    vi.mocked(commands.getOtp).mockReturnValueOnce(ok('OTP12345'))
    const store = useAccountStore()
    expect(await store.getOtp(SERVICE_ACCOUNT)).toBe('OTP12345')
    expect(commands.getOtp).toHaveBeenCalledWith(SERVICE_ACCOUNT)
  })
})
