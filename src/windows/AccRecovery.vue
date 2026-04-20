<script setup lang="ts">
/**
 * Account-recovery dialog (P12.2 D10.3) — AES-128-CBC backup +
 * restore for the local `Users.dat` credential store.
 *
 * # WPF parity
 *
 * Mirrors `Beanfun/Windows/AccRecovery.xaml(.cs)`:
 *
 * - Two text inputs: `Password` + `Data` (the AES ciphertext, base64).
 * - Two action buttons: `Export` (encrypt the current Users.dat with
 *   `Password`, place the base64 ciphertext into `Data` so the user
 *   can copy/paste it elsewhere) and `Recovery` (decrypt the
 *   user-supplied `Data` with `Password` and overwrite `Users.dat`).
 * - WPF flow on Recovery success calls `App.MainWnd.loginMethodInit()`
 *   to re-render the login dropdown after the restore — the SPA emits
 *   `restored` so the parent (e.g. `pages/ManageAccount.vue`) can
 *   re-`loadAccounts()` for the same effect.
 * - WPF flow on Recovery error → `MsgDecryptFailed` (catch-all from
 *   the `try` around `decrypt + UTF-8 decode`) or `RecoveryFailed`
 *   (when the inner `accMan.importRecord` returns `false` — the
 *   "decrypt succeeded but the JSON is bad / persistence broke"
 *   branch). Backend mirrors that split: see `commands/storage.rs`
 *   docblock for the full `aes_backup.*` → WPF-key mapping.
 *
 * Wire format is byte-for-byte compatible with WPF, so a backup
 * exported by either launcher restores cleanly in the other (see
 * `services/storage/aes_backup.rs` for the spec + reference vectors).
 *
 * # WPF deviations (intentional)
 *
 * - **Modal vs new Window**: same rationale as the rest of
 *   `windows/*.vue` — SPA renders dialogs in-page via `el-dialog`.
 * - **No drag-to-move**: meaningless inside a modal; omitted.
 * - **Textarea instead of single-line `TextBox`**: WPF's `t_Data`
 *   was `Width="200"` single-line which forced the user to scroll
 *   horizontally through long base64 strings. We use an
 *   autosize-textarea so the ciphertext is fully visible without a
 *   horizontal scroll race. Behaviour identical (the textbox just
 *   holds a string; line breaks inside the user-pasted ciphertext
 *   are ignored by `base64::decode_engine` after standard whitespace
 *   trimming, so accidental wraps from email clients don't break
 *   restore). Same trim behaviour Rust's `STANDARD` decoder applies.
 * - **Disabled buttons when password empty**: WPF would let you
 *   click Export with an empty password (then encrypt with the
 *   MD5-of-empty-string key — a well-known `d41d8cd9...` constant
 *   that anyone can decrypt). We disable both buttons until the
 *   password field is non-empty so users don't accidentally produce
 *   "encrypted" backups that aren't actually private. This is a
 *   strict UX-tightening that does not break any WPF backup that
 *   used a real password.
 * - **In-flight guard**: WPF lets you spam the button. We disable
 *   both during an in-flight call to prevent racing the IPC.
 * - **Reset on close**: WPF reused the same `Window` instance until
 *   it was closed (i.e. password / ciphertext stayed if the user
 *   re-opened mid-session). We reset on close so a fresh open never
 *   leaks the previous session's password into the form (mirrors
 *   the `AddServiceAccount.vue` reset-on-`@closed` pattern).
 *
 * # Mockup conflict resolution
 *
 * `mockups/AccRecovery.html` paints this as a "forgot password /
 * forgot account / reported stolen / contact support" link launcher
 * — completely orthogonal UX with no AES surface at all. We follow
 * WPF (P12.2 D10 Q11 = WPF parity, user-approved): the dialog is the
 * AES backup/restore tool, full stop. The forgot-password / contact
 * surfaces, if needed, will land later as separate help links from
 * `LoginPage.vue` and don't belong inside this dialog.
 *
 * # Threat model / weak crypto warning
 *
 * The on-disk wire format (MD5-derived 128-bit key + IV from the
 * literal ASCII string `"pungin"`) has **multiple** modern crypto
 * shortcomings:
 *
 * 1. MD5 is a broken hash function — preimage attacks against weak
 *    passwords are practical on commodity GPUs.
 * 2. The IV is a constant derived from a hard-coded salt, which
 *    means semantic security against a chosen-plaintext adversary
 *    is broken.
 * 3. AES-128-CBC + PKCS7 with no MAC is malleable and vulnerable
 *    to padding oracles.
 *
 * We **deliberately preserve** this format because cross-launcher
 * backup compatibility matters for users migrating from WPF. The
 * `aes_backup` Rust module's docblock spells out the constraint and
 * advises against treating these backups as a primary security
 * boundary — they are a portability tool, not a vault. A future
 * P12.X "v2 backup format" would need a parallel command and
 * version-tagging in the wire format; out of scope for D10.
 *
 * # Error mapping (WPF parity)
 *
 * | Backend code                                | Toast key        |
 * |---------------------------------------------|------------------|
 * | `storage.aes_backup_invalid_ciphertext`     | `MsgDecryptFailed` |
 * | `storage.aes_backup_decrypt_failed`         | `MsgDecryptFailed` |
 * | `storage.aes_backup_invalid_utf8`           | `MsgDecryptFailed` |
 * | `storage.json_failed` (decrypt OK, JSON bad)| `RecoveryFailed`   |
 * | other `storage.*` (DPAPI / IO / registry)   | `RecoveryFailed`   |
 * | non-storage (transport / platform)          | wrapCommand default |
 *
 * The first three collapse onto the same toast because the WPF
 * `try { decrypt; UTF-8; importRecord } catch { MsgDecryptFailed }`
 * block doesn't distinguish them — only the `if (importRecord ==
 * false)` branch escapes to `RecoveryFailed`. We map the JSON-failed
 * + IO-failed cases to that same `false` branch because for the
 * user they're indistinguishable from "import failed" (the byte
 * decryption succeeded but the file content / persistence is
 * unhealthy — same call to action: try a different backup).
 */

import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { ElButton, ElDialog, ElForm, ElFormItem, ElIcon, ElInput, ElMessage } from 'element-plus'
import { CircleClose, Download, Lock, Upload } from '@element-plus/icons-vue'

import { commands } from '../types/bindings'
import { safeInvoke, surfaceCommandError, wrapCommand } from '../services/invoke'
import { useAccountStore } from '../stores/account'

defineOptions({ name: 'AccRecovery' })

const props = defineProps<{
  /**
   * Two-way binding for whether the dialog is shown. Caller drives
   * open via `v-model:visible="..."`; the dialog drives close on
   * Esc / outside-click / explicit close button / Recovery success.
   */
  visible: boolean
}>()

const emit = defineEmits<{
  /** Two-way binding back to the caller's `visible` ref. */
  (event: 'update:visible', next: boolean): void
  /**
   * Emitted exactly once after a successful Recovery, before the
   * dialog closes. The parent (e.g. `pages/ManageAccount.vue`) uses
   * this to refresh its account list so the restored entries paint
   * immediately. Mirrors WPF's
   * `App.MainWnd.loginMethodInit()` post-restore call.
   */
  (event: 'restored'): void
}>()

const { t } = useI18n()
/*
 * Pin the store so a successful Recovery can update the in-memory
 * `accounts` list directly with the post-restore array the backend
 * already returned. Avoids a redundant `loadAccounts` round-trip on
 * the parent — `restored` event still fires for parents that prefer
 * to reload from scratch (defensive idempotent behaviour).
 */
const account = useAccountStore()

/* --------------- form state --------------- */

const password = ref('')
const ciphertext = ref('')
const exporting = ref(false)
const restoring = ref(false)

/* --------------- helpers --------------- */

const visible = computed({
  get: (): boolean => props.visible,
  set: (next: boolean): void => emit('update:visible', next),
})

const busy = computed((): boolean => exporting.value || restoring.value)

/**
 * Disable both action buttons when the password field is empty —
 * see the docblock's "Disabled buttons when password empty" note
 * for the rationale (don't let the user accidentally encrypt with
 * the MD5-of-empty-string key).
 */
const passwordValid = computed((): boolean => password.value.length > 0)

const canExport = computed((): boolean => passwordValid.value && !busy.value)

/**
 * Recovery additionally requires a non-empty ciphertext (the
 * `Data` field). WPF would still let the user click but immediately
 * throw on the `Convert.FromBase64String("")` call; we precheck so
 * the disabled state is visible feedback rather than a delayed toast.
 */
const canRestore = computed(
  (): boolean => passwordValid.value && ciphertext.value.length > 0 && !busy.value,
)

/**
 * Reset the form to a pristine state. Called on close so re-opening
 * the dialog never shows a stale password from the previous session.
 * Matches `windows/AddServiceAccount.vue`'s reset-on-`@closed`
 * pattern.
 */
function resetForm(): void {
  password.value = ''
  ciphertext.value = ''
  exporting.value = false
  restoring.value = false
}

/* --------------- export --------------- */

/**
 * Encrypt the current `Users.dat` snapshot under `password` and
 * fill the `Data` field with the resulting base64 ciphertext.
 *
 * On success the ciphertext stays in the textarea so the user can
 * copy it (no auto-clipboard — that's a separate UX choice; the
 * `[Copy]` affordance lives on the textarea via Element Plus's
 * built-in selection + Ctrl+A / Ctrl+C). WPF behaved identically:
 * the user manually selected the text and copied.
 *
 * Failures fall through to `wrapCommand`'s default toast — there's
 * no WPF error key for this path because WPF's `Export_Button_Click`
 * had no try/catch (any Users.dat read failure would crash the app).
 * We're more conservative than WPF here.
 */
async function handleExport(): Promise<void> {
  if (!canExport.value) return
  exporting.value = true
  try {
    const next = await wrapCommand(commands.backupExport(password.value))
    ciphertext.value = next
    ElMessage.success(t('ExportDone'))
  } catch {
    /* `wrapCommand` toasted the underlying cause. */
  } finally {
    exporting.value = false
  }
}

/* --------------- restore --------------- */

/**
 * Decrypt the user-supplied `Data` ciphertext under `password` and
 * overwrite `Users.dat`. On success → `RecoverySuccess` toast,
 * `restored` event, dialog auto-closes (WPF parity for
 * `loginMethodInit()` + return).
 *
 * Branch matrix (see module docblock for the full backend → WPF key
 * mapping):
 *
 * - `storage.aes_backup_invalid_ciphertext`,
 *   `storage.aes_backup_decrypt_failed`,
 *   `storage.aes_backup_invalid_utf8` → `MsgDecryptFailed`
 *   (collapsed because WPF's catch-all collapses them too).
 * - other `storage.*` (json_failed, dpapi_failed, io_failed, …)
 *   → `RecoveryFailed` (the "decrypt OK but persistence path failed"
 *   branch — WPF surfaces this when `importRecord` returns false).
 * - everything else → fall through to `surfaceCommandError`'s
 *   default toast (e.g. `platform.unsupported`, transport, panic
 *   passthrough).
 *
 * The branch dispatcher is `safeInvoke` + a `switch` because the
 * default `wrapCommand` translator only knows `errors.{code}` and we
 * want to route specific codes onto WPF copy without inventing a
 * one-off `errors.storage.aes_backup_decrypt_failed` translation
 * key (the WPF copy is `MsgDecryptFailed`, which lives in the WPF
 * resource scope, not in an `errors.*` namespace). Keeping the
 * mapping local to this component matches the same pattern
 * `stores/auth.ts` uses for `auth.totp_required` etc.
 */
async function handleRestore(): Promise<void> {
  if (!canRestore.value) return
  restoring.value = true
  try {
    const result = await safeInvoke(commands.backupRestore(password.value, ciphertext.value.trim()))
    if (result.ok) {
      account.accounts = result.data
      ElMessage.success(t('RecoverySuccess'))
      emit('restored')
      visible.value = false
      return
    }

    const code = result.error.code
    if (
      code === 'storage.aes_backup_invalid_ciphertext' ||
      code === 'storage.aes_backup_decrypt_failed' ||
      code === 'storage.aes_backup_invalid_utf8'
    ) {
      console.error(`[invoke] ${code}: ${result.error.message}`, result.error.details)
      ElMessage.error(t('MsgDecryptFailed'))
      return
    }
    if (code.startsWith('storage.')) {
      console.error(`[invoke] ${code}: ${result.error.message}`, result.error.details)
      ElMessage.error(t('RecoveryFailed'))
      return
    }
    /*
     * Non-storage failures (e.g. transport, platform_unsupported)
     * funnel through the default error pipeline so the operational
     * channels (console + session-expired hook) stay consistent
     * with every other command failure.
     */
    surfaceCommandError(result.error)
  } finally {
    restoring.value = false
  }
}

/* --------------- close lifecycle --------------- */

function handleCancel(): void {
  if (busy.value) return
  visible.value = false
}

/*
 * Reset form state when the dialog finishes closing (after the
 * fade-out animation), not when the user clicks the close button.
 * This matches the `AddServiceAccount.vue` `@closed` hook: preserves
 * input mid-animation if the user re-opens the dialog very quickly,
 * and lets Element Plus drive the timing.
 */
function handleClosed(): void {
  resetForm()
}

/*
 * Defensive reset on prop-driven open. If the parent flips
 * `visible` true → false → true very quickly (faster than the
 * fade-out animation), `@closed` may not have fired yet and the
 * form would carry stale state. Force a fresh reset on every
 * `false → true` transition to guarantee the open-state contract.
 */
watch(
  visible,
  (next, prev) => {
    if (next && !prev) {
      resetForm()
    }
  },
  { immediate: false },
)
</script>

<template>
  <el-dialog
    v-model="visible"
    :close-on-click-modal="!busy"
    :close-on-press-escape="!busy"
    :show-close="false"
    :before-close="(done: () => void) => (busy ? undefined : done())"
    :width="500"
    align-center
    append-to-body
    class="acc-recovery-dialog"
    data-test="acc-recovery-dialog"
    @closed="handleClosed"
  >
    <template #header>
      <div class="acc-recovery__header">
        <div class="acc-recovery__header-meta">
          <el-icon class="acc-recovery__header-icon" :size="20">
            <Lock />
          </el-icon>
          <span class="acc-recovery__header-title">{{ t('DataRecovery') }}</span>
        </div>
        <button
          type="button"
          class="acc-recovery__header-close"
          :title="t('Cancel')"
          :disabled="busy"
          data-test="acc-recovery-close"
          @click="handleCancel"
        >
          <el-icon><CircleClose /></el-icon>
        </button>
      </div>
    </template>

    <el-form class="acc-recovery__form" label-position="top" hide-required-asterisk>
      <el-form-item :label="t('Password')" class="acc-recovery__field">
        <el-input
          v-model="password"
          type="password"
          show-password
          autocomplete="off"
          :disabled="busy"
          data-test="acc-recovery-password"
        />
      </el-form-item>

      <el-form-item :label="t('Data')" class="acc-recovery__field">
        <el-input
          v-model="ciphertext"
          type="textarea"
          :autosize="{ minRows: 4, maxRows: 12 }"
          autocomplete="off"
          :disabled="busy"
          :placeholder="t('accRecovery.dataPlaceholder')"
          data-test="acc-recovery-data"
        />
      </el-form-item>
    </el-form>

    <template #footer>
      <div class="acc-recovery__footer">
        <button
          type="button"
          class="bf-btn-gradient acc-recovery__btn-primary"
          :disabled="!canExport"
          data-test="acc-recovery-export"
          @click="handleExport"
        >
          <el-icon><Download /></el-icon>
          <span>{{ t('Export') }}</span>
        </button>
        <el-button
          class="bf-btn-secondary acc-recovery__btn-secondary"
          :disabled="!canRestore"
          data-test="acc-recovery-restore"
          @click="handleRestore"
        >
          <el-icon><Upload /></el-icon>
          <span>{{ t('Recovery') }}</span>
        </el-button>
      </div>
    </template>
  </el-dialog>
</template>

<style scoped>
.acc-recovery__header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.5rem;
}

.acc-recovery__header-meta {
  display: inline-flex;
  align-items: center;
  gap: 0.5rem;
  min-width: 0;
}

.acc-recovery__header-icon {
  color: var(--bf-primary);
  flex-shrink: 0;
}

.acc-recovery__header-title {
  font-size: 0.9375rem;
  font-weight: 700;
  color: var(--bf-on-surface);
}

.acc-recovery__header-close {
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

.acc-recovery__header-close:hover:not(:disabled) {
  background: color-mix(in srgb, var(--bf-danger) 80%, transparent);
  color: var(--bf-on-danger);
}

.acc-recovery__header-close:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}

.acc-recovery__form {
  display: flex;
  flex-direction: column;
  gap: 0.25rem;
}

.acc-recovery__field :deep(.el-form-item__label) {
  font-size: 0.75rem;
  font-weight: 600;
  color: var(--bf-on-surface-variant);
  padding-bottom: 0.25rem;
}

.acc-recovery__field :deep(.el-textarea__inner) {
  font-family: ui-monospace, 'SF Mono', Consolas, monospace;
  font-size: 0.75rem;
  line-height: 1.5;
}

.acc-recovery__footer {
  display: flex;
  justify-content: flex-end;
  gap: 0.625rem;
}

.acc-recovery__btn-primary {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 0.375rem;
  min-width: 110px;
  padding: 0.5rem 1rem;
  font-size: 0.875rem;
  font-weight: 700;
}

.acc-recovery__btn-primary:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.acc-recovery__btn-secondary {
  display: inline-flex;
  align-items: center;
  gap: 0.375rem;
  min-width: 110px;
}
</style>
