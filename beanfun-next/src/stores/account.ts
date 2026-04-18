/**
 * Account store — covers both the *stored* Beanfun login credentials
 * (Users.dat-backed) and the *service* MapleStory accounts of the
 * currently-authenticated session.
 *
 * # Why one store for two distinct entities
 *
 * The two collections are conceptually separate (saved Beanfun
 * credentials vs. live MapleStory accounts under one credential),
 * but the UI surfaces them on the same screens (Account list page
 * shows both, Add/Remove flows touch both). Keeping them in one
 * store avoids cross-store calls during boot / refresh — and
 * matches the P11 Q4 = A "one store per AppState concern" rule of
 * thumb (`AppState::auth` ↔ `auth` store; everything account /
 * storage / records ↔ `account` store).
 *
 * # Caching policy
 *
 * - `accounts` (saved Beanfun creds) — refreshed by every action
 *   that mutates Users.dat (`saveAccount`, `removeAccount`,
 *   `importRecords`). Cached value is the authoritative one
 *   between mutations.
 * - `serviceAccounts` (current session) — explicitly refreshed via
 *   {@link refreshServiceAccounts}. The store also auto-refreshes
 *   after `addServiceAccount` / `changeServiceAccountName` since
 *   both invalidate the list.
 * - `email` / `remainPoint` / `contract` — single-shot lookups,
 *   cached after the first fetch (refreshable). All three are
 *   tied to the active session, so {@link clearSessionData} is
 *   called by the auth store's logout flow to wipe them.
 *
 * Every fallible action funnels through `wrapCommand` for the
 * standard error toast + `auth.session_required` redirect — the
 * account store has no flow-continuation special cases (unlike
 * `auth.ts`).
 */

import { computed, ref } from 'vue'
import { defineStore } from 'pinia'

import { commands } from '../types/bindings'
import type {
  Account,
  AccountListResult,
  AmountLimitNotice,
  ServiceAccount,
} from '../types/bindings'
import { wrapCommand } from '../services/invoke'

export const useAccountStore = defineStore('account', () => {
  /* --------------- stored Beanfun credentials (Users.dat) -------------- */

  const accounts = ref<Account[]>([])

  /** Initial Users.dat load — call once at boot via App.vue. */
  async function loadAccounts(): Promise<Account[]> {
    accounts.value = await wrapCommand(commands.loadAccounts())
    return accounts.value
  }

  /** Persist a record (insert if region+account_id is new, else update). */
  async function saveAccount(account: Account): Promise<Account[]> {
    accounts.value = await wrapCommand(commands.saveAccount(account))
    return accounts.value
  }

  /** Delete a stored credential by region+account_id. */
  async function removeAccount(region: string, accountId: string): Promise<Account[]> {
    accounts.value = await wrapCommand(commands.removeAccount(region, accountId))
    return accounts.value
  }

  async function importRecords(path: string): Promise<Account[]> {
    accounts.value = await wrapCommand(commands.importRecords(path))
    return accounts.value
  }

  async function exportRecords(path: string): Promise<void> {
    await wrapCommand(commands.exportRecords(path))
  }

  /* --------------- service accounts (live session) -------------- */

  const serviceAccounts = ref<ServiceAccount[]>([])
  const amountLimitNotice = ref<AmountLimitNotice>({ kind: 'none' })
  const selectedSid = ref<string | null>(null)

  const selectedServiceAccount = computed<ServiceAccount | null>(() => {
    if (selectedSid.value === null) return null
    return serviceAccounts.value.find((a) => a.sid === selectedSid.value) ?? null
  })

  function applyAccountListResult(r: AccountListResult): void {
    serviceAccounts.value = r.accounts
    amountLimitNotice.value = r.amount_limit_notice
  }

  /**
   * First-render fetch of the service accounts. Calls backend
   * `get_accounts` (semantic: "I need them, may use cache").
   */
  async function getServiceAccounts(): Promise<AccountListResult> {
    const result = await wrapCommand(commands.getAccounts())
    applyAccountListResult(result)
    return result
  }

  /**
   * Forced re-fetch (e.g. user clicked the refresh icon). Calls
   * backend `refresh`; same wire shape as {@link getServiceAccounts}.
   */
  async function refreshServiceAccounts(): Promise<AccountListResult> {
    const result = await wrapCommand(commands.refresh())
    applyAccountListResult(result)
    return result
  }

  async function addServiceAccount(name: string): Promise<boolean> {
    const ok = await wrapCommand(commands.addServiceAccount(name))
    if (ok) await refreshServiceAccounts()
    return ok
  }

  async function changeServiceAccountName(
    newName: string,
    account: ServiceAccount,
  ): Promise<boolean> {
    const ok = await wrapCommand(commands.changeDisplayName(newName, account))
    if (ok) await refreshServiceAccounts()
    return ok
  }

  /**
   * Generate an OTP for the given service account. Returns the OTP
   * string verbatim — caller is responsible for clipboard / paste UI
   * (the WPF reference shows the OTP in a small modal and copies
   * to clipboard simultaneously).
   */
  async function getOtp(account: ServiceAccount): Promise<string> {
    return wrapCommand(commands.getOtp(account))
  }

  /* --------------- session-scoped lookups (cached) -------------- */

  const email = ref<string | null>(null)
  const remainPoint = ref<number | null>(null)
  const contract = ref<string | null>(null)

  async function getEmail(force = false): Promise<string> {
    if (!force && email.value !== null) return email.value
    email.value = await wrapCommand(commands.getEmail())
    return email.value
  }

  async function getRemainPoint(force = false): Promise<number> {
    if (!force && remainPoint.value !== null) return remainPoint.value
    remainPoint.value = await wrapCommand(commands.getRemainPoint())
    return remainPoint.value
  }

  async function getContract(force = false): Promise<string> {
    if (!force && contract.value !== null) return contract.value
    contract.value = await wrapCommand(commands.getContract())
    return contract.value
  }

  /**
   * Wipe every session-scoped piece of state. Called by the auth
   * store's logout action so the next login starts from a clean
   * slate (no stale email / remainPoint / OTP for the previous
   * session showing up in the UI).
   */
  function clearSessionData(): void {
    serviceAccounts.value = []
    amountLimitNotice.value = { kind: 'none' }
    selectedSid.value = null
    email.value = null
    remainPoint.value = null
    contract.value = null
  }

  return {
    accounts,
    loadAccounts,
    saveAccount,
    removeAccount,
    importRecords,
    exportRecords,

    serviceAccounts,
    amountLimitNotice,
    selectedSid,
    selectedServiceAccount,
    getServiceAccounts,
    refreshServiceAccounts,
    addServiceAccount,
    changeServiceAccountName,
    getOtp,

    email,
    remainPoint,
    contract,
    getEmail,
    getRemainPoint,
    getContract,

    clearSessionData,
  }
})
