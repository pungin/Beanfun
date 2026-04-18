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
