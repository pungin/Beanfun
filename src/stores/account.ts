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
  LoginRegion,
  ServiceAccount,
} from '../types/bindings'
import { LOGIN_METHOD, type LoginMethod } from '../constants/login'
import { wrapCommand } from '../services/invoke'

/**
 * Input shape for {@link useAccountStore.saveLoginCredentials}.
 *
 * Mirrors the field set WPF `MainWindow.xaml.cs::SaveLoginCredentials`
 * (L1334-1363) reads from the in-memory form controls before
 * calling `accountManager.addAccount`. Each field corresponds to
 * exactly one WPF read site:
 *
 * | Field | WPF source |
 * |---|---|
 * | `region` | `App.LoginRegion` |
 * | `accountId` | `idPassForm.t_AccountID.Text` |
 * | `password` | `idPassForm.t_Password.Password` |
 * | `rememberPassword` | `idPassForm.checkBox_RememberPWD.IsChecked` |
 * | `verify` | `verifyPage.t_Verify.Text` |
 * | `rememberVerify` | `verifyPage.checkBoxRememberVerify.IsChecked` |
 * | `method` | `App.LoginMethod` |
 * | `autoLogin` | `idPassForm.checkBox_AutoLogin.IsChecked` |
 */
export interface SaveLoginCredentialsInput {
  region: LoginRegion
  accountId: string
  password: string
  rememberPassword: boolean
  verify: string
  rememberVerify: boolean
  method: LoginMethod
  autoLogin: boolean
}

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

  /**
   * Lookup helper — returns the saved record matching
   * `(region, accountId)`, or `undefined` when no such row
   * exists. Pure local-cache read (no IPC), so callers can use
   * it inside synchronous Vue templates / computeds.
   *
   * Mirrors WPF `accountManager.getNameByAccount` /
   * `getPasswordByAccount` / `getVerifyByAccount` /
   * `getMethodByAccount` / `getAutoLoginByAccount` collectively —
   * those WPF helpers each scan the parallel-columns lists for
   * the same `(region, accountId)` pair and return one slice of
   * the row. Here we return the full row so callers (mostly
   * `IdPassForm` / `VerifyPage` mount-time prefill) read whatever
   * fields they need without five lookups.
   */
  function findStoredAccount(region: string, accountId: string): Account | undefined {
    return accounts.value.find((a) => a.region === region && a.account_id === accountId)
  }

  /**
   * Persist a successful Regular login's credentials to
   * `Users.dat`, atomically writing all six WPF-tracked fields
   * (`account_name` / `password` / `verify` / `method` /
   * `auto_login` plus the `(region, account_id)` composite key).
   *
   * # WPF parity
   *
   * Direct port of `MainWindow.xaml.cs::SaveLoginCredentials`
   * (L1334-1363) — the field-by-field mapping:
   *
   * - `account_name` is **always written as `""`** to match WPF
   *   L1352 (`accountManager.addAccount(... "" ...)`). This is a
   *   pre-existing WPF quirk: a manual alias set via
   *   `ManageAccount.xaml.cs` gets clobbered by the next login
   *   that hits this code path. We preserve the behaviour for
   *   1:1 parity; a fix lands when D8 (`AddAccount` /
   *   `ChangeAccount` / `ManageAccount` D-step) re-evaluates the
   *   alias-vs-login interaction in scope.
   * - `password` honours `rememberPassword` (empty string when
   *   off) — WPF L1353-1356 ternary.
   * - `verify` honours `rememberVerify` (empty string when off) —
   *   WPF L1357 ternary.
   * - `method` is the active `LoginMethod` numeric value — WPF
   *   L1358 (`App.LoginMethod`).
   * - `auto_login` is the `AutoLogin` checkbox state — WPF
   *   L1359.
   *
   * # Skip rules (when this action returns without writing)
   *
   * 1. `method === GamePass` — WPF `GamePassLoginCompleted`
   *    (L1316-1332) bypasses `OnLoginCompleted` entirely, so
   *    `SaveLoginCredentials` never fires for GamePass logins.
   *    We mirror by short-circuiting here rather than relying on
   *    every call site to filter.
   * 2. `method === QrCode` for **either region** — TW WPF
   *    explicitly skips (`isAccountLogin = App.LoginRegion != "TW"
   *    || App.LoginMethod != QRCode`, L1336-1342); HK WPF *tries*
   *    to write but reads the form's empty `t_AccountID.Text`
   *    field (the QR flow never populates id), producing a
   *    garbage `(HK, "")` record that downstream WPF code
   *    explicitly works around via `Math.Min` floor on the
   *    method enum (`MainWindow.xaml.cs` L375-385). P12.2 D2
   *    pre-flight Q1 = B retires the dead write — the effective
   *    behaviour is identical to WPF (boot method restoration
   *    floor + IdPassForm dropdown empty-row workaround both
   *    no-op on the absence of the row), with strictly cleaner
   *    UX (no empty dropdown row, no hidden `App.LoginMethod`
   *    drift).
   *
   * Both skip rules return early **without** clearing
   * {@link useAuthStore.loginIntent} / {@link useAuthStore.verifyIntent}
   * — the intent slots are owned by the caller (the form
   * component that knows whether the flow is finished or just
   * re-entering); centralising the clear here would rip the
   * carpet out from under multi-step flows.
   *
   * # Why this lives in `account.ts` not `auth.ts`
   *
   * WPF puts the persistence inline in `MainWindow` because both
   * the auth state and the record store are direct fields on the
   * same partial class. Splitting them across two Pinia stores
   * means the persistence has to live in exactly one of them;
   * `account.ts` is the right home (Q3 = A in pre-flight) because
   * (a) the action mutates `accounts.value` (this store's
   * authoritative state), (b) the action's failure modes are
   * `storage.*` errors that match this store's `wrapCommand`
   * surface, and (c) `auth.ts` deliberately avoids cross-store
   * dependencies (it would need to import this store, breaking
   * the SRP boundary).
   *
   * @returns the updated `accounts` list (also assigned to
   *   `accounts.value` for reactive consumers).
   */
  async function saveLoginCredentials(input: SaveLoginCredentialsInput): Promise<Account[]> {
    if (input.method === LOGIN_METHOD.GamePass) return accounts.value
    if (input.method === LOGIN_METHOD.QrCode) return accounts.value

    const password = input.rememberPassword ? input.password : ''
    const verify = input.rememberVerify ? input.verify : ''
    const account: Account = {
      region: input.region,
      account_id: input.accountId,
      account_name: '',
      password,
      verify,
      method: input.method,
      auto_login: input.autoLogin,
    }
    return saveAccount(account)
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

  /* --------------- service-account ordering (D7) --------------- */

  /**
   * # WPF parity
   *
   * Mirrors `Beanfun/Pages/AccountList.xaml.cs::ApplyAccountOrder`
   * (L489-531) **without** the disk-IO pass — this action takes a
   * pre-resolved sid list and reorders `serviceAccounts.value`
   * in place. The two pieces of logic this preserves:
   *
   * 1. **Honour `orderedSids` for known sids** (L511-520). Walk the
   *    saved order, take each sid's account from the current store
   *    and emit it in saved-order. Sids in `orderedSids` that the
   *    store doesn't recognise (e.g. a stale row removed from
   *    Beanfun mid-session) are **silently skipped** — same as
   *    WPF's `accountDict.ContainsKey` guard.
   * 2. **Append the un-ordered tail** (L522-526). Any account
   *    currently in the store but **missing** from `orderedSids`
   *    (e.g. a brand-new row from `addServiceAccount` that the
   *    saved order doesn't yet know about) is appended in its
   *    pre-existing relative order so it never silently disappears
   *    from the UI.
   *
   * The mirroring of WPF's "skip unknown / append unordered"
   * invariants is the entire correctness guarantee of this action;
   * the order in `serviceAccounts.value` is therefore **always a
   * permutation of the same set**, never a subset.
   *
   * # Why not cross-import auth + config stores
   *
   * The WPF call site (`SaveAccountOrder` / `ApplyAccountOrder`)
   * derives `gameCode` from `App.MainWnd.service_code +
   * service_region` and reads / writes `Config.xml` directly. The
   * Pinia equivalent would need to import {@link useAuthStore}
   * (for `session.service_code`) and {@link useConfigStore} (for
   * the persisted CSV). That cross-store coupling would:
   *
   * - violate the SRP boundary documented at the top of this file
   *   ("the account store has no flow-continuation special cases"),
   * - make this store harder to test (every spec would need to
   *   bootstrap two extra stores just to exercise the ordering
   *   logic), and
   * - put the gameCode derivation behind one extra layer of
   *   indirection vs. WPF (which derives it inline at the call
   *   site).
   *
   * The page (`pages/AccountList.vue`) owns both stores already
   * (it computes `gameCode` for D5 specialClick and reads
   * `configStore` for D5 autoPaste), so passing the resolved CSV
   * down as a string is strictly cleaner.
   *
   * @param orderedSids — sids in the desired display order.
   * @returns the new `serviceAccounts` value (also assigned to the
   *   reactive ref for consumers that prefer to watch).
   */
  function setServiceAccountOrder(orderedSids: readonly string[]): ServiceAccount[] {
    if (serviceAccounts.value.length === 0) return serviceAccounts.value

    const remaining = new Map<string, ServiceAccount>()
    for (const account of serviceAccounts.value) {
      remaining.set(account.sid, account)
    }

    const next: ServiceAccount[] = []
    for (const sid of orderedSids) {
      const account = remaining.get(sid)
      if (account === undefined) continue
      next.push(account)
      remaining.delete(sid)
    }

    /*
     * Append the tail in the store's original relative order
     * (Map iteration order = insertion order, which matches the
     * `serviceAccounts` array order at function entry). WPF L523-525
     * iterates `accountDict.Values` whose ordering happens to also
     * match insertion order in .NET's Dictionary today, but that
     * isn't documented; using a Map here makes the invariant
     * explicit and grep-able.
     */
    for (const account of remaining.values()) {
      next.push(account)
    }

    serviceAccounts.value = next
    return next
  }

  /**
   * Convenience wrapper that parses a comma-separated sid list
   * (the format WPF persists under `Config.xml::AccountOrder_<gameCode>`,
   * see `AccountList.xaml.cs::SaveAccountOrder` L485-486) and
   * forwards to {@link setServiceAccountOrder}.
   *
   * `undefined` / empty / whitespace-only input is a no-op so
   * callers can pass `configStore.get(key)` directly without a
   * pre-check (WPF L497-499 has the same `IsNullOrEmpty` guard).
   *
   * @returns the new `serviceAccounts` value, or the existing one
   *   when the CSV is empty / undefined.
   */
  function applyServiceAccountOrderFromSavedCsv(csv: string | undefined): ServiceAccount[] {
    if (csv === undefined || csv.trim() === '') return serviceAccounts.value
    const sids = csv.split(',').filter((s) => s !== '')
    if (sids.length === 0) return serviceAccounts.value
    return setServiceAccountOrder(sids)
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
    findStoredAccount,
    saveLoginCredentials,

    serviceAccounts,
    amountLimitNotice,
    selectedSid,
    selectedServiceAccount,
    getServiceAccounts,
    refreshServiceAccounts,
    addServiceAccount,
    changeServiceAccountName,
    getOtp,
    setServiceAccountOrder,
    applyServiceAccountOrderFromSavedCsv,

    email,
    remainPoint,
    contract,
    getEmail,
    getRemainPoint,
    getContract,

    clearSessionData,
  }
})
