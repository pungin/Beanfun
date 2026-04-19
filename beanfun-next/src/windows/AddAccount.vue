<script setup lang="ts">
/**
 * Add stored Beanfun credential dialog (P12.2 D8).
 *
 * # WPF parity
 *
 * Mirrors `Beanfun/Windows/AddAccount.xaml(.cs)`. WPF showed this
 * as a top-level `Window` opened by the manage-accounts surface;
 * the SPA renders it as an in-page `<el-dialog>` modal — same
 * "modal vs new window" decision rationale as D3
 * (`windows/AddServiceAccount.vue`).
 *
 * Functional parity (mostly 1:1 with WPF `Button_Click`,
 * `Beanfun/Windows/AddAccount.xaml.cs` L41-59):
 *
 * 1. **Region picker** — WPF `ComboBox` with `Taiwan / HongKong`
 *    items and `SelectedIndex == 0 ? "TW" : "HK"` derivation. We
 *    reuse the same two regions; `LoginRegion` IPC type is the
 *    union `'TW' | 'HK'` so no extra mapping needed.
 * 2. **Account ID** — required input. Empty triggers
 *    `MsgBox("AccountNeed")` in WPF L43-46; we toast
 *    `t('AccountNeed')` (warning) and keep the dialog open so the
 *    user can correct (same UX-tightening pattern as D4 vs WPF's
 *    "close-then-error" loop).
 * 3. **Account name** — optional alias / remark.
 * 4. **Password / Verify** — optional. Verify is **TW-only**
 *    (mirror WPF `region_SelectionChanged` → `initPage` L20-27:
 *    `if (s_Region == "TW") t_Verify.Visibility = Visible; else
 *    Collapsed + Text = ""`). The SPA uses a `v-if` toggle that
 *    *also* clears the form value when hidden — exact 1:1 with the
 *    WPF "set Text = '' when collapsed" line — so a user who fills
 *    a verify under TW then switches to HK doesn't accidentally
 *    persist a stale verify under the wrong region.
 * 5. **Auto login** — checkbox. WPF L55 has a quirk:
 *    `t_Password.Text == "" ? false : (bool)autoLogin.IsChecked`
 *    forces auto-login off whenever the password is empty (an
 *    empty password makes auto-login meaningless). We mirror the
 *    same coercion at submit time — see `effectiveAutoLogin`
 *    below — instead of disabling the checkbox in the UI, because
 *    the user might tick the checkbox first and *then* fill the
 *    password.
 * 6. **Submit** — call `account.saveAccount({ region, account_id,
 *    account_name, password, verify, method: 0, auto_login })`
 *    where `method: 0` is `LOGIN_METHOD.Regular` (mirror WPF L54
 *    hard-coded `0` — the AddAccount dialog only ever creates
 *    Regular records; QR / GamePass records get created elsewhere
 *    by the live login flow). The store wraps `commands.save_account`
 *    via `wrapCommand`; on success the store updates `accounts.value`
 *    and we emit `created` + close.
 *
 * # WPF deviations (intentional)
 *
 * - **Modal vs new Window**: same rationale as D3.
 * - **No drag-to-move**: WPF wired `Window_MouseLeftButtonDown` →
 *   `DragMove()`; modal dialogs in the SPA don't move because
 *   they're center-anchored over a backdrop.
 * - **Duplicate `(region, account_id)` block (D8 Q8 = B)**: WPF's
 *   `accountManager.addAccount` is a silent upsert — calling
 *   AddAccount on an existing `(region, id)` overwrites the saved
 *   `password` / `verify` / `auto_login` of the previous record
 *   without warning. That's a footgun in modern UX (the user who
 *   typoed an existing id loses their saved password). We block
 *   the submit with a `t('addAccountDialog.duplicateExists')` toast
 *   and keep the dialog open so the user can correct the input or
 *   route through the Edit / Delete flow instead. This is **the
 *   single non-trivial deviation from WPF parity** in this file —
 *   user-confirmed in D8 pre-flight Q8.
 * - **Mockup field set ignored**: `mockups/AddAccount.html` drops
 *   the region picker and verify field, and adds a "remember
 *   password (DPAPI)" checkbox + security note. We follow WPF
 *   (region picker + TW-only verify field, no extra remember
 *   toggle — the existence of the password field already implies
 *   "remember"; HK / NDT / NDJ regions need a real account-add
 *   path even though the mockup omits them). Mockup chrome (glass
 *   header, fluent input styling, gradient submit) is preserved.
 *
 * # Why one shared empty `password` value, no separate "remember" toggle
 *
 * WPF only has the password text box itself — the absence of text
 * is itself the "don't remember" signal (`account.password = ""`
 * means "ask the user every login"). The SPA mirrors that
 * 1-to-1: an empty password input persists as an empty password
 * field; the existing `IdPassForm` mount-time prefill code
 * already treats `""` as "no saved password". Adding a "remember"
 * checkbox would just invert the same boolean and double the
 * config surface for no functional gain.
 *
 * # Mockup conflict resolution
 *
 * The mockup's `BackdropFilter` glass-panel + gradient submit
 * button are preserved via the shared `bf-btn-gradient` /
 * `bf-btn-secondary` utility classes. The "DPAPI 加密" security
 * note from the mockup is dropped because (a) DPAPI encryption is
 * the only persistence mechanism (the note doesn't communicate a
 * meaningful choice to the user), (b) it's a chrome decoration
 * that conflicts with the WPF dialog's lean field-set UX, and
 * (c) the user always lands on the dialog deliberately from the
 * Manage Accounts page (D9) where the global help affordance can
 * surface DPAPI details if the user actively asks.
 */

import { computed, nextTick, ref, watch } from 'vue'
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
  ElOption,
  ElSelect,
} from 'element-plus'
import { CircleClose, CirclePlus } from '@element-plus/icons-vue'

import { useAccountStore } from '../stores/account'
import type { LoginRegion } from '../types/bindings'
import { LOGIN_METHOD } from '../constants/login'

defineOptions({ name: 'AddAccount' })

const props = defineProps<{
  /**
   * Two-way binding for whether the dialog is shown. Caller drives
   * open via `v-model:visible="..."`; the dialog drives close on
   * cancel / success / Esc / outside-click.
   */
  visible: boolean
}>()

const emit = defineEmits<{
  /** Two-way binding back to the caller's `visible` ref. */
  (event: 'update:visible', next: boolean): void
  /**
   * Emitted exactly once after `account.saveAccount` resolves and
   * the new record is in the store. Caller may use this to surface
   * a success toast / focus the new row / etc. The `(region, id)`
   * pair is the natural key the caller can re-derive into a row
   * lookup without re-reading the entire form payload.
   */
  (event: 'created', payload: { region: LoginRegion; accountId: string }): void
}>()

const { t } = useI18n()
/*
 * Aliased to `accountStore` to avoid a name collision with form
 * field references the template might want to call `account` —
 * keeps things consistent with D4 (`ChangeServiceAccountDisplayName.vue`).
 */
const accountStore = useAccountStore()

/* --------------- form state --------------- */

const region = ref<LoginRegion>('TW')
const accountId = ref('')
const accountName = ref('')
const password = ref('')
const verify = ref('')
const autoLogin = ref(false)
const submitting = ref(false)
const accountIdInput = ref<InstanceType<typeof ElInput> | null>(null)

/* --------------- helpers --------------- */

const visible = computed({
  get: (): boolean => props.visible,
  set: (next: boolean): void => emit('update:visible', next),
})

const showVerify = computed((): boolean => region.value === 'TW')

/**
 * Mirror WPF L55 quirk: empty password forces `auto_login = false`
 * regardless of the checkbox state. Computed so the submit payload
 * always reflects the current pair without an imperative reset.
 */
const effectiveAutoLogin = computed((): boolean => {
  if (password.value === '') return false
  return autoLogin.value
})

function resetForm(): void {
  region.value = 'TW'
  accountId.value = ''
  accountName.value = ''
  password.value = ''
  verify.value = ''
  autoLogin.value = false
  submitting.value = false
}

/**
 * WPF `region_SelectionChanged` → `initPage` clears `t_Verify.Text`
 * to `""` whenever the region collapses to non-TW. Mirror that
 * exactly so a TW-then-HK switch never persists stale verify
 * input. (`v-if` would unmount the input but Vue does *not*
 * automatically reset the bound model value — the parent ref
 * survives across hide/show cycles and would re-prime a hidden
 * verify on re-show.)
 */
watch(region, (next) => {
  if (next !== 'TW') verify.value = ''
})

/**
 * Auto-focus the account-id input on open; matches the WPF tab
 * order which would land focus on `t_AccountID` first (the
 * `IsDefault="True"` Add button doesn't grab focus, only Enter).
 *
 * Pre-prime nothing else — the dialog opens fresh per request.
 * Reset state lives in `handleClosed` (post-animation) instead so
 * a quick re-open before the close animation finishes preserves
 * the user's input.
 */
watch(
  visible,
  async (next) => {
    if (next) {
      await nextTick()
      accountIdInput.value?.focus()
    }
  },
  { immediate: true },
)

/* --------------- submit --------------- */

async function handleSubmit(): Promise<void> {
  if (submitting.value) return

  /*
   * Validation order matches WPF `Button_Click` (L43-46) — empty
   * id is the only hard-required check. The duplicate guard is a
   * D8 Q8 = B addition (see file docblock); both are toasts that
   * keep the dialog open instead of WPF's MessageBox-then-cancel
   * flow.
   */
  const trimmedId = accountId.value.trim()
  if (trimmedId.length === 0) {
    ElMessage.warning(t('AccountNeed'))
    return
  }

  if (accountStore.findStoredAccount(region.value, trimmedId) !== undefined) {
    ElMessage.error(t('addAccountDialog.duplicateExists'))
    return
  }

  submitting.value = true
  try {
    /*
     * `account.saveAccount` funnels through `wrapCommand`, so any
     * thrown IPC error already toasted by the time we land in
     * the catch block. The store also re-assigns
     * `accounts.value` to the upserted list, so the parent
     * Manage Accounts page (D9) sees the new row reactively.
     *
     * `method: LOGIN_METHOD.Regular` mirrors WPF L54 hard-coded
     * `0` — the AddAccount dialog only creates Regular records.
     * QR / GamePass records get created by the live login flow
     * (`account.saveLoginCredentials`), not by manual add.
     */
    await accountStore.saveAccount({
      region: region.value,
      account_id: trimmedId,
      account_name: accountName.value.trim(),
      password: password.value,
      verify: showVerify.value ? verify.value : '',
      method: LOGIN_METHOD.Regular,
      auto_login: effectiveAutoLogin.value,
    })
    emit('created', { region: region.value, accountId: trimmedId })
    visible.value = false
  } catch {
    /* Toasted by `wrapCommand`; stay open so the user can adjust. */
  } finally {
    submitting.value = false
  }
}

function handleCancel(): void {
  visible.value = false
}

/**
 * Reset form state when the dialog finishes closing (after the
 * fade-out animation), not on cancel click — same `@closed` hook
 * pattern as D3 / D4 dialogs so a quick re-open preserves user
 * input mid-animation.
 */
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
    :width="480"
    align-center
    append-to-body
    class="add-account-dialog"
    data-test="add-account-dialog"
    @closed="handleClosed"
  >
    <template #header>
      <div class="add-acc__header">
        <div class="add-acc__header-meta">
          <el-icon class="add-acc__header-icon" :size="20">
            <CirclePlus />
          </el-icon>
          <span class="add-acc__header-title">{{ t('AddAccount') }}</span>
        </div>
        <button
          type="button"
          class="add-acc__header-close"
          :title="t('Cancel')"
          :disabled="submitting"
          data-test="add-account-close"
          @click="handleCancel"
        >
          <el-icon><CircleClose /></el-icon>
        </button>
      </div>
    </template>

    <p class="add-acc__subtitle">{{ t('addAccountDialog.subtitle') }}</p>

    <el-form
      class="add-acc__form"
      label-position="top"
      hide-required-asterisk
      @submit.prevent="handleSubmit"
    >
      <el-form-item :label="t('addAccountDialog.regionLabel')" class="add-acc__field">
        <el-select
          v-model="region"
          :disabled="submitting"
          data-test="add-account-region"
          class="add-acc__region"
        >
          <el-option value="TW" :label="t('Taiwan')" />
          <el-option value="HK" :label="t('HongKong')" />
        </el-select>
      </el-form-item>

      <el-form-item :label="t('addAccountDialog.accountIdLabel')" class="add-acc__field">
        <el-input
          ref="accountIdInput"
          v-model="accountId"
          :disabled="submitting"
          :placeholder="t('tbBeanfunAccount')"
          autocomplete="off"
          data-test="add-account-id"
          @keyup.enter="handleSubmit"
        />
      </el-form-item>

      <el-form-item :label="t('addAccountDialog.accountNameLabel')" class="add-acc__field">
        <el-input
          v-model="accountName"
          :disabled="submitting"
          :placeholder="t('tbBeanfunRemark')"
          autocomplete="off"
          data-test="add-account-name"
          @keyup.enter="handleSubmit"
        />
      </el-form-item>

      <el-form-item :label="t('addAccountDialog.passwordLabel')" class="add-acc__field">
        <el-input
          v-model="password"
          type="password"
          show-password
          :disabled="submitting"
          :placeholder="t('tbBeanfunPassword')"
          autocomplete="new-password"
          data-test="add-account-password"
          @keyup.enter="handleSubmit"
        />
      </el-form-item>

      <el-form-item
        v-if="showVerify"
        :label="t('addAccountDialog.verifyLabel')"
        class="add-acc__field"
      >
        <el-input
          v-model="verify"
          :disabled="submitting"
          :placeholder="t('tbBeanfunAuthInfo')"
          autocomplete="off"
          data-test="add-account-verify"
          @keyup.enter="handleSubmit"
        />
      </el-form-item>

      <el-form-item class="add-acc__field add-acc__autologin">
        <el-checkbox v-model="autoLogin" :disabled="submitting" data-test="add-account-autologin">
          {{ t('AutoLogin') }}
        </el-checkbox>
      </el-form-item>
    </el-form>

    <template #footer>
      <div class="add-acc__footer">
        <el-button
          class="bf-btn-secondary add-acc__btn-secondary"
          :disabled="submitting"
          data-test="add-account-cancel"
          @click="handleCancel"
        >
          {{ t('Cancel') }}
        </el-button>
        <button
          type="button"
          class="bf-btn-gradient add-acc__btn-primary"
          :disabled="submitting"
          data-test="add-account-submit"
          @click="handleSubmit"
        >
          <el-icon><CirclePlus /></el-icon>
          <span>{{ t('addAccountDialog.save') }}</span>
        </button>
      </div>
    </template>
  </el-dialog>
</template>

<style scoped>
.add-acc__header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.5rem;
}

.add-acc__header-meta {
  display: inline-flex;
  align-items: center;
  gap: 0.5rem;
  min-width: 0;
}

.add-acc__header-icon {
  color: var(--bf-primary);
  flex-shrink: 0;
}

.add-acc__header-title {
  font-size: 0.9375rem;
  font-weight: 700;
  color: var(--bf-on-surface);
}

.add-acc__header-close {
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

.add-acc__header-close:hover:not(:disabled) {
  background: color-mix(in srgb, var(--bf-danger) 80%, transparent);
  color: var(--bf-on-danger);
}

.add-acc__header-close:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}

.add-acc__subtitle {
  margin: 0 0 0.75rem;
  font-size: 0.8125rem;
  color: var(--bf-on-surface-variant);
}

.add-acc__form {
  display: flex;
  flex-direction: column;
  gap: 0.25rem;
}

.add-acc__field :deep(.el-form-item__label) {
  font-size: 0.75rem;
  font-weight: 600;
  color: var(--bf-on-surface-variant);
  padding-bottom: 0.25rem;
}

.add-acc__region {
  width: 100%;
}

.add-acc__autologin :deep(.el-form-item__content) {
  align-items: center;
}

.add-acc__footer {
  display: flex;
  justify-content: flex-end;
  gap: 0.625rem;
}

.add-acc__btn-secondary {
  min-width: 88px;
}

.add-acc__btn-primary {
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
