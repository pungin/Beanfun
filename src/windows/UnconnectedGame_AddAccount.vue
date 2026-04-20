<script setup lang="ts">
/**
 * Unconnected-game add-account dialog (P12.3 D6).
 *
 * # WPF parity
 *
 * Mirrors `Beanfun/Windows/UnconnectedGame_AddAccount.xaml(.cs)`,
 * the WPF dialog `MainWindow.btnAddServiceAccount_Click` opens
 * when the currently-selected game is one of the two
 * "unconnected" titles (see `UNCONNECTED_GAME_CODES` in
 * `stores/game.ts`):
 *
 * 1. Constructor calls `App.MainWnd.UnconnectedGame_AddAccountInit()`
 *    → in the SPA, [`commands.unconnectedGameInitAddAccountPayload`]
 *    fetches the {@link AddAccountInit} bundle (game name, account
 *    length range, `check_nickname_supported`, opaque session
 *    triplet). On null payload WPF showed `UnknownError` and closed;
 *    we mirror that exactly via `ElMessage.error(t('UnknownError'))`
 *    and `visible = false`.
 * 2. `payload.Get("CheckNickName") == ""` ⇒ DN row + "check
 *    nickname" hyperlink hidden. We branch on
 *    `init.check_nickname_supported` (the boolean the backend
 *    extracts from the same HTML probe).
 * 3. The dialog displays the game name in five different positions
 *    (intro paragraph + bullet 1 + DN row + pwd row × 2 + ToS link)
 *    by interpolating `init.game_name` into the corresponding
 *    `UnconnectedGame_AddAccount_*` localized strings — same key
 *    layout as WPF's five `<Run x:Name="lblGameName{N}"/>` slots.
 * 4. **Hyperlink "Check Account"** (`Hyperlink_Click`) →
 *    [`commands.unconnectedGameAddAccountCheck`] with the current
 *    session, `accountId`, and `accountDn` if the DN row is
 *    visible (WPF: `DNtr.Visibility == Visible ? "" : null`). The
 *    response always carries the **next** session triplet
 *    (we re-stash it into `mgmtSession`) plus a `lblErrorMessage`
 *    string. Empty string ⇒ WPF `UnknownError` toast (the WPF
 *    branch where the server returned no message at all);
 *    non-empty ⇒ inline `lblErrorMessage` display (WPF parity:
 *    the same red label that doubles as a success info channel —
 *    "帳號可使用" / "此帳號已被使用" both flow through here).
 * 5. **Hyperlink "Check Nickname"** (`Hyperlink_Click_1`) — same
 *    contract as above but invokes
 *    [`commands.unconnectedGameAddAccountCheckNickname`] (no
 *    accountId field, only DN). Skipped entirely when
 *    `check_nickname_supported === false` (mirrors WPF's early
 *    return in `Hyperlink_Click_1`).
 * 6. **Hyperlink "Service Contract"** (`Hyperlink_Click_2`) →
 *    [`useAccountStore.getContract`] → opens the nested
 *    `windows/Contract.vue` dialog (P12.2 D10.2). Empty contract
 *    surfaces `UnknownError` (WPF parity, same as
 *    `AddServiceAccount.vue`).
 * 7. **Submit** (`Button_Click`) runs the full WPF validation
 *    chain in the original order — every branch surfaces the
 *    matching `UnconnectedGame_AddAccount_{18..27}` resource via
 *    `ElMessage.warning`:
 *
 *    | Step | WPF resource | Trigger |
 *    | --- | --- | --- |
 *    | empty accountId | `_18` | `txtServiceAccountID.Text == ""` |
 *    | accountId length out of range | `_19` | `< accountLenMin || > accountLenMax` |
 *    | empty pwd | `_20` | `txtNewPwd.Password == ""` |
 *    | pwd length out of range | `_21` | `< accountLenMin || > accountLenMax` |
 *    | empty pwd2 | `_22` | `txtNewPwd2.Password == ""` |
 *    | pwd2 length out of range | `_23` | `< accountLenMin || > accountLenMax` |
 *    | DN visible: empty DN | `_24` | `txtServiceAccountDN.Text == ""` |
 *    | DN visible: DN length out of 2..6 | `_25` | hardcoded literal range |
 *    | terms unchecked | `_26` | `chkBox1.IsChecked != true` |
 *
 *    On all checks passing, [`commands.unconnectedGameAddAccount`]
 *    fires; the response is an [`AddAccountOutcome`] discriminated
 *    union:
 *
 *    - `kind === 'success'` ⇒ emit `created` and close the dialog
 *      (WPF: `result == ""` → `this.Close()`).
 *    - `kind === 'error_message'` ⇒ display the verbatim message
 *      inline (WPF: `result != ""` → `lblErrorMessage.Content =
 *      result`).
 *
 *    A backend transport / parse failure throws `auth.*` /
 *    `beanfun.*` and is toasted by `wrapCommand`; the WPF
 *    `result == null` fallback branch (resource `_27`,
 *    "新增遊戲帳號失敗, 可能這個遊戲無法創建帳號。") is folded into
 *    the same toast pipeline because the SPA already classifies
 *    the failure (transport / non-2xx / missing payload) more
 *    specifically than WPF's null-collapse — surfacing the
 *    cause-specific message is strictly more useful than the
 *    WPF generic copy. See
 *    `services::beanfun::unconnected_game_add_account` for the
 *    typed error variants we surface upstream.
 *
 * # WPF deviations (intentional)
 *
 * - **Modal vs new Window**: same rationale as the rest of
 *   `windows/*.vue`. WPF used a top-level `Window`; the SPA
 *   renders an in-page `<el-dialog>`. No functional impact.
 * - **No drag-to-move**: `Window_MouseLeftButtonDown` →
 *   `DragMove()` is meaningless inside a modal layered over a
 *   backdrop; omitted (mirrors `AddServiceAccount.vue` /
 *   `Contract.vue`).
 * - **Validation toasts vs MessageBox alerts**: WPF used
 *   `MessageBox.Show(...)` (blocking modal alert) for every
 *   validation rejection. We use `ElMessage.warning(...)` (toast)
 *   for the same set, mirroring the convention established in
 *   `AddServiceAccount.vue` (P12.2 D3). The user still sees the
 *   exact same i18n message, but doesn't have to dismiss a
 *   second modal layered on top of the dialog.
 * - **Submit button disabled mid-flight**: WPF `IsDefault="True"`
 *   would re-fire on Enter while the previous request was still
 *   pending (no built-in guard). We guard with `submitting.value`
 *   so a double Enter / double click can't issue duplicate POSTs.
 *
 * # Mockup conflict resolution (P12.3 plan, user-approved)
 *
 * `mockups/UnconnectedGame_AddAccount.html` redesigns the form
 * around: (1) a server-side captcha check, (2) a nickname-only
 * field set, (3) a strength meter on the password field, (4) a
 * "Suggested account id" generator hyperlink. None of these
 * exist in WPF or the backend, so we drop them all:
 *
 * 1. **Captcha** — WPF never sends one and the Beanfun account
 *    management endpoint does not require it (the session-cookie
 *    binding is the rate-limit signal). Adding a captcha would
 *    necessitate a new IPC + a new server contract — out of
 *    scope.
 * 2. **Nickname-only field set** — the WPF form has accountId +
 *    DN + pwd × 2 + agree checkbox (see resource keys above);
 *    nickname-only would silently strip the password fields the
 *    server requires.
 * 3. **Strength meter** — the server enforces length only (via
 *    `accountLenMin/Max`); a UI strength meter would imply
 *    additional rules (uppercase, digits, etc.) that the server
 *    does not validate, so the meter would lie about the
 *    server's actual policy.
 * 4. **Suggested account id generator** — has no backend
 *    counterpart; we'd have to invent a generator client-side
 *    that may collide with server-side uniqueness checks.
 *
 * The mockup chrome we *do* preserve: glass dialog header with
 * icon + title + close button (matches every other
 * `windows/*.vue`).
 *
 * # State / lifecycle
 *
 * - `init` is fetched lazily on the first `false → true`
 *   transition. `mgmtSession` follows the same pattern as
 *   `AddAccountSession` round-tripping through the WPF
 *   `NameValueCollection`: every successful call (`check`,
 *   `checkNickname`, `addAccount`) MAY return a refreshed
 *   triplet, which we re-stash so the next call uses the latest
 *   view-state. The frontend treats it as opaque (no field
 *   inspection) — see the [`AddAccountSession`] docblock for
 *   the rationale.
 * - All form state + the inline `errorMessage` reset on dialog
 *   close (`closed` event) so reopening starts pristine,
 *   matching WPF's "new Window per open" lifecycle.
 *
 * # Caller wiring (`AccountList.vue` D8)
 *
 * ```vue
 * <UnconnectedGameAddAccount
 *   v-model:visible="addAccountOpen"
 *   @created="account.refresh()"
 * />
 * ```
 *
 * No `service_code` / `region` props because the backend pulls
 * both from the active session (the dialog only ever targets the
 * user's currently-selected unconnected game). This keeps the
 * caller free from re-passing data the IPC already has — the
 * same pattern `unconnected_game_change_password` follows.
 */

import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import {
  ElButton,
  ElCheckbox,
  ElDialog,
  ElForm,
  ElFormItem,
  ElIcon,
  ElInput,
  ElMessage,
} from 'element-plus'
import { CircleClose, CirclePlus } from '@element-plus/icons-vue'

import { commands } from '../types/bindings'
import type { AddAccountInit, AddAccountSession } from '../types/bindings'
import { useAccountStore } from '../stores/account'
import { wrapCommand } from '../services/invoke'
import Contract from './Contract.vue'

defineOptions({ name: 'UnconnectedGameAddAccountDialog' })

const props = defineProps<{
  /**
   * Two-way binding for whether the dialog is shown. Caller drives
   * open via `v-model:visible="..."`; the dialog drives close on
   * Esc / outside-click / cancel / successful create / init failure.
   */
  visible: boolean
}>()

const emit = defineEmits<{
  /** Two-way binding back to the caller's `visible` ref. */
  (event: 'update:visible', next: boolean): void
  /**
   * Emitted exactly once after
   * [`commands.unconnectedGameAddAccount`] returns
   * `AddAccountOutcome.success`. Caller (`AccountList.vue` D8)
   * uses this to refresh the account list — parity with WPF
   * `MainWindow.redrawSAccountList()` after the dialog closes.
   */
  (event: 'created'): void
}>()

const { t } = useI18n()
const account = useAccountStore()

/* --------------- init payload + session --------------- */

const init = ref<AddAccountInit | null>(null)
const mgmtSession = ref<AddAccountSession | null>(null)
const loadingInit = ref(false)

/* --------------- form state --------------- */

const accountId = ref('')
const accountDn = ref('')
const newPwd = ref('')
const newPwd2 = ref('')
const agreed = ref(false)
const errorMessage = ref('')

/* --------------- busy guards --------------- */

const submitting = ref(false)
const checkingId = ref(false)
const checkingNickname = ref(false)

/* --------------- contract nested dialog state --------------- */

const contractVisible = ref(false)
const contractText = ref('')
const contractLoading = ref(false)

/* --------------- helpers --------------- */

const visible = computed({
  get: (): boolean => props.visible,
  set: (next: boolean): void => emit('update:visible', next),
})

const gameName = computed<string>(() => init.value?.game_name ?? '')
const accountLenText = computed<string>(() => init.value?.account_len ?? '')
const nicknameSupported = computed<boolean>(() => init.value?.check_nickname_supported === true)

/**
 * Parse the server-supplied `account_len` ("min - max" e.g.
 * "6 - 12") into numeric bounds. Mirrors WPF's
 * `Button_Click` length parser which `Split(" - ")` + `byte.Parse`.
 * Returns `null` when the format is unexpected (WPF showed
 * `UnknownError` and aborted the submit; we propagate the same
 * sentinel so the validation chain can surface the toast).
 */
const lengthRange = computed<{ min: number; max: number } | null>(() => {
  const text = accountLenText.value
  if (!text || !text.includes(' - ')) return null
  const [minStr, maxStr] = text.split(' - ')
  const min = Number.parseInt(minStr ?? '', 10)
  const max = Number.parseInt(maxStr ?? '', 10)
  if (!Number.isFinite(min) || !Number.isFinite(max)) return null
  return { min, max }
})

const anyBusy = computed<boolean>(
  () => loadingInit.value || submitting.value || checkingId.value || checkingNickname.value,
)

/* --------------- init lifecycle --------------- */

/**
 * Fetch the init payload on the first `false → true` transition.
 *
 * On failure WPF showed `UnknownError` MessageBox and closed the
 * dialog immediately (constructor never finished). We mirror that
 * with a toast + `visible = false`. The init payload is cached
 * for the lifetime of the dialog open — closing + reopening
 * fetches fresh state (matches WPF's "new Window per open"
 * lifecycle).
 */
async function ensureInit(): Promise<void> {
  if (init.value !== null || loadingInit.value) return
  loadingInit.value = true
  try {
    const payload = await wrapCommand(commands.unconnectedGameInitAddAccountPayload())
    init.value = payload
    mgmtSession.value = payload.session
  } catch {
    /*
     * `wrapCommand` already surfaced a localized error toast for
     * the underlying transport / parse failure. WPF additionally
     * showed a generic `UnknownError` MessageBox; the more
     * specific toast strictly improves on that, so we don't
     * duplicate the generic copy. Close the dialog regardless
     * (WPF parity: the constructor calls `this.Close()`).
     */
    visible.value = false
  } finally {
    loadingInit.value = false
  }
}

watch(
  visible,
  (next, prev) => {
    if (next && prev !== true) void ensureInit()
  },
  { immediate: true },
)

/* --------------- validation + submit --------------- */

/**
 * Run the WPF `Button_Click` validation chain in the same order.
 * Returns `true` when every check passed; surfaces the matching
 * resource via `ElMessage.warning` and returns `false` otherwise.
 *
 * Extracted for readability — the chain has nine sequential
 * branches, inlining it would balloon `handleSubmit` past the
 * point where the order is obvious at a glance.
 */
function runSubmitValidation(): boolean {
  const range = lengthRange.value
  if (range === null) {
    ElMessage.error(t('UnknownError'))
    return false
  }
  const { min, max } = range

  if (accountId.value === '') {
    ElMessage.warning(t('UnconnectedGame_AddAccount_18'))
    return false
  }
  if (accountId.value.length < min || accountId.value.length > max) {
    ElMessage.warning(t('UnconnectedGame_AddAccount_19'))
    return false
  }
  if (newPwd.value === '') {
    ElMessage.warning(t('UnconnectedGame_AddAccount_20'))
    return false
  }
  if (newPwd.value.length < min || newPwd.value.length > max) {
    ElMessage.warning(t('UnconnectedGame_AddAccount_21'))
    return false
  }
  if (newPwd2.value === '') {
    ElMessage.warning(t('UnconnectedGame_AddAccount_22'))
    return false
  }
  if (newPwd2.value.length < min || newPwd2.value.length > max) {
    ElMessage.warning(t('UnconnectedGame_AddAccount_23'))
    return false
  }
  if (nicknameSupported.value) {
    if (accountDn.value === '') {
      ElMessage.warning(t('UnconnectedGame_AddAccount_24'))
      return false
    }
    /*
     * WPF hard-codes 2..6 as the DN length range, distinct from the
     * server-supplied `accountLen` range used for the account id
     * and passwords. Preserved verbatim — see
     * `UnconnectedGame_AddAccount.xaml.cs` L197.
     */
    if (accountDn.value.length < 2 || accountDn.value.length > 6) {
      ElMessage.warning(t('UnconnectedGame_AddAccount_25'))
      return false
    }
  }
  if (!agreed.value) {
    ElMessage.warning(t('UnconnectedGame_AddAccount_26'))
    return false
  }
  return true
}

async function handleSubmit(): Promise<void> {
  if (anyBusy.value) return
  if (mgmtSession.value === null) {
    ElMessage.error(t('UnknownError'))
    return
  }
  if (!runSubmitValidation()) return

  submitting.value = true
  try {
    const outcome = await wrapCommand(
      commands.unconnectedGameAddAccount(
        mgmtSession.value,
        accountId.value,
        newPwd.value,
        newPwd2.value,
        nicknameSupported.value ? accountDn.value : null,
      ),
    )
    if (outcome.kind === 'success') {
      errorMessage.value = ''
      emit('created')
      visible.value = false
      return
    }
    /*
     * `outcome.kind === 'error_message'` — WPF parity: write the
     * verbatim server message into the inline `lblErrorMessage`
     * label (red label below the form). Stay open so the user
     * can adjust inputs and retry.
     */
    errorMessage.value = outcome.data
  } catch {
    /* `wrapCommand` toasted the underlying cause. Stay open. */
  } finally {
    submitting.value = false
  }
}

/* --------------- check hyperlinks --------------- */

async function handleCheckId(): Promise<void> {
  if (anyBusy.value) return
  if (mgmtSession.value === null) {
    ElMessage.error(t('UnknownError'))
    return
  }
  checkingId.value = true
  try {
    const outcome = await wrapCommand(
      commands.unconnectedGameAddAccountCheck(
        mgmtSession.value,
        accountId.value,
        nicknameSupported.value ? accountDn.value : null,
      ),
    )
    mgmtSession.value = outcome.session
    if (outcome.error_message === '') {
      /*
       * WPF parity: empty `lblErrorMessage` after a successful
       * POST means the server returned no message at all — WPF
       * treated this as the "UnknownError" sentinel
       * (`UnconnectedGame_AddAccount.xaml.cs` L61). Mirror that
       * exactly. We don't clear the existing inline message in
       * this branch (WPF only overwrites on populated content).
       */
      ElMessage.error(t('UnknownError'))
      return
    }
    errorMessage.value = outcome.error_message
  } catch {
    /* `wrapCommand` toasted; preserve the existing inline message. */
  } finally {
    checkingId.value = false
  }
}

async function handleCheckNickname(): Promise<void> {
  if (anyBusy.value) return
  if (!nicknameSupported.value) return
  if (mgmtSession.value === null) {
    ElMessage.error(t('UnknownError'))
    return
  }
  checkingNickname.value = true
  try {
    const outcome = await wrapCommand(
      commands.unconnectedGameAddAccountCheckNickname(mgmtSession.value, accountDn.value),
    )
    mgmtSession.value = outcome.session
    if (outcome.error_message === '') {
      ElMessage.error(t('UnknownError'))
      return
    }
    errorMessage.value = outcome.error_message
  } catch {
    /* `wrapCommand` toasted; preserve the existing inline message. */
  } finally {
    checkingNickname.value = false
  }
}

/* --------------- contract preview --------------- */

async function handleOpenContract(): Promise<void> {
  if (contractLoading.value) return
  contractLoading.value = true
  try {
    const text = await account.getContract()
    if (text === '') {
      ElMessage.error(t('UnknownError'))
      return
    }
    contractText.value = text
    contractVisible.value = true
  } catch {
    /* `wrapCommand` already toasted the underlying cause. */
  } finally {
    contractLoading.value = false
  }
}

/* --------------- close lifecycle --------------- */

function handleCancel(): void {
  if (submitting.value) return
  visible.value = false
}

/**
 * Reset every piece of dialog state on close (after the fade-out
 * animation, via `<el-dialog @closed>`). Mirrors WPF's "new Window
 * per open" lifecycle so reopening the dialog never shows stale
 * input from a previous session.
 */
function handleClosed(): void {
  init.value = null
  mgmtSession.value = null
  accountId.value = ''
  accountDn.value = ''
  newPwd.value = ''
  newPwd2.value = ''
  agreed.value = false
  errorMessage.value = ''
  loadingInit.value = false
  submitting.value = false
  checkingId.value = false
  checkingNickname.value = false
  contractVisible.value = false
  contractText.value = ''
  contractLoading.value = false
}
</script>

<template>
  <el-dialog
    v-model="visible"
    :close-on-click-modal="false"
    :close-on-press-escape="!submitting"
    :show-close="false"
    :before-close="(done: () => void) => (submitting ? undefined : done())"
    :width="560"
    align-center
    append-to-body
    class="ucga-dialog"
    data-test="unconnected-game-add-account-dialog"
    @closed="handleClosed"
  >
    <template #header>
      <div class="ucga__header">
        <div class="ucga__header-meta">
          <el-icon class="ucga__header-icon" :size="20">
            <CirclePlus />
          </el-icon>
          <span class="ucga__header-title" data-test="unconnected-game-add-account-title">
            {{ t('AddServiceAccount') }}
          </span>
        </div>
        <button
          type="button"
          class="ucga__header-close"
          :title="t('Cancel')"
          :disabled="submitting"
          data-test="unconnected-game-add-account-close"
          @click="handleCancel"
        >
          <el-icon><CircleClose /></el-icon>
        </button>
      </div>
    </template>

    <div
      v-if="loadingInit && init === null"
      class="ucga__state ucga__state--loading"
      data-test="unconnected-game-add-account-loading"
      role="status"
      aria-live="polite"
    >
      <p class="ucga__state-text">{{ t('unconnectedGameAddAccount.loading') }}</p>
    </div>

    <div v-else-if="init !== null" class="ucga__body bf-custom-scrollbar">
      <p class="ucga__intro" data-test="unconnected-game-add-account-intro">
        <span>{{ t('UnconnectedGame_AddAccount_1') }}</span>
        <strong class="ucga__intro-game">{{ gameName }}</strong>
        <span>{{ t('UnconnectedGame_AddAccount_2') }}</span>
      </p>
      <ul class="ucga__bullets" data-test="unconnected-game-add-account-bullets">
        <li>
          <span>{{ t('UnconnectedGame_AddAccount_3') }}</span>
          <strong>{{ gameName }}</strong>
          <span>{{ t('UnconnectedGame_AddAccount_4') }}</span>
        </li>
        <li>{{ t('UnconnectedGame_AddAccount_5') }}</li>
        <li>
          <span>{{ t('UnconnectedGame_AddAccount_6') }}</span>
          <strong data-test="unconnected-game-add-account-len">{{ accountLenText }}</strong>
          <span>{{ t('UnconnectedGame_AddAccount_7') }}</span>
        </li>
      </ul>

      <el-form
        class="ucga__form"
        label-position="top"
        hide-required-asterisk
        @submit.prevent="handleSubmit"
      >
        <div class="ucga__form-grid">
          <div class="ucga__form-fields">
            <el-form-item
              :label="`${t('UnconnectedGame_AddAccount_8')}${gameName}${t('Account')}:`"
              class="ucga__field"
            >
              <el-input
                v-model="accountId"
                :disabled="anyBusy"
                autocomplete="off"
                data-test="unconnected-game-add-account-id"
              />
            </el-form-item>

            <template v-if="nicknameSupported">
              <el-form-item
                :label="`${t('UnconnectedGame_AddAccount_9')}${gameName}${t('UnconnectedGame_AddAccount_10')}`"
                class="ucga__field"
              >
                <el-input
                  v-model="accountDn"
                  :disabled="anyBusy"
                  :placeholder="t('UnconnectedGame_AddAccount_17')"
                  autocomplete="off"
                  data-test="unconnected-game-add-account-dn"
                />
              </el-form-item>
              <p class="ucga__field-hint" data-test="unconnected-game-add-account-dn-hint">
                {{ t('UnconnectedGame_AddAccount_11') }}
              </p>
            </template>

            <el-form-item
              :label="`${t('UnconnectedGame_AddAccount_8')}${gameName}${t('Password')}:`"
              class="ucga__field"
            >
              <el-input
                v-model="newPwd"
                type="password"
                :disabled="anyBusy"
                autocomplete="new-password"
                show-password
                data-test="unconnected-game-add-account-pwd"
              />
            </el-form-item>

            <el-form-item
              :label="`${t('UnconnectedGame_AddAccount_9')}${gameName}${t('Password')}:`"
              class="ucga__field"
            >
              <el-input
                v-model="newPwd2"
                type="password"
                :disabled="anyBusy"
                autocomplete="new-password"
                show-password
                data-test="unconnected-game-add-account-pwd2"
              />
            </el-form-item>
          </div>

          <div class="ucga__form-actions">
            <button
              type="button"
              class="ucga__hyperlink"
              :disabled="anyBusy"
              data-test="unconnected-game-add-account-check-id"
              @click="handleCheckId"
            >
              {{ t('UnconnectedGame_AddAccount_13') }}
            </button>
            <button
              v-if="nicknameSupported"
              type="button"
              class="ucga__hyperlink"
              :disabled="anyBusy"
              data-test="unconnected-game-add-account-check-nickname"
              @click="handleCheckNickname"
            >
              {{ t('UnconnectedGame_AddAccount_14') }}
            </button>
          </div>
        </div>

        <p
          v-if="errorMessage !== ''"
          class="ucga__error"
          data-test="unconnected-game-add-account-error"
          role="alert"
        >
          {{ errorMessage }}
        </p>

        <div class="ucga__terms">
          <el-checkbox
            v-model="agreed"
            :disabled="anyBusy"
            data-test="unconnected-game-add-account-agree"
          >
            <span class="ucga__terms-text">
              {{ t('UnconnectedGame_AddAccount_15') }}
              <button
                type="button"
                class="ucga__terms-link"
                :disabled="contractLoading || anyBusy"
                data-test="unconnected-game-add-account-terms"
                @click.prevent.stop="handleOpenContract"
              >
                {{ gameName }}{{ t('UnconnectedGame_AddAccount_16') }}
              </button>
            </span>
          </el-checkbox>
        </div>
      </el-form>
    </div>

    <template #footer>
      <div class="ucga__footer">
        <el-button
          class="bf-btn-secondary ucga__btn-cancel"
          :disabled="submitting"
          data-test="unconnected-game-add-account-cancel"
          @click="handleCancel"
        >
          {{ t('Cancel') }}
        </el-button>
        <el-button
          class="bf-btn-primary ucga__btn-submit"
          :loading="submitting"
          :disabled="loadingInit || init === null"
          data-test="unconnected-game-add-account-submit"
          @click="handleSubmit"
        >
          {{ t('AuthConfirm') }}
        </el-button>
      </div>
    </template>
  </el-dialog>

  <Contract v-model:visible="contractVisible" :text="contractText" />
</template>

<style scoped>
.ucga__header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.5rem;
}

.ucga__header-meta {
  display: inline-flex;
  align-items: center;
  gap: 0.5rem;
  min-width: 0;
}

.ucga__header-icon {
  color: var(--bf-primary);
  flex-shrink: 0;
}

.ucga__header-title {
  font-size: 0.9375rem;
  font-weight: 700;
  color: var(--bf-on-surface);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.ucga__header-close {
  appearance: none;
  border: 0;
  background: transparent;
  width: 28px;
  height: 28px;
  display: grid;
  place-items: center;
  border-radius: var(--bf-radius-input);
  color: var(--bf-on-surface-variant);
  cursor: pointer;
  transition:
    background var(--bf-motion-fast),
    color var(--bf-motion-fast);
}

.ucga__header-close:hover:not(:disabled) {
  background: color-mix(in srgb, var(--bf-danger) 80%, transparent);
  color: var(--bf-on-danger);
}

.ucga__header-close:disabled {
  opacity: 0.45;
  cursor: not-allowed;
}

.ucga__state {
  display: flex;
  align-items: center;
  justify-content: center;
  min-height: 280px;
  padding: 1rem;
}

.ucga__state-text {
  margin: 0;
  font-size: 0.8125rem;
  color: var(--bf-on-surface-variant);
}

.ucga__body {
  max-height: 70vh;
  overflow-y: auto;
  padding: 0.25rem 0.5rem;
}

.ucga__intro {
  margin: 0 0 0.5rem 0;
  font-size: 0.875rem;
  color: var(--bf-on-surface);
  display: flex;
  flex-wrap: wrap;
  gap: 0.25rem;
  align-items: baseline;
}

.ucga__intro-game {
  color: var(--bf-primary);
  font-size: 0.9375rem;
}

.ucga__bullets {
  margin: 0 0 1rem 0;
  padding-left: 1.25rem;
  font-size: 0.8125rem;
  color: var(--bf-on-surface-variant);
  line-height: 1.6;
}

.ucga__bullets li {
  margin: 0 0 0.25rem 0;
}

.ucga__bullets strong {
  color: var(--bf-on-surface);
  font-weight: 600;
}

.ucga__form {
  display: flex;
  flex-direction: column;
  gap: 0.75rem;
}

.ucga__form-grid {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  gap: 1rem;
  align-items: start;
}

.ucga__form-fields {
  display: flex;
  flex-direction: column;
  gap: 0.25rem;
}

.ucga__field :deep(.el-form-item__label) {
  font-size: 0.75rem;
  color: var(--bf-on-surface-variant);
  padding-bottom: 0.25rem;
}

.ucga__field-hint {
  margin: -0.25rem 0 0.25rem 0;
  font-size: 0.6875rem;
  color: var(--bf-on-surface-variant);
}

.ucga__form-actions {
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
  padding-top: 1.5rem;
}

.ucga__hyperlink {
  appearance: none;
  border: 0;
  background: transparent;
  font-size: 0.8125rem;
  color: var(--bf-primary);
  text-decoration: underline;
  cursor: pointer;
  padding: 0.25rem 0;
  text-align: left;
}

.ucga__hyperlink:disabled {
  color: var(--bf-on-surface-variant);
  cursor: not-allowed;
  text-decoration: none;
}

.ucga__error {
  margin: 0;
  font-size: 0.8125rem;
  color: var(--bf-danger, #ba1a1a);
  text-align: center;
}

.ucga__terms {
  margin-top: 0.25rem;
}

.ucga__terms-text {
  font-size: 0.8125rem;
}

.ucga__terms-link {
  appearance: none;
  border: 0;
  background: transparent;
  color: var(--bf-primary);
  text-decoration: underline;
  cursor: pointer;
  padding: 0;
}

.ucga__terms-link:disabled {
  color: var(--bf-on-surface-variant);
  cursor: not-allowed;
  text-decoration: none;
}

.ucga__footer {
  display: flex;
  justify-content: flex-end;
  gap: 0.625rem;
}
</style>
