<script setup lang="ts">
/**
 * Read-only contract viewer dialog (P12.2 D10.2).
 *
 * # WPF parity
 *
 * Mirrors `Beanfun/Windows/Contract.xaml(.cs)`:
 *
 * - Constructor `Contract(string ct)` takes the contract body up
 *   front; the SPA threads it in as `text` so the parent owns the
 *   data flow (same shape as `windows/CopyBox.vue`).
 * - Title binds to the `TermsOfService` resource (`{DynamicResource
 *   TermsOfService}`); we default the prop to that key for parity
 *   while still allowing per-call overrides if a future caller needs
 *   to surface a non-ToS contract (e.g. UnconnectedGame ToS variant).
 * - Body renders the `ct` payload inside a read-only `TextBox`
 *   (`IsReadOnly="True"`, `TextWrapping="Wrap"`, vertical scroll).
 *   We use `<pre class="...">{{ text }}</pre>` to preserve the
 *   exact whitespace / line breaks the Beanfun server sends back —
 *   `getContract()` returns plain text (after `wrapCommand` strips
 *   any RTF/HTML envelope), and a `<pre>` block is the closest DOM
 *   equivalent to WPF's wrapping read-only TextBox.
 * - Single close button mirrors WPF's "click anywhere off / press
 *   Esc to dismiss" UX (WPF `Window` had no explicit OK button —
 *   the user closed via the chrome — but a modal dialog needs a
 *   visible action so we add one labelled `Confirm` to match the
 *   established pattern in `AddServiceAccount.vue`'s previous
 *   inline preview).
 *
 * # WPF deviations (intentional)
 *
 * - **Modal vs new Window**: same rationale as the rest of
 *   `windows/*.vue` — SPA renders dialogs in-page via `el-dialog`
 *   instead of a top-level `Window`. No functional impact.
 * - **No drag-to-move**: `Window_MouseLeftButtonDown` →
 *   `DragMove()` is meaningless inside a modal layered over a
 *   backdrop; omitted.
 * - **No `v-html`**: the WPF TextBox renders plain text only —
 *   even when the server wraps the contract in RTF, the WPF widget
 *   shows the markup as literal text (TextBox is not RichTextBox).
 *   We deliberately mirror that here so a malicious / corrupted
 *   server payload can't inject HTML into the renderer. If a future
 *   product decision wants RTF rendering it has to land as a
 *   separate sanitization layer in this same component, not as a
 *   template-side `v-html`.
 *
 * # Mockup conflict resolution
 *
 * `mockups/Contract.html` paints this as an **acceptance gate**
 * (Agree checkbox + "Agree and continue" / "Disagree" buttons),
 * effectively duplicating the responsibility of
 * `AddServiceAccount.vue` (which already owns the
 * `IAgree` checkbox + `MsgTermsOfServiceNeed` validation). WPF
 * separates the two concerns: `Contract` is a pure viewer,
 * `AddServiceAccount` owns the agreement gate. We follow WPF
 * (P12.2 D10 Q9 = WPF parity, user-approved) so:
 *
 * - This component is render-only — no agreement state, no submit.
 * - Caller decides what "close" means (e.g. simply hide the dialog;
 *   or, if the caller is `AddServiceAccount`, the user still has to
 *   tick the form's existing `IAgree` checkbox afterwards).
 *
 * Mockup chrome (glass header, scrollable card body, single accent
 * close button) is preserved.
 *
 * # Lifecycle / state
 *
 * Stateless beyond the `visible` v-model — every render derives
 * directly from the `title` / `text` props. No reset on close (no
 * input to clear). Re-opening with new props swaps the body on
 * the next tick (Vue reactivity).
 */

import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { ElButton, ElDialog, ElIcon } from 'element-plus'
import { CircleClose, Document } from '@element-plus/icons-vue'

/*
 * Registered name is `ContractDialog` (not `Contract`) to satisfy
 * `vue/multi-word-component-names` — single-word component names
 * collide with HTML elements per Vue style guide. The file name
 * stays `Contract.vue` to mirror WPF `Beanfun/Windows/Contract.xaml`
 * and to keep the import shape (`import Contract from
 * '../windows/Contract.vue'`) parity-aligned with the rest of the
 * `windows/*.vue` import sites.
 */
defineOptions({ name: 'ContractDialog' })

const props = withDefaults(
  defineProps<{
    /**
     * Two-way binding for whether the dialog is shown. Caller drives
     * open via `v-model:visible="..."`; the dialog drives close on
     * Esc / outside-click / explicit close button.
     */
    visible: boolean
    /**
     * Contract body to render, plain text. Whitespace and line
     * breaks are preserved verbatim (rendered inside `<pre>`).
     */
    text: string
    /**
     * Dialog title, defaults to the WPF `TermsOfService` resource
     * key. Overridable so a future caller can surface a non-ToS
     * contract (e.g. unconnected-game ToS variant) without forking
     * this component.
     */
    title?: string
  }>(),
  {
    title: 'TermsOfService',
  },
)

const emit = defineEmits<{
  (event: 'update:visible', next: boolean): void
}>()

const { t } = useI18n()

const visible = computed({
  get: (): boolean => props.visible,
  set: (next: boolean): void => emit('update:visible', next),
})

/*
 * Resolve the title prop through i18n when it matches a known
 * resource key, otherwise treat it as a literal label. This mirrors
 * WPF's `{DynamicResource TermsOfService}` binding for the default
 * while still letting callers pass an already-translated string
 * (e.g. a server-supplied contract name) without double-lookup.
 */
const resolvedTitle = computed((): string => {
  const candidate = props.title
  const localized = t(candidate)
  return localized === candidate ? candidate : localized
})

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
    :width="540"
    align-center
    append-to-body
    class="contract-dialog"
    data-test="contract-dialog"
  >
    <template #header>
      <div class="contract__header">
        <div class="contract__header-meta">
          <el-icon class="contract__header-icon" :size="20">
            <Document />
          </el-icon>
          <span class="contract__header-title" data-test="contract-title">{{ resolvedTitle }}</span>
        </div>
        <button
          type="button"
          class="contract__header-close"
          :title="t('Cancel')"
          data-test="contract-close"
          @click="handleClose"
        >
          <el-icon><CircleClose /></el-icon>
        </button>
      </div>
    </template>

    <pre class="contract__body bf-custom-scrollbar" data-test="contract-text">{{ text }}</pre>

    <template #footer>
      <div class="contract__footer">
        <el-button
          class="bf-btn-secondary contract__btn-confirm"
          data-test="contract-confirm"
          @click="handleClose"
        >
          {{ t('Confirm') }}
        </el-button>
      </div>
    </template>
  </el-dialog>
</template>

<style scoped>
.contract__header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.5rem;
}

.contract__header-meta {
  display: inline-flex;
  align-items: center;
  gap: 0.5rem;
  min-width: 0;
}

.contract__header-icon {
  color: var(--bf-primary);
  flex-shrink: 0;
}

.contract__header-title {
  font-size: 0.9375rem;
  font-weight: 700;
  color: var(--bf-on-surface);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.contract__header-close {
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

.contract__header-close:hover {
  background: color-mix(in srgb, var(--bf-danger) 80%, transparent);
  color: var(--bf-on-danger);
}

.contract__body {
  margin: 0;
  padding: 0.875rem 1rem;
  max-height: 360px;
  overflow-y: auto;
  background: var(--bf-surface-container-low);
  border: 1px solid var(--bf-outline-variant);
  border-radius: var(--bf-radius-input);
  font-family:
    'Plus Jakarta Sans',
    system-ui,
    -apple-system,
    Segoe UI,
    sans-serif;
  font-size: 0.8125rem;
  line-height: 1.55;
  color: var(--bf-on-surface);
  white-space: pre-wrap;
  word-break: break-word;
}

.contract__footer {
  display: flex;
  justify-content: flex-end;
  gap: 0.625rem;
}

.contract__btn-confirm {
  min-width: 88px;
}
</style>
