<script setup lang="ts">
/**
 * Generic `(title, value) + Copy` dialog (P12.2 D10.1).
 *
 * # WPF parity
 *
 * Mirrors `Beanfun/Windows/CopyBox.xaml(.cs)`:
 *
 * - Constructor `CopyBox(string title, string value)` takes both
 *   strings up front; the SPA threads them in as props so the
 *   parent owns the data flow.
 * - Body is a read-only `TextBox` showing `value` next to a
 *   `[Copy]` button.
 * - `Button_Click` runs `Clipboard.SetText(t_Value.Text)` then
 *   shows `MessageBox(TryFindResource("CopyFinished"))`. On any
 *   exception (including the legendary "clipboard busy" race
 *   from another `WM_DRAWCLIPBOARD` listener) it shows
 *   `CopyFailed`.
 * - We mirror the success / failure branches as Element Plus
 *   toasts using the same `CopyFinished` / `CopyFailed` keys.
 *
 * # WPF deviations (intentional)
 *
 * - **Modal vs new Window**: same rationale as the rest of
 *   `windows/*.vue` (D3, D4, D6, D7) — SPA renders dialogs in-page
 *   via `el-dialog` instead of a top-level `Window`.
 * - **No drag-to-move**: `Window_MouseLeftButtonDown` →
 *   `DragMove()` is meaningless inside a modal layered over a
 *   backdrop; omitted.
 * - **`navigator.clipboard.writeText` instead of
 *   `Clipboard.SetText`**: the WebView2 / Tauri renderer has the
 *   web Clipboard API; we await the promise and surface the
 *   rejection through the same `CopyFailed` toast WPF used.
 *   Asynchronous failures (permissions denied, focus lost) are
 *   modelled as Promise rejections rather than throws — same
 *   try/catch shape with `await` semantics.
 *
 * # OTP-specific mockup deferred
 *
 * `mockups/CopyBox.html` shows an OTP-themed variant with a
 * 40-px monospace digit display, a hide-toggle, and a progress
 * ring. That UX is **already provided** by D5's inline OTP card on
 * the AccountList row (`AccountList.vue` shows the OTP + countdown
 * in the active row's expanded panel) — surfacing it again as a
 * dialog would duplicate state and split the lifecycle. So
 * `windows/CopyBox.vue` keeps the WPF generic shape, and the OTP
 * card stays inline. P12.2 D10 Q7 = WPF parity (already user-
 * approved).
 *
 * # Lifecycle / state
 *
 * Stateless beyond the `visible` v-model — every render derives
 * directly from the `title` / `value` props. No reset on close
 * (no input to clear). Re-opening with the same props produces
 * the same render; re-opening with new props swaps the visible
 * value on the next tick (Vue reactivity).
 *
 * # Error mapping (WPF parity)
 *
 * | Path                                      | Toast key (WPF resource) |
 * |-------------------------------------------|--------------------------|
 * | `clipboard.writeText` resolves            | `CopyFinished` (success) |
 * | `clipboard.writeText` rejects / throws    | `CopyFailed`   (error)   |
 *
 * Both keys live in `src/locales/zh-TW.json` (and siblings) from
 * the WPF locale port — no new i18n keys.
 */

import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { ElDialog, ElIcon, ElInput, ElMessage } from 'element-plus'
import { CircleClose, CopyDocument } from '@element-plus/icons-vue'

defineOptions({ name: 'CopyBox' })

const props = defineProps<{
  /**
   * Two-way binding for whether the dialog is shown. Caller drives
   * open via `v-model:visible="..."`; the dialog drives close on
   * Esc / outside-click / explicit close button.
   */
  visible: boolean
  /**
   * Dialog title shown in the header — caller-provided so a single
   * `CopyBox` SFC serves every "show + copy" use case (`AuthEmail`,
   * `gameAccount`, etc.) without per-call subclassing. Mirrors
   * WPF's `CopyBox(string title, string value)` constructor: title
   * comes from the caller's resource lookup, not hard-coded inside.
   */
  title: string
  /**
   * The string to display + copy. Bound `:model-value` (read-only)
   * to the `el-input` body so the user sees exactly what
   * `clipboard.writeText` will receive.
   */
  value: string
}>()

const emit = defineEmits<{
  (event: 'update:visible', next: boolean): void
}>()

const { t } = useI18n()

const visible = computed({
  get: (): boolean => props.visible,
  set: (next: boolean): void => emit('update:visible', next),
})

/**
 * Copy `props.value` to the clipboard.
 *
 * `navigator.clipboard.writeText` returns a Promise that rejects
 * when the clipboard write is denied (permissions, document not
 * focused, or the renderer process lacks clipboard scope). We
 * `await` + try/catch the rejection and surface either branch as
 * the WPF-equivalent toast — semantically identical to WPF's
 * synchronous `Clipboard.SetText(...)` wrapped in try/catch.
 *
 * Runtime guard: `navigator.clipboard` is `undefined` in the
 * legacy Edge / non-secure-context fallback paths Tauri WebView2
 * does not actually use — but keep the guard so unit tests that
 * deliberately strip the global don't crash with a TypeError; they
 * exercise the failure branch instead.
 */
async function handleCopy(): Promise<void> {
  try {
    if (!navigator.clipboard?.writeText) {
      throw new Error('clipboard API unavailable')
    }
    await navigator.clipboard.writeText(props.value)
    ElMessage.success(t('CopyFinished'))
  } catch {
    ElMessage.error(t('CopyFailed'))
  }
}

function handleClose(): void {
  visible.value = false
}
</script>

<template>
  <el-dialog
    v-model="visible"
    :close-on-click-modal="true"
    :close-on-press-escape="true"
    :show-close="false"
    :width="380"
    align-center
    append-to-body
    class="copy-box-dialog"
    data-test="copy-box-dialog"
  >
    <template #header>
      <div class="copy-box__header">
        <div class="copy-box__header-meta">
          <el-icon class="copy-box__header-icon" :size="18">
            <CopyDocument />
          </el-icon>
          <span class="copy-box__header-title" data-test="copy-box-title">{{ title }}</span>
        </div>
        <button
          type="button"
          class="copy-box__header-close"
          :title="t('Cancel')"
          data-test="copy-box-close"
          @click="handleClose"
        >
          <el-icon><CircleClose /></el-icon>
        </button>
      </div>
    </template>

    <div class="copy-box__body">
      <el-input
        :model-value="value"
        readonly
        autocomplete="off"
        data-test="copy-box-value"
        class="copy-box__value"
      />
      <button
        type="button"
        class="bf-btn-gradient copy-box__copy-btn"
        data-test="copy-box-copy"
        @click="handleCopy"
      >
        <el-icon><CopyDocument /></el-icon>
        <span>{{ t('Copy') }}</span>
      </button>
    </div>
  </el-dialog>
</template>

<style scoped>
.copy-box__header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.5rem;
}

.copy-box__header-meta {
  display: inline-flex;
  align-items: center;
  gap: 0.5rem;
  min-width: 0;
}

.copy-box__header-icon {
  color: var(--bf-primary);
  flex-shrink: 0;
}

.copy-box__header-title {
  font-size: 0.9375rem;
  font-weight: 700;
  color: var(--bf-on-surface);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.copy-box__header-close {
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

.copy-box__header-close:hover {
  background: color-mix(in srgb, var(--bf-danger) 80%, transparent);
  color: var(--bf-on-danger);
}

.copy-box__body {
  display: flex;
  align-items: center;
  gap: 0.5rem;
}

.copy-box__value {
  flex: 1 1 auto;
  min-width: 0;
}

.copy-box__copy-btn {
  display: inline-flex;
  align-items: center;
  gap: 0.375rem;
  padding: 0.5rem 0.875rem;
  font-size: 0.875rem;
  font-weight: 600;
  flex-shrink: 0;
}
</style>
