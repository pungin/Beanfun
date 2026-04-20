<script setup lang="ts">
/**
 * Change Service Account display-name dialog (P12.2 D4).
 *
 * # WPF parity
 *
 * Mirrors `Beanfun/Windows/ChangeServiceAccountDisplayName.xaml(.cs)`
 * + the orchestration in `MainWindow.xaml.cs::ChangeServiceAccountDisplayName`
 * (L2060-2081). WPF showed this as a top-level `Window` opened by
 * `AccountList.m_ChangeAccName_Click` (`AccountList.xaml.cs` L142+).
 * The SPA renders it as an in-page `<el-dialog>` modal — same
 * "modal vs new window" decision rationale as D3
 * (`windows/AddServiceAccount.vue`).
 *
 * Functional parity (1:1 with WPF `ButtonOk_Click` +
 * `MainWindow.ChangeServiceAccountDisplayName`):
 *
 * 1. Display-name input is pre-filled with `account.sname` on open
 *    (WPF: `txtNewServiceAccountDisplayName.Text = name;` in the
 *    constructor).
 * 2. Empty input is rejected. WPF returns `false` from
 *    `MainWindow.ChangeServiceAccountDisplayName` and surfaces
 *    `MsgChangeDisplayNameError` after closing the dialog. The SPA
 *    tightens this UX: we toast `MsgDisplayNameNeed` (warning) and
 *    keep the dialog open so the user can correct the input. The
 *    "close-then-error" loop in WPF is a known UX papercut; our
 *    SPA path is strictly better and uses the same key set the WPF
 *    Add dialog already uses for empty input.
 * 3. Unchanged input short-circuit: when `trimmed === account.sname`
 *    we close the dialog without invoking the command (WPF L2068-69:
 *    `if (newName == account.sname) return true;`). No toast, no
 *    server round-trip, no `updated` event.
 * 4. On both checks passing, call
 *    `account.changeServiceAccountName(trimmed, account)`:
 *    - Returns `true` → dialog closes, list refresh is handled by
 *      the store action (parity with WPF L2076 `redrawSAccountList()`
 *      after the in-place sname mutation). `updated` event fires.
 *    - Returns `false` → `MsgChangeDisplayNameError` toast, dialog
 *      stays open so the user can adjust the name and retry.
 *    - Throws → `wrapCommand` already toasted the cause; dialog
 *      stays open.
 *
 * # WPF deviations (intentional)
 *
 * - **Modal vs new Window**: same rationale as D3.
 * - **No drag-to-move**: WPF wired `Window_MouseLeftButtonDown` →
 *   `DragMove()`; modal dialogs in the SPA don't move because
 *   they're center-anchored over a backdrop.
 * - **Empty-input loop fix**: see point 2 above. WPF closes the
 *   dialog then shows MsgChangeDisplayNameError, which forces the
 *   user to re-open the menu and re-trigger the dialog. We keep the
 *   dialog open and warn-toast. Functional contract preserved
 *   (server is not called with empty input).
 * - **Maxlength 32**: tightened SPA-side. WPF has no client-side
 *   length cap; the server eventually rejects overlong names but
 *   the round-trip wastes the user's time. 32 chars matches the
 *   D3 Add dialog and the longest WPF-rendered sname observed in
 *   QA traces.
 * - **No success toast**: WPF doesn't toast on success — the list
 *   refresh is the visual confirmation. We mirror that exactly to
 *   avoid double-affirming a happy path. Caller can still listen
 *   to the `updated` event to surface its own affordance if needed.
 *
 * # Mockup conflict resolution
 *
 * `mockups/ChangeServiceAccountDisplayName.html` claims the rename
 * is "僅顯示於本機，不會同步到伺服器" (local only, not synced).
 * That is **wrong**. WPF posts to `gamezone.ashx` with
 * `strFunction=ChangeServiceAccountDisplayName` (`BeanfunClient.Account.cs::ChangeServiceAccountDisplayName`)
 * and the new name is persisted server-side; the WPF
 * `redrawSAccountList()` then re-fetches and the local list reflects
 * the updated name. We follow WPF (server-side change) and omit the
 * misleading "local only" copy. The dialog chrome (glass header,
 * fluent input, gradient submit) is preserved.
 *
 * # Why the dialog accepts the full ServiceAccount, not just sname
 *
 * `commands.changeDisplayName(newName, account)` requires the full
 * service-account record (it forwards `said` / `sl` to the server
 * — see `BeanfunClient.Account.cs`). Passing only `sname` here
 * would force the dialog to look up the account from the store,
 * which is brittle: the user could click "Change Alias" on row A,
 * then click row B before the dialog opens, and the wrong record
 * would be renamed. Threading the account object end-to-end keeps
 * the dialog's target unambiguous regardless of post-open store
 * mutations (matches the D3 `<AddServiceAccount />` pattern's
 * principle of "explicit input over store coupling").
 */

import { computed, nextTick, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { ElButton, ElDialog, ElForm, ElFormItem, ElIcon, ElInput, ElMessage } from 'element-plus'
import { Check, CircleClose, EditPen } from '@element-plus/icons-vue'

import { useAccountStore } from '../stores/account'
import type { ServiceAccount } from '../types/bindings'

defineOptions({ name: 'ChangeServiceAccountDisplayName' })

const props = defineProps<{
  /**
   * Two-way binding for whether the dialog is shown. Caller drives
   * open via `v-model:visible="..."`; the dialog drives close on
   * cancel / success / Esc / outside-click.
   */
  visible: boolean
  /**
   * The service account to rename. `null` while the dialog is
   * closed (the caller clears it after the row context-menu fires
   * to avoid leaking stale account references between sessions).
   * The dialog is rendered as a no-op shell when `account === null`
   * so the v-model binding round-trips cleanly.
   */
  account: ServiceAccount | null
}>()

const emit = defineEmits<{
  /** Two-way binding back to the caller's `visible` ref. */
  (event: 'update:visible', next: boolean): void
  /**
   * Emitted exactly once after `account.changeServiceAccountName`
   * returns `true` (and the store has refreshed the list). Caller
   * may use this to surface a success toast / re-arm
   * `selectedSid` / etc. The new name is forwarded for convenience
   * so the caller doesn't need to re-derive it.
   */
  (event: 'updated', payload: { sid: string; newName: string }): void
}>()

const { t } = useI18n()
/*
 * Aliased to `accountStore` (rather than the more natural
 * `account`) to avoid a name collision with the `account` prop —
 * `vue/no-dupe-keys` flags the duplicate.
 */
const accountStore = useAccountStore()

/* --------------- form state --------------- */

const displayName = ref('')
const submitting = ref(false)
const displayNameInput = ref<InstanceType<typeof ElInput> | null>(null)

/* --------------- helpers --------------- */

const visible = computed({
  get: (): boolean => props.visible,
  set: (next: boolean): void => emit('update:visible', next),
})

/**
 * Reset the form state. Called when the dialog finishes closing
 * (after the fade-out animation) so re-opening the dialog never
 * shows stale input from the previous session.
 */
function resetForm(): void {
  displayName.value = ''
  submitting.value = false
}

/**
 * Pre-fill the input with the target's current sname every time
 * the dialog opens. WPF's constructor ran fresh per open
 * (`txtNewServiceAccountDisplayName.Text = name`) — mirror that
 * here by re-priming on the `false → true` transition rather than
 * on first mount, so the dialog tracks whatever the caller passes
 * in even if a previous open was for a different account.
 *
 * Auto-focus the input after pre-fill. WPF set the OK button as
 * `IsDefault="True"` which puts focus on the form's first input
 * via the standard Windows tab order; mirror that here so keyboard
 * users land on the right field. We use a microtask after render
 * to guarantee the `<el-input>` ref is mounted.
 */
watch(
  visible,
  async (next) => {
    if (next) {
      displayName.value = props.account?.sname ?? ''
      await nextTick()
      displayNameInput.value?.focus()
    }
  },
  { immediate: true },
)

/* --------------- submit --------------- */

async function handleSubmit(): Promise<void> {
  if (submitting.value) return

  const target = props.account
  if (!target) return

  /*
   * Validation order matches WPF
   * `MainWindow.ChangeServiceAccountDisplayName` (L2066-69):
   * empty first, then the unchanged-name short-circuit. The SPA
   * tightens "empty → MsgChangeDisplayNameError after dialog
   * close" into "empty → MsgDisplayNameNeed warning, dialog stays
   * open" — see the docblock at the top of this file.
   */
  const trimmed = displayName.value.trim()
  if (trimmed.length === 0) {
    ElMessage.warning(t('MsgDisplayNameNeed'))
    return
  }

  /*
   * Unchanged-name short-circuit (WPF L2068-69:
   * `if (newName == account.sname) return true;`). No server
   * round-trip, no toast, no `updated` event — the dialog simply
   * closes as if the user had cancelled.
   */
  if (trimmed === target.sname) {
    visible.value = false
    return
  }

  submitting.value = true
  try {
    /*
     * `account.changeServiceAccountName` already funnels through
     * `wrapCommand`, so any thrown error is toasted by the time
     * we land in our catch block. The boolean return represents a
     * server-side business failure (e.g. name already taken,
     * forbidden character) rather than a transport / auth error
     * — that's the branch where WPF surfaced its
     * `MsgChangeDisplayNameError` MessageBox.
     */
    const ok = await accountStore.changeServiceAccountName(trimmed, target)
    if (!ok) {
      ElMessage.error(t('MsgChangeDisplayNameError'))
      return
    }
    emit('updated', { sid: target.sid, newName: trimmed })
    visible.value = false
  } catch {
    /*
     * Toasted by `wrapCommand`. Stay open so the user can adjust
     * the input (e.g. session expired → router guard already
     * kicked to `/login`, but the modal would also unmount along
     * with the AccountList page — no extra cleanup needed here).
     */
  } finally {
    submitting.value = false
  }
}

function handleCancel(): void {
  visible.value = false
}

/* --------------- close lifecycle --------------- */

/*
 * Reset form state when the dialog finishes closing (after the
 * fade-out animation), not on cancel click. This preserves the
 * user's input mid-animation if they re-open the dialog very
 * quickly, and matches Element Plus's recommended `@closed` hook
 * for after-animation cleanup.
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
    :width="440"
    align-center
    append-to-body
    class="change-display-name-dialog"
    data-test="change-display-name-dialog"
    @closed="handleClosed"
  >
    <template #header>
      <div class="change-dn__header">
        <div class="change-dn__header-meta">
          <el-icon class="change-dn__header-icon" :size="20">
            <EditPen />
          </el-icon>
          <span class="change-dn__header-title">{{ t('ChangeAccountName') }}</span>
        </div>
        <button
          type="button"
          class="change-dn__header-close"
          :title="t('Cancel')"
          :disabled="submitting"
          data-test="change-display-name-close"
          @click="handleCancel"
        >
          <el-icon><CircleClose /></el-icon>
        </button>
      </div>
    </template>

    <el-form
      class="change-dn__form"
      label-position="top"
      hide-required-asterisk
      @submit.prevent="handleSubmit"
    >
      <el-form-item :label="t('ServiceAccountDisplayName')" class="change-dn__field">
        <el-input
          ref="displayNameInput"
          v-model="displayName"
          :disabled="submitting"
          :maxlength="32"
          show-word-limit
          autocomplete="off"
          data-test="change-display-name-input"
          @keyup.enter="handleSubmit"
        />
      </el-form-item>
    </el-form>

    <template #footer>
      <div class="change-dn__footer">
        <el-button
          class="bf-btn-secondary change-dn__btn-secondary"
          :disabled="submitting"
          data-test="change-display-name-cancel"
          @click="handleCancel"
        >
          {{ t('Cancel') }}
        </el-button>
        <button
          type="button"
          class="bf-btn-gradient change-dn__btn-primary"
          :disabled="submitting"
          data-test="change-display-name-submit"
          @click="handleSubmit"
        >
          <el-icon><Check /></el-icon>
          <span>{{ t('EditAccountSave') }}</span>
        </button>
      </div>
    </template>
  </el-dialog>
</template>

<style scoped>
.change-dn__header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.5rem;
}

.change-dn__header-meta {
  display: inline-flex;
  align-items: center;
  gap: 0.5rem;
  min-width: 0;
}

.change-dn__header-icon {
  color: var(--bf-primary);
  flex-shrink: 0;
}

.change-dn__header-title {
  font-size: 0.9375rem;
  font-weight: 700;
  color: var(--bf-on-surface);
}

.change-dn__header-close {
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

.change-dn__header-close:hover:not(:disabled) {
  background: color-mix(in srgb, var(--bf-danger) 80%, transparent);
  color: var(--bf-on-danger);
}

.change-dn__header-close:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}

.change-dn__form {
  display: flex;
  flex-direction: column;
  gap: 0.25rem;
}

.change-dn__field :deep(.el-form-item__label) {
  font-size: 0.75rem;
  font-weight: 600;
  color: var(--bf-on-surface-variant);
  padding-bottom: 0.25rem;
}

.change-dn__footer {
  display: flex;
  justify-content: flex-end;
  gap: 0.625rem;
}

.change-dn__btn-secondary {
  min-width: 88px;
}

.change-dn__btn-primary {
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
