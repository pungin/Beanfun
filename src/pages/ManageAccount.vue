<script setup lang="ts">
/**
 * Stored-credential management page (P12.2 D9).
 *
 * # WPF parity
 *
 * Mirrors `Beanfun/Pages/ManageAccount.xaml(.cs)` — the surface that
 * lets the user add / edit / remove the Beanfun login records persisted
 * in `Users.dat` (DPAPI-encrypted) plus import / export the whole list
 * to / from disk. WPF rendered this as a `Page` hosted inside
 * `Setting.xaml`; the SPA exposes it via a top-level
 * `/manage-account` route registered in `router/index.ts` (the entry
 * button itself lands in P12.4 alongside the rest of the Settings
 * surface — see Q10 in the Todo.md D9 design table for why we don't
 * wire an entry from `AccountList.vue`).
 *
 * # 12 design Q&A — what differs from WPF (all approved by user)
 *
 * | # | Decision | Why it differs from WPF |
 * |---|----------|------------------------|
 * | Q1 | **Single table + region chip** instead of WPF's two TW/HK tabs | Mockup direction; lets one search filter (Q6) and one stats card (Q7) cover both regions in one render pass; WPF's `btn_TW` / `btn_HK` toggle was a `ListView` filter workaround, not a UX decision |
 * | Q2 | **Backend `Account` has no `last_login_at`** — the column renders `—` everywhere | The schema would need an additive Users.dat migration; out of scope for D9 |
 * | Q3 | **Drag handle is rendered but disabled** (`cursor: not-allowed` + tooltip) | The Rust `commands::save_account` upserts by `(region, account_id)` and has no indexed-insert API; reorder needs a dedicated backend D-step (`reorder_accounts(orderedKeys)`) and persistence policy first |
 * | Q4 | **Plaintext file picker** for import / export (mockup parity); the WPF AES-recovery flow lives in D10 | Backend already ships `import_records(path)` / `export_records(path)` plaintext JSON commands; AES backup is a separate UX concern (cross-Windows-user portability) handled by `windows/AccRecovery.vue` in D10 |
 * | Q5 | Uses `@tauri-apps/plugin-dialog` for native open/save | Tauri v2 standard file picker; plugin wired in D9.0 (Cargo.toml + lib.rs + capabilities) |
 * | Q6 | **Search box** filters `account_id` + `account_name` (case-insensitive contains) | Mockup adds it; WPF had no search; UX improvement for users with 5+ stored accounts |
 * | Q7 | **Single stats card "Stored accounts"** — drop the mockup's DPAPI / storage-location cards | Those were chrome describing implementation details, not actionable info; "how many accounts" is the only stat that helps the user |
 * | Q8 | **Edit + Copy ID + Delete** row icons all wired | Mockup has all three; Copy ID uses `navigator.clipboard.writeText` matching the D5 OTP-copy pattern |
 * | Q9 | **Single delete only** (per-row); no multi-select / batch delete | WPF supported multi-select but mockup omits it; rare use case for a credential list (typically single deletion) |
 * | Q10 | **Register `/manage-account` route**, defer entry button to P12.4 Settings page | WPF entered from `Setting.xaml`; aligning the entry surface to its WPF parent keeps `AccountList.vue` stable and avoids D9 churn there |
 * | Q11 | **`ElMessageBox.confirm` for delete**, reusing WPF `MsgDeleteAccountMng` / `MsgDeleteAccountSingle` keys | 1:1 with WPF's `MessageBox.Show + YesNo` UX, just rendered as a Vue dialog instead of a Win32 modal |
 * | Q12 | **`ElMessageBox.confirm` for import overwrite** with new `manageAccount.importOverwriteConfirm` key | The backend `import_records` is a full-file overwrite (parses JSON and replaces every entry, not a merge); destructive-action guard is essential UX. WPF used the AES-password prompt as the implicit gate; the plaintext path needs its own explicit dialog |
 * | Q13 | **Back button in footer** (P12.4-followup-B-fix F6) — fallback `router.push('/settings')` (not `/login` like `Settings.vue` / `About.vue`) | The page had no back affordance at all in the original D9 ship — a regression caught during P12.4 smoke testing. ManageAccount has exactly one production entry point (`Settings.vue::handleManageAccount`), so the fallback target is `/settings` (mirrors that single relationship). Settings/About fall back to `/login` because they're reachable from the login funnel too — that branch does not apply here. Implementation mirrors `Settings.vue::handleBack` byte-for-byte (`window.history.length > 1` proxy + `router.back()` primary path); see `handleBack` docblock for the per-line rationale |
 *
 * # State machine — 4 list states
 *
 * Mirror `AccountList.vue` for consistency:
 *
 * 1. `loading` — first paint while {@link useAccountStore.loadAccounts}
 *    is in-flight. The toolbar / search / add button are visible but
 *    the list area shows a status placeholder.
 * 2. `error` — load failed. Inline retry button reruns
 *    `loadAccounts`. The store's `wrapCommand` already toasted the
 *    raw `CommandError`; the inline message + retry is the user's
 *    actionable affordance.
 * 3. `empty` — load succeeded with `accounts.length === 0`. Renders
 *    the "no stored accounts yet" CTA pointing at the add button.
 * 4. `ready` — populated. Search-filtered to `searchedAccounts`; if
 *    that is empty the in-list placeholder switches to
 *    `noSearchResult` to differentiate "you don't have accounts" from
 *    "your search returns nothing".
 *
 * # Reactive data flow
 *
 * - `account.accounts` (Pinia setup-store ref, auto-unwrapped on
 *   property access) is the source of truth. Every mutation funnels
 *   through the store (`saveAccount` / `removeAccount` /
 *   `importRecords` / `exportRecords`); we never re-call
 *   `loadAccounts` after a mutation because the store re-assigns
 *   `accounts.value` to the freshly-returned list.
 * - `searchedAccounts` is a `computed` over `accounts` + `query`;
 *   pure local filter, no IPC.
 * - The two dialogs are mounted unconditionally (mount-always
 *   pattern from D3 / D4) so their open / close transitions can play;
 *   visibility is driven by `addVisible` / `editVisible` refs and the
 *   `editTarget` snapshot is reset to `null` via a `watch(editVisible)`
 *   on every `true → false` transition (mirrors the same parent-owned
 *   release pattern `AccountList.vue` uses for `changeAliasTarget` —
 *   `ChangeAccount.vue` itself does not re-emit `@closed`, so a parent
 *   watcher is the only race-free hook to clear the reference).
 *
 * # Why a Page, not a Window
 *
 * D8 dialogs (`AddAccount` / `ChangeAccount`) are modals because they
 * are short-lived form interactions. ManageAccount is a long-lived
 * surface — search / scroll / multi-action — so it gets a full route.
 * A `tauri::Window` would also work but introduces native window
 * lifecycle concerns (focus, multi-monitor placement, close-X); a
 * router page reuses the existing window chrome (P12.4 TitleBar) and
 * the same back / forward navigation any other page gets for free.
 */

import { computed, onMounted, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useRouter } from 'vue-router'
import { ElButton, ElIcon, ElInput, ElMessage, ElMessageBox } from 'element-plus'
import {
  ArrowLeft,
  Delete,
  DocumentCopy,
  Download,
  EditPen,
  InfoFilled,
  Lock,
  Plus,
  Rank,
  Search,
  Upload,
  UserFilled,
} from '@element-plus/icons-vue'
import { open as openFileDialog, save as saveFileDialog } from '@tauri-apps/plugin-dialog'

import AccRecovery from '../windows/AccRecovery.vue'
import AddAccount from '../windows/AddAccount.vue'
import ChangeAccount from '../windows/ChangeAccount.vue'
import { useAccountStore } from '../stores/account'
import type { Account } from '../types/bindings'
import TitleBar from '../components/TitleBar.vue'

defineOptions({ name: 'ManageAccount' })

const { t } = useI18n()
const router = useRouter()
const account = useAccountStore()

/* --------------- list-state machine --------------- */

type LoadState = 'loading' | 'error' | 'empty' | 'ready'

const loadState = ref<LoadState>('loading')
const loadError = ref<string | null>(null)

/**
 * Initial load + reload-after-error trigger. The store's
 * {@link useAccountStore.loadAccounts} funnels through `wrapCommand`,
 * which surfaces a toast on failure and re-throws — we catch here so
 * the in-page error UI can render with the message and the user gets
 * an inline retry affordance (matches the WPF "MessageBox + remain on
 * page" recovery pattern).
 */
async function loadList(): Promise<void> {
  loadState.value = 'loading'
  loadError.value = null
  try {
    const list = await account.loadAccounts()
    loadState.value = list.length === 0 ? 'empty' : 'ready'
  } catch (err) {
    loadError.value = err instanceof Error ? err.message : null
    loadState.value = 'error'
  }
}

onMounted(() => {
  void loadList()
})

/* --------------- search filter (Q6) --------------- */

const query = ref('')

/**
 * Lower-cased query, computed once per typed character to avoid
 * re-`toLowerCase()`-ing on every list iteration in
 * {@link searchedAccounts}.
 */
const normalizedQuery = computed((): string => query.value.trim().toLowerCase())

/**
 * Filtered view over `account.accounts`. Empty query short-circuits
 * to the full list (no copy) so the common case stays cheap.
 *
 * Search semantics: case-insensitive substring against
 * `account_id` + `account_name` only. Region chip text is intentionally
 * **not** searched (typing "TW" would match every TW account, which
 * is noise — region filtering belongs to a future filter chip if the
 * dataset grows large enough to need one).
 */
const searchedAccounts = computed((): Account[] => {
  const q = normalizedQuery.value
  if (q === '') return account.accounts
  return account.accounts.filter((acc) => {
    return acc.account_id.toLowerCase().includes(q) || acc.account_name.toLowerCase().includes(q)
  })
})

/**
 * Stats card value — bound to the unfiltered store list so the
 * "you have 5 accounts" reading does not drop to "0" mid-search.
 */
const totalAccounts = computed((): number => account.accounts.length)

/* --------------- dialog wiring (Add + Edit + AccRecovery) --------------- */

const addVisible = ref(false)
const editVisible = ref(false)
/**
 * Drives the AES backup / restore dialog (`windows/AccRecovery.vue`,
 * P12.2 D10.3). The dialog sits next to the plaintext Import / Export
 * file pickers because the two surfaces address different
 * portability concerns:
 *
 * - **Plaintext export (D9 Q4)**: file-on-disk JSON; meant for
 *   inspection / git / human review. Reveals every credential.
 * - **AES backup (D10)**: base64 ciphertext copy/pasteable via any
 *   transport channel; meant for cross-machine portability while
 *   protecting the password fields with a user-supplied passphrase.
 *
 * The two stay separate so users have an explicit choice about
 * confidentiality. WPF kept them separate too (`Import` / `Export`
 * buttons → file picker; `DataBackup` button → AES dialog).
 */
const accRecoveryVisible = ref(false)
/**
 * Snapshot of the account to edit. Cleared via the visibility
 * watcher below (matches the `AccountList.vue::changeAliasTarget`
 * pattern: a parent-owned `watch` releases the snapshot on every
 * `true → false` transition without piggy-backing on Vue's internal
 * v-model listener-merge behaviour). The dialog's own `@closed`
 * hook resets its form state internally; this watcher is only
 * responsible for releasing the parent-owned reference.
 */
const editTarget = ref<Account | null>(null)

watch(editVisible, (next) => {
  if (!next) editTarget.value = null
})

function openAdd(): void {
  addVisible.value = true
}

/**
 * Mirror of WPF `DataBackup_Click` (`ManageAccount.xaml.cs`):
 * open the AES backup / restore dialog. The dialog itself owns the
 * password / ciphertext form state and clears it on close so we
 * don't need to pre-seed anything from the parent.
 */
function openBackup(): void {
  accRecoveryVisible.value = true
}

/**
 * Re-derive the list state after a successful restore. The dialog
 * has already updated `account.accounts` in the store (avoiding a
 * redundant IPC round-trip), so we only have to flip the in-page
 * load-state machine to reflect the post-restore size of the list.
 *
 * Mirrors WPF's `App.MainWnd.loginMethodInit()` post-restore call:
 * the list re-renders, no extra Users.dat fetch.
 */
function handleRestored(): void {
  loadState.value = account.accounts.length === 0 ? 'empty' : 'ready'
}

/**
 * Mirror of WPF `Change_Button_Click` (`ManageAccount.xaml.cs`):
 * snapshot the row, open the dialog. The dialog mutates only
 * `account_name` + `auto_login` (per D8 Q5 = mockup parity:
 * `account_id` is read-only) and emits `updated` on save which
 * we surface as a success toast.
 */
function openEdit(target: Account): void {
  editTarget.value = target
  editVisible.value = true
}

/* --------------- row actions (delete + copy id) --------------- */

/**
 * Mirror of WPF `Delete_Button_Click` confirm flow. WPF used
 * `MessageBox.Show(MsgDeleteAccountMng + MsgDeleteAccountSingle("..."), ..., YesNo)`;
 * we render the same pair via `ElMessageBox.confirm`. Cancellation
 * is a no-op (matches WPF's `if (result == No) return`); confirmation
 * removes the row through the store, which re-`accounts.value`-assigns
 * the post-delete list so the UI re-renders without an explicit
 * reload call.
 */
async function handleDelete(target: Account): Promise<void> {
  /*
   * vue-i18n list interpolation: the WPF locale strings use `{0}`
   * as positional placeholder syntax, which vue-i18n resolves via
   * **array** values (`[arg]`) — not object form (`{ 0: arg }`),
   * which would silently leave the literal `{0}` in the rendered
   * string. Same call shape as `ServiceAccountInfo::CreateDate`.
   * The spec asserts the rendered message contains `target.account_id`
   * to guard against this bug class regressing.
   */
  const subject = t('MsgDeleteAccountSingle', [target.account_id])
  try {
    await ElMessageBox.confirm(t('MsgDeleteAccountMng', [subject]), t('DeleteAccount'), {
      confirmButtonText: t('Yes'),
      cancelButtonText: t('No'),
      type: 'warning',
    })
  } catch {
    /*
     * `ElMessageBox.confirm` rejects on cancel / Esc / outside-click.
     * Swallow silently — matches WPF MessageBox.NoButton behaviour.
     */
    return
  }

  try {
    await account.removeAccount(target.region, target.account_id)
    /*
     * Recompute the empty/ready transition: the store updated
     * `accounts.value` to the post-delete list; if that emptied the
     * list we need to flip back to the empty state so the in-list
     * placeholder takes over.
     */
    loadState.value = account.accounts.length === 0 ? 'empty' : 'ready'
  } catch {
    /* Toasted by `wrapCommand`. */
  }
}

/**
 * Copy the account ID to the system clipboard. Mockup adds this
 * affordance (WPF had no equivalent); we surface a success toast
 * matching the D5 `clipboardWriteOtp` flow's UX.
 *
 * The browser `navigator.clipboard.writeText` API works inside
 * Tauri because the WebView2 webview exposes the standard clipboard
 * surface to the page. No tauri-plugin-clipboard needed for this
 * write-only path.
 */
async function handleCopyId(target: Account): Promise<void> {
  try {
    await navigator.clipboard.writeText(target.account_id)
    ElMessage.success(t('manageAccount.idCopied'))
  } catch (err) {
    /*
     * Clipboard write can fail on some Windows session contexts
     * (RDP, locked screen). Surface the platform error verbatim so
     * the user has actionable info without the SPA inventing copy.
     */
    const msg = err instanceof Error ? err.message : String(err)
    ElMessage.error(msg)
  }
}

/* --------------- import / export (Q4 + Q5) --------------- */

const importing = ref(false)
const exporting = ref(false)

/**
 * Plaintext-JSON import via `tauri-plugin-dialog::open`. Three
 * branching points the user can land on:
 *
 * 1. **User cancels the file picker** (`open(...)` returns `null`)
 *    → silent no-op, no toast (matches WPF dialog-cancel UX).
 * 2. **User picks a file but cancels the overwrite confirm**
 *    (`ElMessageBox.confirm` rejects) → silent no-op.
 * 3. **Confirm + import** → store call → success toast + state
 *    refresh. Failure is toasted by `wrapCommand`.
 *
 * The overwrite confirm (Q12) exists because the backend
 * `commands::import_records` is a full-file overwrite — a successful
 * import wipes every previously-stored credential. WPF gated this
 * implicitly via the AES password prompt; the plaintext D9 path
 * needs an explicit confirmation.
 */
async function handleImport(): Promise<void> {
  if (importing.value) return

  let selected: string | string[] | null
  try {
    selected = await openFileDialog({
      multiple: false,
      directory: false,
      filters: [{ name: 'JSON', extensions: ['json'] }],
    })
  } catch (err) {
    const msg = err instanceof Error ? err.message : String(err)
    ElMessage.error(msg)
    return
  }
  if (selected === null || Array.isArray(selected)) return

  try {
    await ElMessageBox.confirm(
      t('manageAccount.importOverwriteConfirm'),
      t('manageAccount.importOverwriteConfirmTitle'),
      {
        confirmButtonText: t('Yes'),
        cancelButtonText: t('No'),
        type: 'warning',
      },
    )
  } catch {
    return
  }

  importing.value = true
  try {
    await account.importRecords(selected)
    loadState.value = account.accounts.length === 0 ? 'empty' : 'ready'
    ElMessage.success(t('manageAccount.importSuccess'))
  } catch {
    /* Toasted by `wrapCommand`. */
  } finally {
    importing.value = false
  }
}

/**
 * Plaintext-JSON export via `tauri-plugin-dialog::save`. The
 * `defaultPath` filename is hoisted into the locale tree
 * (`exportDefaultFilename`) so en-US users see an English suggestion
 * rather than a Chinese-locale literal that WPF hard-coded into
 * `Beanfun.json`. User cancellation (`save` returns `null`) is a
 * silent no-op; failure is toasted by `wrapCommand`; success surfaces
 * a confirmation toast.
 */
async function handleExport(): Promise<void> {
  if (exporting.value) return

  let target: string | null
  try {
    target = await saveFileDialog({
      defaultPath: t('manageAccount.exportDefaultFilename'),
      filters: [{ name: 'JSON', extensions: ['json'] }],
    })
  } catch (err) {
    const msg = err instanceof Error ? err.message : String(err)
    ElMessage.error(msg)
    return
  }
  if (target === null) return

  exporting.value = true
  try {
    await account.exportRecords(target)
    ElMessage.success(t('manageAccount.exportSuccess'))
  } catch {
    /* Toasted by `wrapCommand`. */
  } finally {
    exporting.value = false
  }
}

/* --------------- presentation helpers --------------- */

/**
 * First non-empty character of the account_id, uppercased. Drives
 * the avatar bubble glyph. Empty / whitespace-only IDs (which the
 * AddAccount dialog blocks via `AccountNeed`, so should never
 * happen in practice) fall back to `?` so the bubble renders rather
 * than collapsing to an empty span.
 */
function avatarGlyph(target: Account): string {
  const trimmed = target.account_id.trim()
  if (trimmed.length === 0) return '?'
  return trimmed.charAt(0).toUpperCase()
}

/**
 * Region label hoisted into a helper so the chip + dialog stay
 * phrasing-aligned with the WPF locale's `Taiwan` / `HongKong`
 * strings. Anything outside `TW` / `HK` falls through to the raw
 * region code rather than a hard error — defensive against future
 * `Account.region` schema additions (e.g. `JP`).
 */
function regionLabel(region: string): string {
  if (region === 'TW') return t('Taiwan')
  if (region === 'HK') return t('HongKong')
  return region
}

/* --------------- back navigation --------------- */

/**
 * Returns the user to the previous route, mirroring the same
 * `router.back()` idiom that `Settings.vue::handleBack` and
 * `About.vue::handleBack` use. Originally missing — without this
 * the page was a dead-end (P12.4-followup-B-fix F6 audit trail).
 *
 * # Why fallback to `/settings`, not `/login`
 *
 * Settings and About are both reachable from **two** parents
 * (the login funnel and `AccountList`), so they fall back to
 * `/login` when history is empty. ManageAccount has exactly one
 * production entry point: `Settings.vue` (`router.push('/manage-account')`).
 * The route is also `requiresAuth: true`, so falling back to
 * `/login` would either kick the user out of their session UX
 * unnecessarily (if still authenticated) or trigger the auth
 * guard's `?redirect=/login` self-loop (if expired). `/settings`
 * is the canonical re-entry surface in both cases — and in the
 * unlikely event of an expired session, the auth guard on
 * `/settings` … wait, `/settings` is `requiresAuth: undefined`,
 * so it's public; the user lands there cleanly either way.
 *
 * # Edge case: direct hash entry
 *
 * `window.history.length === 1` means the page was opened
 * directly via `#/manage-account` (devtools / external link).
 * Vue-router does not expose a "can go back" predicate, so we
 * use the `window.history.length` proxy — same heuristic
 * `Settings.vue::handleBack` uses. The check is best-effort.
 */
function handleBack(): void {
  if (window.history.length > 1) {
    router.back()
    return
  }
  void router.push('/settings')
}
</script>

<template>
  <main class="manage bf-glass-panel" data-window-root>
    <TitleBar />
    <div class="manage__scroll">
      <div class="manage__container">
        <!-- Header -->
        <header class="manage__header">
          <div class="manage__header-icon" aria-hidden="true">
            <el-icon :size="24"><UserFilled /></el-icon>
          </div>
          <div class="manage__header-text">
            <h1 class="manage__title bf-text-gradient">{{ t('ManageAccount') }}</h1>
            <p class="manage__subline">{{ t('manageAccount.subtitle') }}</p>
          </div>
        </header>

        <!-- Toolbar: search + import / export / add -->
        <section class="manage__toolbar bf-glass-panel">
          <div class="manage__toolbar-search">
            <el-input
              v-model="query"
              :placeholder="t('manageAccount.searchPlaceholder')"
              :disabled="loadState === 'loading'"
              clearable
              data-test="manage-account-search"
            >
              <template #prefix>
                <el-icon><Search /></el-icon>
              </template>
            </el-input>
          </div>
          <div class="manage__toolbar-actions">
            <el-button
              class="bf-btn-secondary manage__action-btn"
              :disabled="importing"
              data-test="manage-account-import"
              @click="handleImport"
            >
              <el-icon><Upload /></el-icon>
              <span>{{ t('manageAccount.import') }}</span>
            </el-button>
            <el-button
              class="bf-btn-secondary manage__action-btn"
              :disabled="exporting || account.accounts.length === 0"
              data-test="manage-account-export"
              @click="handleExport"
            >
              <el-icon><Download /></el-icon>
              <span>{{ t('manageAccount.export') }}</span>
            </el-button>
            <el-button
              class="bf-btn-secondary manage__action-btn"
              data-test="manage-account-backup"
              @click="openBackup"
            >
              <el-icon><Lock /></el-icon>
              <span>{{ t('DataBackup') }}</span>
            </el-button>
            <button
              type="button"
              class="bf-btn-gradient manage__action-btn"
              data-test="manage-account-add"
              @click="openAdd"
            >
              <el-icon><Plus /></el-icon>
              <span>{{ t('Add') }}</span>
            </button>
          </div>
        </section>

        <!-- Stats: single card (Q7) -->
        <section class="manage__stats">
          <div class="manage__stat-card bf-glass-card bf-ghost-border">
            <el-icon class="manage__stat-icon" :size="20"><UserFilled /></el-icon>
            <div class="manage__stat-text">
              <span class="manage__stat-label">
                {{ t('manageAccount.totalAccounts') }}
              </span>
              <span class="manage__stat-value" data-test="manage-account-total">
                {{ totalAccounts }}
              </span>
            </div>
          </div>
        </section>

        <!-- Table -->
        <section class="manage__list bf-glass-panel">
          <div class="manage__list-header">
            <div class="manage__col-handle" />
            <div class="manage__col-account">{{ t('manageAccount.colAccount') }}</div>
            <div class="manage__col-remark">{{ t('manageAccount.colRemark') }}</div>
            <div class="manage__col-region">{{ t('manageAccount.colRegion') }}</div>
            <div class="manage__col-last">{{ t('manageAccount.colLastLogin') }}</div>
            <div class="manage__col-actions">{{ t('manageAccount.colActions') }}</div>
          </div>

          <div class="manage__list-body bf-custom-scrollbar">
            <p
              v-if="loadState === 'loading'"
              class="manage__list-state"
              data-test="manage-account-loading"
            >
              {{ t('accountList.loading') }}
            </p>

            <div
              v-else-if="loadState === 'error'"
              class="manage__list-state manage__list-state--error"
              data-test="manage-account-error"
            >
              <p>{{ loadError ?? t('accountList.loadFailed') }}</p>
              <el-button
                type="primary"
                plain
                size="small"
                data-test="manage-account-retry"
                @click="loadList"
              >
                {{ t('accountList.retry') }}
              </el-button>
            </div>

            <p
              v-else-if="loadState === 'empty'"
              class="manage__list-state"
              data-test="manage-account-empty"
            >
              {{ t('manageAccount.empty') }}
            </p>

            <p
              v-else-if="searchedAccounts.length === 0"
              class="manage__list-state"
              data-test="manage-account-no-search-result"
            >
              {{ t('manageAccount.noSearchResult') }}
            </p>

            <div
              v-for="(row, index) in searchedAccounts"
              v-else
              :key="`${row.region}|${row.account_id}`"
              class="manage__row"
              :class="{ 'manage__row--last': index === searchedAccounts.length - 1 }"
              :data-test="`manage-account-row-${row.region}-${row.account_id}`"
            >
              <span
                class="manage__drag-handle"
                :title="t('manageAccount.dragDisabledTip')"
                data-test="manage-account-drag-handle"
              >
                <el-icon><Rank /></el-icon>
              </span>
              <div class="manage__cell-account">
                <div class="manage__avatar" aria-hidden="true">{{ avatarGlyph(row) }}</div>
                <span class="manage__account-id">{{ row.account_id }}</span>
              </div>
              <div class="manage__cell-remark">
                <span v-if="row.account_name.trim().length > 0">{{ row.account_name }}</span>
                <span v-else class="manage__remark-empty">
                  {{ t('manageAccount.remarkEmpty') }}
                </span>
              </div>
              <div class="manage__cell-region">
                <span
                  class="manage__chip"
                  :class="{ 'manage__chip--primary': row.region === 'TW' }"
                >
                  {{ regionLabel(row.region) }}
                </span>
              </div>
              <div class="manage__cell-last">
                {{ t('manageAccount.lastLoginUnknown') }}
              </div>
              <div class="manage__cell-actions">
                <button
                  type="button"
                  class="manage__icon-btn"
                  :title="t('manageAccount.editAction')"
                  :data-test="`manage-account-edit-${row.region}-${row.account_id}`"
                  @click="openEdit(row)"
                >
                  <el-icon><EditPen /></el-icon>
                </button>
                <button
                  type="button"
                  class="manage__icon-btn"
                  :title="t('manageAccount.copyIdAction')"
                  :data-test="`manage-account-copy-${row.region}-${row.account_id}`"
                  @click="handleCopyId(row)"
                >
                  <el-icon><DocumentCopy /></el-icon>
                </button>
                <button
                  type="button"
                  class="manage__icon-btn manage__icon-btn--danger"
                  :title="t('manageAccount.deleteAction')"
                  :data-test="`manage-account-delete-${row.region}-${row.account_id}`"
                  @click="handleDelete(row)"
                >
                  <el-icon><Delete /></el-icon>
                </button>
              </div>
            </div>
          </div>
        </section>

        <!--
        Footer: encryption hint (left) + Back button (right).
        Hint and back button share the same flex row so the
        layout collapses gracefully on narrow widths (the back
        button hugs the right edge while the hint wraps left).
        Back action mirrors `Settings.vue` / `About.vue`
        (P12.4-followup-B-fix F6 — without this the page had no
        navigation affordance back to Settings).
      -->
        <footer class="manage__footer">
          <div class="manage__footer-hint">
            <el-icon><InfoFilled /></el-icon>
            <span>{{ t('manageAccount.footerHint') }}</span>
          </div>
          <el-button
            class="bf-btn-secondary manage__back-btn"
            data-test="manage-account-back"
            @click="handleBack"
          >
            <el-icon><ArrowLeft /></el-icon>
            <span>{{ t('Back') }}</span>
          </el-button>
        </footer>

        <!-- Add / Edit / Recovery dialogs — mounted unconditionally for transitions. -->
        <AddAccount v-model:visible="addVisible" />
        <ChangeAccount v-model:visible="editVisible" :account="editTarget" />
        <AccRecovery v-model:visible="accRecoveryVisible" @restored="handleRestored" />
      </div>
    </div>
  </main>
</template>

<style scoped>
.manage {
  display: flex;
  flex-direction: column;
  overflow: hidden;
  background:
    linear-gradient(180deg, rgba(255, 255, 255, 0.55) 0%, rgba(255, 255, 255, 0.45) 100%),
    radial-gradient(
      1200px 800px at 10% -10%,
      color-mix(in srgb, var(--bf-primary-container) 30%, transparent) 0%,
      transparent 60%
    ),
    radial-gradient(
      900px 700px at 110% 110%,
      color-mix(in srgb, var(--bf-primary) 22%, transparent) 0%,
      transparent 55%
    ),
    linear-gradient(180deg, #f7f1ec 0%, #ece1d6 100%);
}

.manage__scroll {
  flex: 1;
  overflow-y: auto;
  padding: 1.5rem;
}

.manage__container {
  width: 100%;
  max-width: 880px;
  display: flex;
  flex-direction: column;
  gap: 1rem;
}

/* --------------- header --------------- */

.manage__header {
  display: flex;
  align-items: center;
  gap: 0.875rem;
  margin-bottom: 0.25rem;
}

.manage__header-icon {
  width: 44px;
  height: 44px;
  border-radius: var(--bf-radius-button);
  background: linear-gradient(
    135deg,
    color-mix(in srgb, var(--bf-primary-container) 30%, transparent),
    color-mix(in srgb, var(--bf-primary) 25%, transparent)
  );
  color: var(--bf-on-primary);
  display: grid;
  place-items: center;
  flex-shrink: 0;
  box-shadow: var(--bf-shadow-card);
}

.manage__header-text {
  min-width: 0;
}

.manage__title {
  margin: 0;
  font-size: 1.625rem;
  font-weight: 800;
  letter-spacing: -0.01em;
  line-height: 1.15;
}

.manage__subline {
  margin: 0.25rem 0 0;
  font-size: 0.8125rem;
  color: var(--bf-on-surface-variant);
}

/* --------------- toolbar --------------- */

.manage__toolbar {
  padding: 0.875rem;
  display: flex;
  align-items: center;
  gap: 0.75rem;
}

.manage__toolbar-search {
  flex: 1;
  max-width: 320px;
}

.manage__toolbar-actions {
  margin-left: auto;
  display: flex;
  align-items: center;
  gap: 0.5rem;
}

.manage__action-btn {
  display: inline-flex;
  align-items: center;
  gap: 0.375rem;
  font-size: 0.8125rem;
}

/* --------------- stats --------------- */

.manage__stats {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
  gap: 0.75rem;
}

.manage__stat-card {
  padding: 0.875rem 1rem;
  display: flex;
  align-items: center;
  gap: 0.75rem;
}

.manage__stat-icon {
  color: var(--bf-primary);
  flex-shrink: 0;
}

.manage__stat-text {
  display: flex;
  flex-direction: column;
  gap: 0.125rem;
  min-width: 0;
}

.manage__stat-label {
  font-size: 0.75rem;
  color: var(--bf-on-surface-variant);
}

.manage__stat-value {
  font-size: 1.125rem;
  font-weight: 700;
  color: var(--bf-on-surface);
}

/* --------------- list --------------- */

.manage__list {
  padding: 0;
  overflow: hidden;
}

.manage__list-header,
.manage__row {
  display: grid;
  grid-template-columns: 32px 1.5fr 1.5fr 110px 130px 130px;
  align-items: center;
  gap: 0.5rem;
  padding: 0.75rem 1rem;
}

.manage__list-header {
  font-size: 0.75rem;
  font-weight: 600;
  color: var(--bf-on-surface-variant);
  border-bottom: 1px solid color-mix(in srgb, var(--bf-on-surface) 8%, transparent);
  background: color-mix(in srgb, var(--bf-surface) 50%, transparent);
}

.manage__col-actions {
  text-align: right;
}

.manage__list-body {
  max-height: 480px;
  overflow-y: auto;
}

.manage__list-state {
  margin: 0;
  padding: 2.5rem 1.5rem;
  text-align: center;
  color: var(--bf-on-surface-variant);
  font-size: 0.875rem;
}

.manage__list-state--error {
  color: var(--bf-danger);
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 0.75rem;
}

.manage__row {
  border-bottom: 1px solid color-mix(in srgb, var(--bf-on-surface) 6%, transparent);
  transition: background var(--bf-motion-fast);
}

.manage__row:hover {
  background: color-mix(in srgb, var(--bf-primary-container) 8%, transparent);
}

.manage__row--last {
  border-bottom: 0;
}

.manage__drag-handle {
  display: grid;
  place-items: center;
  width: 24px;
  height: 24px;
  color: var(--bf-on-surface-variant);
  cursor: not-allowed;
  opacity: 0.55;
}

.manage__cell-account {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  min-width: 0;
}

.manage__avatar {
  width: 32px;
  height: 32px;
  border-radius: var(--bf-radius-pill);
  background: linear-gradient(135deg, var(--bf-primary-container), var(--bf-primary));
  color: var(--bf-on-primary);
  display: grid;
  place-items: center;
  font-weight: 700;
  font-size: 0.8125rem;
  flex-shrink: 0;
  box-shadow: var(--bf-shadow-card);
}

.manage__account-id {
  font-weight: 600;
  font-size: 0.875rem;
  color: var(--bf-on-surface);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.manage__cell-remark {
  font-size: 0.8125rem;
  color: var(--bf-on-surface);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.manage__remark-empty {
  font-style: italic;
  color: var(--bf-on-surface-variant);
}

.manage__chip {
  display: inline-block;
  font-size: 0.6875rem;
  padding: 0.125rem 0.5rem;
  border-radius: var(--bf-radius-pill);
  background: color-mix(in srgb, var(--bf-on-surface) 6%, transparent);
  color: var(--bf-on-surface-variant);
}

.manage__chip--primary {
  background: color-mix(in srgb, var(--bf-primary-container) 30%, transparent);
  color: var(--bf-primary);
  font-weight: 600;
}

.manage__cell-last {
  font-size: 0.8125rem;
  color: var(--bf-on-surface-variant);
  font-family: ui-monospace, 'SF Mono', Consolas, monospace;
}

.manage__cell-actions {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 0.25rem;
}

.manage__icon-btn {
  appearance: none;
  border: 1px solid transparent;
  background: transparent;
  width: 32px;
  height: 32px;
  display: grid;
  place-items: center;
  border-radius: var(--bf-radius-input);
  color: var(--bf-on-surface-variant);
  cursor: pointer;
  transition:
    background var(--bf-motion-fast),
    color var(--bf-motion-fast),
    border-color var(--bf-motion-fast);
}

.manage__icon-btn:hover {
  background: color-mix(in srgb, var(--bf-primary-container) 15%, transparent);
  border-color: color-mix(in srgb, var(--bf-primary) 20%, transparent);
  color: var(--bf-primary);
}

.manage__icon-btn--danger:hover {
  background: color-mix(in srgb, var(--bf-danger) 12%, transparent);
  border-color: color-mix(in srgb, var(--bf-danger) 30%, transparent);
  color: var(--bf-danger);
}

/* --------------- footer --------------- */

.manage__footer {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 1rem;
  padding: 0 0.25rem;
  flex-wrap: wrap;
}

.manage__footer-hint {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  font-size: 0.75rem;
  color: var(--bf-on-surface-variant);
  flex: 1 1 auto;
  min-width: 0;
}

.manage__footer-hint .el-icon {
  flex-shrink: 0;
}

.manage__back-btn {
  display: inline-flex;
  align-items: center;
  gap: 0.375rem;
  flex-shrink: 0;
}
</style>
