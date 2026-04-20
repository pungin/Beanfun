<script setup lang="ts">
/**
 * Change stored Beanfun credential dialog (P12.2 D8).
 *
 * # WPF parity
 *
 * Mirrors `Beanfun/Windows/ChangeAccount.xaml(.cs)`. WPF showed
 * this as a top-level `Window` opened by `ManageAccount` row
 * actions; the SPA renders it as an in-page `<el-dialog>` modal —
 * same "modal vs new window" rationale as D3 / D4 / D8 AddAccount.
 *
 * Functional parity (deviates from WPF on the ID-edit axis — see
 * the Q5 = mockup parity note below):
 *
 * 1. **Pre-fill on open** — `account_name` and `auto_login` are
 *    primed from the prop record on the `false → true` visibility
 *    transition. WPF's constructor (L21-23) does the same prime
 *    via `accountManager.getNameByAccount` / `getAutoLoginByAccount`.
 *    The SPA passes the full `Account` record as a prop instead
 *    of looking it up by `(region, id)` because (a) the parent
 *    page already has the record in hand from the row click and
 *    (b) it matches the D4 pattern of "explicit input over store
 *    coupling".
 * 2. **Submit** — call `account.saveAccount({ ...prop, account_name,
 *    auto_login })`. The store wraps `commands.save_account` (which
 *    is an upsert by `(region, account_id)`); since `region` and
 *    `account_id` are unchanged the upsert lands on the existing
 *    row and overwrites `account_name` + `auto_login` in place
 *    while preserving `password` / `verify` / `method`.
 *
 * # WPF deviations (intentional, all under D8 Q5 = mockup parity)
 *
 * - **Account ID is read-only**. WPF allowed editing
 *   `t_AccountID.Text`; on submit it called `removeAccount(old_id)`
 *   then `addAccount(changedIndex, region, new_id, ...)` to
 *   re-position the row at its original index (L41-51). The Rust
 *   backend `save_account` only supports upsert by `(region, id)`,
 *   not indexed insertion (`src-tauri/src/commands/storage.rs`),
 *   so an "edit ID" path here would have to do remove + save which
 *   loses the row's positional order and opens a race window
 *   (other clients reading the file mid-flow see the row absent).
 *   The mockup (`mockups/ChangeAccount.html`) also presents the
 *   id as an avatar caption (read-only), so keeping it
 *   non-editable is the intersection of mockup + safe SPA behavior.
 *   The "delete + re-add to change ID" workaround is documented
 *   inline via `t('changeAccountDialog.accountIdReadonlyHint')`
 *   and routed through the Manage Accounts (D9) page's row Delete
 *   action.
 * - **No password / verify field**. The mockup adds a "change
 *   password" input + "remember password" checkbox; we drop both:
 *   - Password rotation that doesn't validate against the Beanfun
 *     server is a footgun (the saved password could silently
 *     desync from reality, and the next login would fail with
 *     "bad credentials" without the user knowing why).
 *   - The legitimate password update path is to log in with the
 *     new password via `IdPassForm` with "remember password"
 *     ticked — that hits `account.saveLoginCredentials` which
 *     re-writes the saved password atomically. Routing through a
 *     real login is the only way to validate the new password.
 *   - Verify is even more transient (HK-only second-factor token
 *     bound to a server-side challenge) — editing the saved
 *     verify out-of-band has no benefit over re-logging.
 * - **No "delete account" button**. The mockup's footer has a
 *   destructive delete button next to Cancel / Save; we route
 *   destructive ops through the Manage Accounts (D9) row Delete
 *   action instead — the dialog should be focused on edit, not
 *   mode-switching to delete.
 * - **Empty-name allowed**. WPF L33-37 only blocks empty
 *   `t_AccountID.Text` (which we don't expose for editing). An
 *   empty `account_name` is a valid state in WPF's storage model
 *   (`accountNameList[i]` defaults to `""` for fresh logins —
 *   see `account.saveLoginCredentials` docblock in
 *   `stores/account.ts`), so we don't add a synthetic SPA
 *   validation that WPF doesn't enforce.
 *
 * # Why a separate SFC instead of a shared composable with AddAccount
 *
 * Rule-of-three: AddAccount + ChangeAccount + (future) D10
 * IdPassForm prefill / D9 manage-row form would be the third
 * caller before we have enough variance signal to abstract. The
 * field set differs significantly today (Add: 5 inputs + region +
 * conditional verify, Change: 1 editable input + 1 checkbox +
 * read-only chrome), so a shared composable would just be
 * prop-flag spaghetti. We let both files hold their own copy and
 * revisit when D9 lands.
 */

import { computed, nextTick, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { ElButton, ElCheckbox, ElDialog, ElForm, ElFormItem, ElIcon, ElInput } from 'element-plus'
import { Check, CircleClose, EditPen, InfoFilled } from '@element-plus/icons-vue'

import { useAccountStore } from '../stores/account'
import type { Account } from '../types/bindings'

defineOptions({ name: 'ChangeAccount' })

const props = defineProps<{
  /**
   * Two-way binding for whether the dialog is shown. Caller drives
   * open via `v-model:visible="..."`; the dialog drives close on
   * cancel / success / Esc / outside-click.
   */
  visible: boolean
  /**
   * The stored account to edit. `null` while the dialog is closed
   * (the caller clears it after the row click has been handled to
   * avoid leaking stale account references between sessions). The
   * dialog is rendered as a no-op shell when `account === null` so
   * the v-model binding round-trips cleanly.
   *
   * The dialog mutates **only** `account_name` + `auto_login`; the
   * rest of the record (`region` / `account_id` / `password` /
   * `verify` / `method`) is forwarded verbatim into the upsert
   * payload to preserve fields the user can't see / edit here.
   */
  account: Account | null
}>()

const emit = defineEmits<{
  /** Two-way binding back to the caller's `visible` ref. */
  (event: 'update:visible', next: boolean): void
  /**
   * Emitted exactly once after `account.saveAccount` resolves and
   * the updated record is in the store. Caller may use this to
   * surface a success toast / re-arm row selection / etc.
   */
  (
    event: 'updated',
    payload: { region: string; accountId: string; accountName: string; autoLogin: boolean },
  ): void
}>()

const { t } = useI18n()
/* Aliased to `accountStore` to avoid name collision with the `account` prop. */
const accountStore = useAccountStore()

/* --------------- form state --------------- */

const accountName = ref('')
const autoLogin = ref(false)
const submitting = ref(false)
const accountNameInput = ref<InstanceType<typeof ElInput> | null>(null)

/* --------------- helpers --------------- */

const visible = computed({
  get: (): boolean => props.visible,
  set: (next: boolean): void => emit('update:visible', next),
})

const regionLabel = computed((): string => {
  const region = props.account?.region
  if (region === 'TW') return t('Taiwan')
  if (region === 'HK') return t('HongKong')
  return region ?? ''
})

const accountIdDisplay = computed((): string => props.account?.account_id ?? '')

function resetForm(): void {
  accountName.value = ''
  autoLogin.value = false
  submitting.value = false
}

/**
 * Pre-fill the form fields with the prop record's values every
 * time the dialog opens, then auto-focus the editable name input
 * (matches WPF tab order: account_id was focused but read-only
 * here, so focus advances to the first editable field).
 *
 * Re-priming on every `false → true` transition (rather than
 * `onMounted`) tracks whatever record the caller passes in — the
 * user might click "edit" on row A, close, then click row B; the
 * dialog needs to reflect the latest selection.
 */
watch(
  visible,
  async (next) => {
    if (next && props.account !== null) {
      accountName.value = props.account.account_name
      autoLogin.value = props.account.auto_login
      await nextTick()
      accountNameInput.value?.focus()
    }
  },
  { immediate: true },
)

/* --------------- submit --------------- */

async function handleSubmit(): Promise<void> {
  if (submitting.value) return

  const target = props.account
  if (target === null) return

  submitting.value = true
  try {
    /*
     * Upsert: the store wraps `commands.save_account` which
     * matches the existing row by `(region, account_id)` and
     * overwrites all 7 fields. We forward `password` / `verify`
     * / `method` from the prop verbatim so the unedited fields
     * survive the round-trip — this is the entire reason
     * ChangeAccount accepts the full record instead of letting
     * the backend "merge" partials.
     */
    await accountStore.saveAccount({
      region: target.region,
      account_id: target.account_id,
      account_name: accountName.value.trim(),
      password: target.password,
      verify: target.verify,
      method: target.method,
      auto_login: autoLogin.value,
    })
    emit('updated', {
      region: target.region,
      accountId: target.account_id,
      accountName: accountName.value.trim(),
      autoLogin: autoLogin.value,
    })
    visible.value = false
  } catch {
    /* Toasted by `wrapCommand`; stay open so the user can retry. */
  } finally {
    submitting.value = false
  }
}

function handleCancel(): void {
  visible.value = false
}

function handleClosed(): void {
  resetForm()
}
</script>

<template>
  <el-dialog
    v-model="visible"
    :close-on-click-modal="false"
    :close-on-press-escape="!submitting"
    :show-close="false"
    :before-close="(done: () => void) => (submitting ? undefined : done())"
    :width="460"
    align-center
    append-to-body
    class="change-account-dialog"
    data-test="change-account-dialog"
    @closed="handleClosed"
  >
    <template #header>
      <div class="change-acc__header">
        <div class="change-acc__header-meta">
          <el-icon class="change-acc__header-icon" :size="20">
            <EditPen />
          </el-icon>
          <span class="change-acc__header-title">{{ t('changeAccountDialog.title') }}</span>
        </div>
        <button
          type="button"
          class="change-acc__header-close"
          :title="t('Cancel')"
          :disabled="submitting"
          data-test="change-account-close"
          @click="handleCancel"
        >
          <el-icon><CircleClose /></el-icon>
        </button>
      </div>
    </template>

    <p class="change-acc__subtitle">{{ t('changeAccountDialog.subtitle') }}</p>

    <el-form
      class="change-acc__form"
      label-position="top"
      hide-required-asterisk
      @submit.prevent="handleSubmit"
    >
      <div class="change-acc__readonly-grid">
        <div class="change-acc__readonly-field">
          <span class="change-acc__readonly-label">
            {{ t('changeAccountDialog.regionLabel') }}
          </span>
          <span class="change-acc__readonly-value" data-test="change-account-region-display">
            {{ regionLabel }}
          </span>
        </div>
        <div class="change-acc__readonly-field">
          <span class="change-acc__readonly-label">
            {{ t('changeAccountDialog.accountIdLabel') }}
          </span>
          <span class="change-acc__readonly-value" data-test="change-account-id-display">
            {{ accountIdDisplay }}
          </span>
        </div>
      </div>

      <p class="change-acc__readonly-hint" data-test="change-account-id-hint">
        <el-icon><InfoFilled /></el-icon>
        <span>{{ t('changeAccountDialog.accountIdReadonlyHint') }}</span>
      </p>

      <el-form-item :label="t('changeAccountDialog.accountNameLabel')" class="change-acc__field">
        <el-input
          ref="accountNameInput"
          v-model="accountName"
          :disabled="submitting"
          :placeholder="t('tbBeanfunRemark')"
          autocomplete="off"
          data-test="change-account-name"
          @keyup.enter="handleSubmit"
        />
      </el-form-item>

      <el-form-item class="change-acc__field change-acc__autologin">
        <el-checkbox
          v-model="autoLogin"
          :disabled="submitting"
          data-test="change-account-autologin"
        >
          {{ t('AutoLogin') }}
        </el-checkbox>
      </el-form-item>
    </el-form>

    <template #footer>
      <div class="change-acc__footer">
        <el-button
          class="bf-btn-secondary change-acc__btn-secondary"
          :disabled="submitting"
          data-test="change-account-cancel"
          @click="handleCancel"
        >
          {{ t('Cancel') }}
        </el-button>
        <button
          type="button"
          class="bf-btn-gradient change-acc__btn-primary"
          :disabled="submitting"
          data-test="change-account-submit"
          @click="handleSubmit"
        >
          <el-icon><Check /></el-icon>
          <span>{{ t('changeAccountDialog.save') }}</span>
        </button>
      </div>
    </template>
  </el-dialog>
</template>

<style scoped>
.change-acc__header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.5rem;
}

.change-acc__header-meta {
  display: inline-flex;
  align-items: center;
  gap: 0.5rem;
  min-width: 0;
}

.change-acc__header-icon {
  color: var(--bf-primary);
  flex-shrink: 0;
}

.change-acc__header-title {
  font-size: 0.9375rem;
  font-weight: 700;
  color: var(--bf-on-surface);
}

.change-acc__header-close {
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

.change-acc__header-close:hover:not(:disabled) {
  background: color-mix(in srgb, var(--bf-danger) 80%, transparent);
  color: var(--bf-on-danger);
}

.change-acc__header-close:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}

.change-acc__subtitle {
  margin: 0 0 0.875rem;
  font-size: 0.8125rem;
  color: var(--bf-on-surface-variant);
}

.change-acc__form {
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
}

.change-acc__readonly-grid {
  display: grid;
  grid-template-columns: minmax(96px, max-content) 1fr;
  gap: 0.375rem 0.875rem;
  padding: 0.75rem 0.875rem;
  background: var(--bf-surface-container-low);
  border: 1px solid var(--bf-outline-variant);
  border-radius: var(--bf-radius-input);
}

.change-acc__readonly-field {
  display: contents;
}

.change-acc__readonly-label {
  font-size: 0.75rem;
  font-weight: 600;
  color: var(--bf-on-surface-variant);
  align-self: center;
}

.change-acc__readonly-value {
  font-size: 0.875rem;
  font-weight: 600;
  color: var(--bf-on-surface);
  align-self: center;
  word-break: break-all;
}

.change-acc__readonly-hint {
  display: flex;
  align-items: flex-start;
  gap: 0.375rem;
  margin: 0.25rem 0 0.5rem;
  font-size: 0.75rem;
  color: var(--bf-on-surface-variant);
}

.change-acc__readonly-hint :deep(.el-icon) {
  color: var(--bf-primary);
  flex-shrink: 0;
  margin-top: 0.125rem;
}

.change-acc__field :deep(.el-form-item__label) {
  font-size: 0.75rem;
  font-weight: 600;
  color: var(--bf-on-surface-variant);
  padding-bottom: 0.25rem;
}

.change-acc__autologin :deep(.el-form-item__content) {
  align-items: center;
}

.change-acc__footer {
  display: flex;
  justify-content: flex-end;
  gap: 0.625rem;
}

.change-acc__btn-secondary {
  min-width: 88px;
}

.change-acc__btn-primary {
  display: inline-flex;
  align-items: center;
  gap: 0.375rem;
  min-width: 96px;
  padding: 0.5rem 1rem;
  font-size: 0.875rem;
  font-weight: 700;
  justify-content: center;
}
</style>
