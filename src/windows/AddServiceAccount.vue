<script setup lang="ts">
/**
 * Add Service Account dialog (P12.2 D3).
 *
 * # WPF parity
 *
 * Mirrors `Beanfun/Windows/AddServiceAccount.xaml(.cs)`. WPF showed
 * this as a top-level `Window` opened by
 * `AccountList.btnAddServiceAccount_Click`. The SPA renders it as
 * an in-page `<el-dialog>` modal — see the "modal vs new window"
 * decision in `Todo.md` P12.2 D3 rationale.
 *
 * Functional parity (1:1 with WPF `ButtonOk_Click`):
 *
 * 1. Display-name input is required; empty triggers
 *    `MsgDisplayNameNeed` toast and the dialog **stays open**.
 * 2. Terms-of-service checkbox is required; unchecked triggers
 *    `MsgTermsOfServiceNeed` toast and the dialog **stays open**.
 * 3. On both checks passing, call `account.addServiceAccount(name)`:
 *    - Returns `true` → dialog closes, list refresh is handled by
 *      the store action (parity with WPF
 *      `redrawSAccountList()` after `bfClient.GetAccounts(...)`).
 *    - Returns `false` → `MsgCreateServiceAccountFailed` toast,
 *      dialog stays open so the user can rename and retry.
 *    - Throws → `wrapCommand` already toasted the cause; dialog
 *      stays open.
 * 4. Terms-of-service hyperlink calls `account.getContract()`:
 *    - Non-empty → opens nested contract preview dialog.
 *    - Empty → `UnknownError` toast (WPF parity for the `""`
 *      branch in `aContract_Click`).
 *    - Throws → `wrapCommand` already toasted.
 *
 * # WPF deviations (intentional)
 *
 * - **Modal vs new Window**: WPF opens this as a separate `Window`
 *   because its navigation is window-based; the SPA uses an
 *   in-page modal via Element Plus `<el-dialog>` for unified
 *   focus / Pinia / a11y handling. No functional impact.
 * - **No drag-to-move**: WPF wired `Window_MouseLeftButtonDown` →
 *   `DragMove()`; modal dialogs in the SPA don't move because
 *   they're center-anchored over a backdrop. Drop-on-the-floor
 *   parity for WPF `Window_MouseLeftButtonDown`.
 * - **Unconnected-game branch deferred**: WPF
 *   `btnAddServiceAccount_Click` opens `UnconnectedGame_AddAccount`
 *   instead when `service_code/region` matches the two
 *   unconnected-game IDs (`610153/TN`, `610085/TC`). We currently
 *   have no game switcher (P12.3) so the user cannot reach those
 *   IDs; the branch will be wired when P12.3 lands the game
 *   switcher. See `AccountList.vue::handleAddAccount` docblock.
 *
 * # Mockup conflict resolution
 *
 * `mockups/AddServiceAccount.html` accidentally renders the
 * `UnconnectedGame_AddAccount` shape (display name + password +
 * confirm password + strength meter) under the connected-game
 * title. WPF separates the two: connected-game `AddServiceAccount`
 * has only display-name + terms checkbox because the server
 * generates the password. We follow WPF, not the mockup, for the
 * field set. The mockup chrome (glass header, fluent input
 * styling, gradient submit button) is preserved.
 *
 * # Contract preview
 *
 * The terms-of-service preview is delegated to the dedicated
 * `windows/Contract.vue` component (P12.2 D10.2). This dialog only
 * owns the agreement gate (`IAgree` checkbox + validation); the
 * read-only contract viewer lives next door so the same component
 * can be reused by future callers (e.g. AccountList tools dropdown
 * "view contract" surface) without duplicating the dialog chrome.
 * Plain text only — see `Contract.vue` docblock for the v-html /
 * sanitization rationale.
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
} from 'element-plus'
import { CircleClose, CirclePlus } from '@element-plus/icons-vue'

import { useAccountStore } from '../stores/account'
import Contract from './Contract.vue'

defineOptions({ name: 'AddServiceAccount' })

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
   * Emitted exactly once after `account.addServiceAccount` returns
   * `true` (and the store has refreshed the list). Caller may use
   * this to surface a success toast / focus the new row / etc.
   */
  (event: 'created', name: string): void
}>()

const { t } = useI18n()
const account = useAccountStore()

/* --------------- form state --------------- */

const displayName = ref('')
const agreed = ref(false)
const submitting = ref(false)
const displayNameInput = ref<InstanceType<typeof ElInput> | null>(null)

/* --------------- contract nested dialog state --------------- */

const contractVisible = ref(false)
const contractText = ref('')
const contractLoading = ref(false)

/* --------------- helpers --------------- */

const visible = computed({
  get: (): boolean => props.visible,
  set: (next: boolean): void => emit('update:visible', next),
})

/**
 * Reset the form to a pristine state. Called on close so re-opening
 * the dialog never shows stale input from the previous session
 * (parity with WPF `Window` constructor running fresh per open).
 */
function resetForm(): void {
  displayName.value = ''
  agreed.value = false
  submitting.value = false
  contractVisible.value = false
  contractText.value = ''
  contractLoading.value = false
}

/**
 * Auto-focus the display-name input on open. WPF set
 * `IsDefault="True"` on the OK button which implicitly puts focus
 * on the form's first input via the standard Windows tab order;
 * mirror that here so keyboard users land on the right field.
 */
watch(
  visible,
  async (next) => {
    if (next) {
      await nextTick()
      displayNameInput.value?.focus()
    }
  },
  { immediate: true },
)

/* --------------- submit --------------- */

async function handleSubmit(): Promise<void> {
  if (submitting.value) return

  /*
   * Validation order matches WPF `ButtonOk_Click` (L23-41):
   * empty display-name first, then unchecked terms. Order matters
   * because WPF only surfaces one MessageBox at a time and the
   * display-name check fires first.
   */
  const trimmed = displayName.value.trim()
  if (trimmed.length === 0) {
    ElMessage.warning(t('MsgDisplayNameNeed'))
    return
  }
  if (!agreed.value) {
    ElMessage.warning(t('MsgTermsOfServiceNeed'))
    return
  }

  submitting.value = true
  try {
    /*
     * `account.addServiceAccount` already funnels through
     * `wrapCommand`, so any thrown error is toasted by the time
     * we land in our catch block. The boolean return represents a
     * server-side business failure (e.g. quota exceeded, name
     * taken) rather than a transport / auth error — that's the
     * branch where WPF surfaced its `MsgCreateServiceAccountFailed`
     * MessageBox.
     */
    const ok = await account.addServiceAccount(trimmed)
    if (!ok) {
      ElMessage.error(t('MsgCreateServiceAccountFailed'))
      return
    }
    emit('created', trimmed)
    visible.value = false
  } catch {
    /*
     * Toasted by `wrapCommand`. Stay open so the user can adjust
     * the input (e.g. session expired → router guard already kicked
     * to `/login`, but the modal would also unmount along with the
     * AccountList page — no extra cleanup needed here).
     */
  } finally {
    submitting.value = false
  }
}

function handleCancel(): void {
  visible.value = false
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
    /* `wrapCommand` toasted the underlying cause. */
  } finally {
    contractLoading.value = false
  }
}

/* --------------- close lifecycle --------------- */

/*
 * Reset form state when the dialog finishes closing (after the
 * fade-out animation), not when the user clicks Cancel. This
 * preserves the user's input mid-animation if they re-open the
 * dialog very quickly, and matches Element Plus's recommended
 * `@closed` hook for after-animation cleanup.
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
    class="add-service-account-dialog"
    data-test="add-service-account-dialog"
    @closed="handleClosed"
  >
    <template #header>
      <div class="add-svc__header">
        <div class="add-svc__header-meta">
          <el-icon class="add-svc__header-icon" :size="20">
            <CirclePlus />
          </el-icon>
          <span class="add-svc__header-title">{{ t('AddServiceAccount') }}</span>
        </div>
        <button
          type="button"
          class="add-svc__header-close"
          :title="t('Cancel')"
          :disabled="submitting"
          data-test="add-service-account-close"
          @click="handleCancel"
        >
          <el-icon><CircleClose /></el-icon>
        </button>
      </div>
    </template>

    <el-form
      class="add-svc__form"
      label-position="top"
      hide-required-asterisk
      @submit.prevent="handleSubmit"
    >
      <el-form-item :label="t('ServiceAccountDisplayName')" class="add-svc__field">
        <el-input
          ref="displayNameInput"
          v-model="displayName"
          :disabled="submitting"
          :maxlength="32"
          show-word-limit
          autocomplete="off"
          data-test="add-service-account-name"
          @keyup.enter="handleSubmit"
        />
      </el-form-item>

      <el-form-item class="add-svc__field add-svc__terms">
        <el-checkbox v-model="agreed" :disabled="submitting" data-test="add-service-account-agree">
          <span class="add-svc__terms-text">
            {{ t('IAgree') }}
            <button
              type="button"
              class="add-svc__terms-link"
              :disabled="contractLoading"
              data-test="add-service-account-terms"
              @click.prevent.stop="handleOpenContract"
            >
              {{ t('TermsOfService') }}
            </button>
          </span>
        </el-checkbox>
      </el-form-item>
    </el-form>

    <template #footer>
      <div class="add-svc__footer">
        <el-button
          class="bf-btn-secondary add-svc__btn-secondary"
          :disabled="submitting"
          data-test="add-service-account-cancel"
          @click="handleCancel"
        >
          {{ t('Cancel') }}
        </el-button>
        <button
          type="button"
          class="bf-btn-gradient add-svc__btn-primary"
          :disabled="submitting"
          data-test="add-service-account-submit"
          @click="handleSubmit"
        >
          <el-icon><CirclePlus /></el-icon>
          <span>{{ t('Add') }}</span>
        </button>
      </div>
    </template>
  </el-dialog>

  <Contract v-model:visible="contractVisible" :text="contractText" />
</template>

<style scoped>
/* Header strip — picks up the same gradient accent as the mockup. */
.add-svc__header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.5rem;
}

.add-svc__header-meta {
  display: inline-flex;
  align-items: center;
  gap: 0.5rem;
  min-width: 0;
}

.add-svc__header-icon {
  color: var(--bf-primary);
  flex-shrink: 0;
}

.add-svc__header-title {
  font-size: 0.9375rem;
  font-weight: 700;
  color: var(--bf-on-surface);
}

.add-svc__header-close {
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

.add-svc__header-close:hover:not(:disabled) {
  background: color-mix(in srgb, var(--bf-danger) 80%, transparent);
  color: var(--bf-on-danger);
}

.add-svc__header-close:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}

/* Body form */
.add-svc__form {
  display: flex;
  flex-direction: column;
  gap: 0.25rem;
}

.add-svc__field :deep(.el-form-item__label) {
  font-size: 0.75rem;
  font-weight: 600;
  color: var(--bf-on-surface-variant);
  padding-bottom: 0.25rem;
}

.add-svc__terms :deep(.el-form-item__content) {
  align-items: flex-start;
}

.add-svc__terms-text {
  display: inline-flex;
  align-items: center;
  gap: 0.25rem;
  flex-wrap: wrap;
  font-size: 0.8125rem;
  color: var(--bf-on-surface);
}

.add-svc__terms-link {
  appearance: none;
  background: transparent;
  border: 0;
  padding: 0;
  font: inherit;
  color: var(--bf-primary);
  text-decoration: underline;
  cursor: pointer;
}

.add-svc__terms-link:disabled {
  opacity: 0.5;
  cursor: progress;
}

.add-svc__terms-link:hover:not(:disabled) {
  filter: brightness(0.9);
}

/* Footer button row */
.add-svc__footer {
  display: flex;
  justify-content: flex-end;
  gap: 0.625rem;
}

.add-svc__btn-secondary {
  min-width: 88px;
}

.add-svc__btn-primary {
  display: inline-flex;
  align-items: center;
  gap: 0.375rem;
  min-width: 96px;
  padding: 0.5rem 1rem;
  font-size: 0.875rem;
  font-weight: 700;
  justify-content: center;
}

/* Contract body lives in `windows/Contract.vue`; styles owned there. */
</style>
