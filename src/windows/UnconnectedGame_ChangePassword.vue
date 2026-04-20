<script setup lang="ts">
/**
 * Unconnected-game change-password dialog (P12.3 D7).
 *
 * # WPF parity
 *
 * Mirrors `Beanfun/Windows/UnconnectedGame_ChangePassword.xaml(.cs)`,
 * the dialog `MainWindow.ResetPassword_Click` opens for a service
 * account on an unconnected game. The WPF surface is intentionally
 * tiny — one email field, one Confirm button, one inline red error
 * label — and the SPA mirrors that shape 1:1:
 *
 * 1. User types their previously-verified email into the single
 *    text field.
 * 2. Confirm calls [`commands.unconnectedGameChangePassword`] with
 *    `(accountIndex, email)`. WPF passed
 *    `accountList.list_Account.SelectedIndex` as the second arg —
 *    the SPA receives the same row index via the `accountIndex`
 *    prop the parent (`AccountList.vue` D8) sets when opening
 *    this dialog. `service_code` / `service_region` are pulled
 *    from the active session by the backend (parity with WPF
 *    `MainWindow.service_code/region`), so no extra props.
 * 3. Backend response is the [`ChangePasswordOutcome`] tagged
 *    union:
 *
 *    - `kind === 'verify_code_sent'` ⇒ surface the WPF
 *      `MsgChangePassword` blocking confirm via
 *      `ElMessageBox.alert(message, DataSended)`. Message body
 *      interpolates `data` (the verification token) into the WPF
 *      `{0}` placeholder. After the user dismisses, close the
 *      dialog. Mirrors WPF L32-39.
 *    - `kind === 'error_message'` ⇒ write `data` verbatim into
 *      the inline `lblErrorMessage` red label and stay open
 *      (mirrors WPF L43-44).
 *
 *    A backend `Err(LoginError::Unknown)` (the catch-all WPF used
 *    for `result == null`) bubbles through `wrapCommand` as a
 *    localized toast — strictly more informative than WPF's
 *    generic `MessageBox(UnknownError)` because the Rust side
 *    distinguishes transport / parse / missing-payload causes.
 *
 * # WPF deviations (intentional)
 *
 * - **Modal vs new Window**: same rationale as the rest of
 *   `windows/*.vue`; SPA renders an `<el-dialog>` instead of a
 *   top-level `Window`. No functional impact.
 * - **No drag-to-move**: meaningless inside a modal layered over
 *   a backdrop.
 * - **`ElMessageBox.alert` vs WPF `MessageBox.Show`**: both are
 *   blocking confirm dialogs that require an explicit click to
 *   dismiss. Element Plus's variant additionally supports a
 *   render function so we can preserve the WPF multiline body
 *   (`\r\n` separators) without `dangerouslyUseHTMLString` —
 *   we wrap the unescaped text in a `<pre>` VNode for honest
 *   plaintext rendering.
 *
 * # Mockup conflict resolution (P12.3 plan, user-approved)
 *
 * `mockups/UnconnectedGame_ChangePassword.html` reshapes the
 * dialog into a three-field "current pwd / new pwd / confirm
 * new pwd" form, fundamentally misunderstanding the WPF flow:
 *
 * - **WPF design**: the upstream Beanfun account-management
 *   endpoint is **email-based reset** — the user cannot supply a
 *   new password directly; the server emails a verification
 *   token back, which the user then pastes into the Beanfun
 *   "verify code" page (separate flow). The dialog only collects
 *   the email and surfaces the token.
 * - **Mockup design**: imagines a self-service "current → new"
 *   flow that has no backend implementation. Implementing it
 *   would mean inventing IPC + a new Beanfun endpoint that does
 *   not exist.
 *
 * We follow WPF: single email field, server-side verify token
 * surfaced in an alert. Mockup chrome (glass header + rounded
 * input) is preserved.
 *
 * # i18n note: `MsgChangePassword` `\r\n` unescape
 *
 * The WPF resource value is `"...\\r\\n..."` in the JSON locale
 * tree (literal backslash-r-backslash-n), matching the
 * .resx / .xaml string-table format. WPF dialogs normalise it
 * via `Regex.Unescape(TryFindResource(...))`. We do the same
 * client-side via {@link unescapeWpfCRLF} so the alert renders
 * three line breaks, not the literal `\r\n` text.
 *
 * # State / lifecycle
 *
 * - `email` resets on close (`@closed` after fade-out) so a
 *   reopen never shows the previous attempt.
 * - `errorMessage` follows the same reset; the alert path closes
 *   the dialog before the error path can touch it.
 * - `submitting` guards against double-Enter / double-click
 *   issuing duplicate requests (WPF had no equivalent guard
 *   because `MessageBox.Show` was modal-blocking by default).
 */

import { computed, h, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { ElButton, ElDialog, ElForm, ElFormItem, ElIcon, ElInput, ElMessageBox } from 'element-plus'
import { CircleClose, Key } from '@element-plus/icons-vue'

import { commands } from '../types/bindings'
import { wrapCommand } from '../services/invoke'

defineOptions({ name: 'UnconnectedGameChangePasswordDialog' })

const props = defineProps<{
  /**
   * Two-way binding for whether the dialog is shown. Caller drives
   * open via `v-model:visible="..."`; the dialog drives close on
   * Esc / outside-click / cancel / successful verify-code prompt
   * dismissal.
   */
  visible: boolean
  /**
   * 0-based row index inside the parent `gvServiceAccountList`
   * the user invoked "change password" on. Mirrors WPF's
   * `accountList.list_Account.SelectedIndex` argument the
   * `MainWindow.UnconnectedGame_ChangePassword` shim threads
   * through to `bfClient.UnconnectedGame_ChangePassword`. The
   * backend has no other way to tell which row the action
   * originated from, so the index has to flow through IPC.
   */
  accountIndex: number
}>()

const emit = defineEmits<{
  /** Two-way binding back to the caller's `visible` ref. */
  (event: 'update:visible', next: boolean): void
  /**
   * Emitted exactly once after the user dismisses the
   * `MsgChangePassword` alert (the verify-code success path).
   * Caller (`AccountList.vue` D8) can use this to refresh the
   * account list / surface a follow-up toast — parity with
   * WPF `this.Close()` after the alert.
   */
  (event: 'verify-code-sent'): void
}>()

const { t } = useI18n()

/* --------------- form state --------------- */

const email = ref('')
const errorMessage = ref('')
const submitting = ref(false)

/* --------------- helpers --------------- */

const visible = computed({
  get: (): boolean => props.visible,
  set: (next: boolean): void => emit('update:visible', next),
})

/**
 * Convert WPF-style escaped CR/LF (`\r\n` literal characters in
 * the i18n string) into actual newlines so `ElMessageBox.alert`'s
 * `<pre>` body renders the line breaks correctly.
 *
 * Matches the WPF `Regex.Unescape(TryFindResource(...))` call
 * but is intentionally narrower: we only unescape `\r\n` and
 * `\n` because that's the only escape sequence the WPF
 * `MsgChangePassword` resource uses. A general regex-unescape
 * would also handle `\t`, `\\`, hex / unicode escapes — none
 * of which appear in the localized strings we surface here, so
 * adding them would just bloat the helper.
 */
function unescapeWpfCRLF(s: string): string {
  return s.replace(/\\r\\n/g, '\n').replace(/\\n/g, '\n')
}

/* --------------- submit --------------- */

async function handleSubmit(): Promise<void> {
  if (submitting.value) return
  submitting.value = true
  try {
    const outcome = await wrapCommand(
      commands.unconnectedGameChangePassword(props.accountIndex, email.value),
    )
    if (outcome.kind === 'verify_code_sent') {
      /*
       * Build the WPF MsgChangePassword body with `{0}` replaced
       * by the verification token, then unescape the literal
       * `\r\n` separators. Render via a `<pre>` VNode so the
       * line breaks survive (`ElMessageBox.alert` accepts either
       * a string — collapsed on whitespace — or a VNode for
       * explicit rendering).
       */
      const raw = t('MsgChangePassword', [outcome.data])
      const body = unescapeWpfCRLF(raw)
      try {
        await ElMessageBox.alert(
          h(
            'pre',
            {
              style:
                'white-space: pre-wrap; word-break: break-word; margin: 0; font-family: inherit;',
              'data-test': 'unconnected-game-change-password-success-body',
            },
            body,
          ),
          t('DataSended'),
          { confirmButtonText: t('Confirm') },
        )
      } catch {
        /*
         * `ElMessageBox.alert` resolves on confirm and rejects on
         * Esc / outside-click. WPF only exposes a confirm button
         * (Esc still dismisses), so all dismissal paths should
         * behave identically — close the dialog regardless.
         */
      }
      errorMessage.value = ''
      emit('verify-code-sent')
      visible.value = false
      return
    }
    /*
     * `outcome.kind === 'error_message'` — WPF parity: write the
     * verbatim server message into the inline `lblErrorMessage`
     * label. Stay open so the user can adjust the email and
     * retry.
     */
    errorMessage.value = outcome.data
  } catch {
    /* `wrapCommand` toasted the underlying cause. Stay open. */
  } finally {
    submitting.value = false
  }
}

/* --------------- close lifecycle --------------- */

function handleCancel(): void {
  if (submitting.value) return
  visible.value = false
}

/**
 * Reset every piece of dialog state on close (after the fade-out
 * animation, via `<el-dialog @closed>`). Mirrors WPF's "new
 * Window per open" lifecycle so reopening starts pristine.
 */
function handleClosed(): void {
  email.value = ''
  errorMessage.value = ''
  submitting.value = false
}

/*
 * Keep the inline error reset opportunity available even before
 * the dialog finishes its fade-out: when `visible` flips from
 * false to true (dialog reopened) we proactively clear the
 * stashed error so the user does not see a stale message during
 * the entrance animation. The fade-out reset still runs via
 * `handleClosed` — this is just a defensive belt-and-suspenders
 * for the rare case the parent reopens with the same `visible`
 * ref instance before the previous fade-out finished.
 */
watch(
  visible,
  (next, prev) => {
    if (next && prev !== true) {
      errorMessage.value = ''
    }
  },
  { immediate: true },
)
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
    class="ucpw-dialog"
    data-test="unconnected-game-change-password-dialog"
    @closed="handleClosed"
  >
    <template #header>
      <div class="ucpw__header">
        <div class="ucpw__header-meta">
          <el-icon class="ucpw__header-icon" :size="20">
            <Key />
          </el-icon>
          <span class="ucpw__header-title" data-test="unconnected-game-change-password-title">
            {{ t('ChangePassword') }}
          </span>
        </div>
        <button
          type="button"
          class="ucpw__header-close"
          :title="t('Cancel')"
          :disabled="submitting"
          data-test="unconnected-game-change-password-close"
          @click="handleCancel"
        >
          <el-icon><CircleClose /></el-icon>
        </button>
      </div>
    </template>

    <el-form
      class="ucpw__form"
      label-position="top"
      hide-required-asterisk
      @submit.prevent="handleSubmit"
    >
      <el-form-item :label="t('AuthEmailNeed')" class="ucpw__field">
        <el-input
          v-model="email"
          :disabled="submitting"
          autocomplete="email"
          data-test="unconnected-game-change-password-email"
          @keyup.enter="handleSubmit"
        />
      </el-form-item>

      <p
        v-if="errorMessage !== ''"
        class="ucpw__error"
        data-test="unconnected-game-change-password-error"
        role="alert"
      >
        {{ errorMessage }}
      </p>
    </el-form>

    <template #footer>
      <div class="ucpw__footer">
        <el-button
          class="bf-btn-secondary ucpw__btn-cancel"
          :disabled="submitting"
          data-test="unconnected-game-change-password-cancel"
          @click="handleCancel"
        >
          {{ t('Cancel') }}
        </el-button>
        <el-button
          class="bf-btn-primary ucpw__btn-submit"
          :loading="submitting"
          data-test="unconnected-game-change-password-submit"
          @click="handleSubmit"
        >
          {{ t('AuthConfirm') }}
        </el-button>
      </div>
    </template>
  </el-dialog>
</template>

<style scoped>
.ucpw__header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.5rem;
}

.ucpw__header-meta {
  display: inline-flex;
  align-items: center;
  gap: 0.5rem;
  min-width: 0;
}

.ucpw__header-icon {
  color: var(--bf-primary);
  flex-shrink: 0;
}

.ucpw__header-title {
  font-size: 0.9375rem;
  font-weight: 700;
  color: var(--bf-on-surface);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.ucpw__header-close {
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

.ucpw__header-close:hover:not(:disabled) {
  background: color-mix(in srgb, var(--bf-danger) 80%, transparent);
  color: var(--bf-on-danger);
}

.ucpw__header-close:disabled {
  opacity: 0.45;
  cursor: not-allowed;
}

.ucpw__form {
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
}

.ucpw__field :deep(.el-form-item__label) {
  font-size: 0.75rem;
  color: var(--bf-on-surface-variant);
  padding-bottom: 0.25rem;
}

.ucpw__error {
  margin: 0;
  font-size: 0.8125rem;
  color: var(--bf-danger, #ba1a1a);
  text-align: center;
}

.ucpw__footer {
  display: flex;
  justify-content: flex-end;
  gap: 0.625rem;
}
</style>
