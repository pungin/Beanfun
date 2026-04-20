<script setup lang="ts">
/**
 * MapleStory tools dialog (P12.5 D2).
 *
 * # WPF parity
 *
 * Mirrors `Beanfun/Windows/MapleTools.xaml(.cs)` exactly:
 *
 * - Vertical stack of `Button`s in a `StackPanel Margin="20"`,
 *   each with `Margin="5"`. Original WPF order:
 *   Recycling → PlayerReport → VideoReport →
 *   EquipStarForceCalculator → PerfectCoreCalculator. SPA order
 *   matches but with VideoReport removed (see "WPF deviation:
 *   VideoReport removed" below).
 * - Window title `{DynamicResource ToolBox}` ("工具箱" / "Toolbox").
 * - **Recycling** (`btn_Recycling_Click` L51-112): YesNo confirm
 *   `MsgRecycling` → backend filesystem sweep → `MsgRecyclingDone`
 *   alert. P12.5 D1's `commands.cleanMapleGameCache` ports the
 *   filesystem walk verbatim; this component owns only the IPC
 *   round-trip + user-facing toast plumbing.
 * - **PlayerReport** (`btn_PlayerReport_Click` L24-32): if the
 *   active session is HK, alert `MsgPlayerReport` first ("this
 *   feature requires a TW account"), **then** open the URL
 *   regardless. WPF's behaviour is to inform-and-still-open so
 *   the user can read the page even if reporting itself won't
 *   succeed; we replicate that exactly.
 * - **EquipCalc / CoreCalc** (`btn_EquipCalculator_Click` L41-44 /
 *   `btn_CoreCaculator_Click` L46-49): open the matching child
 *   dialog. WPF spawns a new `Window`; we emit upward and let the
 *   parent host both calculators as siblings of this dialog (so
 *   the user can jump between Tools and a calculator without
 *   nested-modal z-index gymnastics).
 *
 * # WPF deviations (intentional)
 *
 * - **VideoReport removed (P12.4-followup-B-fix F2, Q12)**: WPF
 *   `btn_VideoReport_Click` L34-39 navigates to
 *   `event.beanfun.com/MapleStory/eventad/EventAD.aspx?EventADID=3453`,
 *   which redirects to a `tw.hicdn.beanfun.com/.../404.html` page
 *   — the upstream EventAD record was retired, the button has
 *   been dead-link for an indeterminate amount of time. WPF still
 *   ships the button (it just opens 404). User instruction during
 *   the followup-B smoke test was to drop the button rather than
 *   leave a confusing affordance; this is the only intentional
 *   deviation from strict WPF parity in MapleTools. If beanfun
 *   ever restores the EventAD page, revert this commit's
 *   MapleTools changes (the button itself is mechanical to add
 *   back; the i18n key `VideoReport` stays in the WPF locale
 *   tree even now).
 * - **Modal vs new Window**: same rationale as the rest of
 *   `windows/*.vue` — the SPA renders dialogs in-page via
 *   `el-dialog`. The MapleTools dialog stays open while a child
 *   calculator / web browser dialog is shown alongside it (the
 *   parent decides the layering); WPF keeps the same multi-window
 *   coexistence by virtue of being a desktop app, so behaviour is
 *   equivalent.
 * - **Recycling errors surfaced**: WPF wraps each delete in
 *   `try { ... } catch { }` and unconditionally shows the success
 *   toast — silently swallowing partial failures. The backend now
 *   returns a [`CleanCacheReport`] with per-item errors; we
 *   surface those by appending a localized "completed with N
 *   errors" line to the success alert when the report is
 *   non-empty (P12.5 D1 design decision — improves observability
 *   without changing the WPF user flow).
 * - **No drag-to-move** (`MouseLeftButtonDown="DragMove"`):
 *   meaningless inside a modal; omitted, mirrors every other
 *   `windows/*.vue`.
 *
 * # Why the parent owns child-dialog hosting
 *
 * Two downstream dialogs (`EquipCalculator`, `CoreCalculator`)
 * need to coexist with this one. Hosting them here would require
 * nested `el-dialog`s, which Element Plus supports but trips on
 * `append-to-body` / focus-trap stacking in subtle ways. Emitting
 * upward keeps every dialog at the same DOM depth and matches
 * the existing pattern (`AccountList.vue` already hosts
 * `GameList`, `AddAccount` etc. as sibling children — adding
 * two more is mechanical). The third historical delegate,
 * `open-web-browser`, is no longer a sibling dialog: the parent
 * forwards it to `useInAppBrowser` (followup-B B7) which spawns
 * a native `WebviewWindow` per click instead.
 *
 * # Caller wiring
 *
 * ```vue
 * <MapleTools
 *   v-model:visible="mapleToolsOpen"
 *   :game-path="gamePath"
 *   :login-region="auth.session?.region"
 *   @open-web-browser="(url) => openWebBrowser(url)"
 *   @open-equip-calculator="equipCalcOpen = true"
 *   @open-core-calculator="coreCalcOpen = true"
 * />
 * ```
 */

import { useI18n } from 'vue-i18n'
import { ElButton, ElDialog, ElIcon, ElMessage, ElMessageBox } from 'element-plus'
import { CircleClose, Delete, Document, Pointer, Setting } from '@element-plus/icons-vue'

import { commands, type LoginRegion } from '../types/bindings'
import { safeInvoke } from '../services/invoke'

defineOptions({ name: 'MapleToolsDialog' })

/**
 * PlayerReport URL ported verbatim from `MapleTools.xaml.cs` L31
 * so the Tauri build hits the exact same Beanfun event-portal
 * endpoint WPF does. Kept as a module-level const (not prop /
 * config) because it is part of the WPF surface — every Beanfun
 * build (TW / HK) historically navigates to this same URL.
 *
 * `VIDEO_REPORT_URL` was removed in P12.4-followup-B-fix F2 (see
 * the file docblock "WPF deviations" section for the audit
 * trail).
 */
const PLAYER_REPORT_URL =
  'https://event.beanfun.com/customerservice/PluginReporting/PlayerReport.aspx'

const props = withDefaults(
  defineProps<{
    /**
     * Two-way visibility binding (`v-model:visible`). Same
     * convention as every other `windows/*.vue`.
     */
    visible: boolean
    /**
     * Path to the game `.exe` (the value that lives in
     * `Config.xml::Path.<gameCode>`). The Recycling action
     * forwards it verbatim to `commands.cleanMapleGameCache`,
     * which derives the parent directory and runs the sweep.
     *
     * Empty string is permitted at mount time so the parent can
     * pre-mount the dialog and lazy-load the path. The Recycling
     * handler guards on empty / missing-file with the WPF-equivalent
     * `MsgCantFindGame` toast (`MainWindow.runGame` L1727-1738
     * uses the same key for the same precondition).
     */
    gamePath?: string
    /**
     * Active session region — `'HK'` triggers the WPF-mirrored
     * `MsgPlayerReport` advisory before opening PlayerReport
     * (the report flow itself is TW-only). `undefined` skips the
     * advisory (defensive — production callers always have a
     * session by the time this dialog is reachable).
     */
    loginRegion?: LoginRegion
  }>(),
  {
    gamePath: '',
    loginRegion: undefined,
  },
)

const emit = defineEmits<{
  (event: 'update:visible', next: boolean): void
  /**
   * Ask the parent to open the in-app browser at `url`. Used for
   * PlayerReport — the URL lands on `event.beanfun.com`, which
   * sits inside the backend `web_browser::is_allowed_host` suffix
   * policy (`*.beanfun.com`) since P12.4-followup-B-fix F1, so
   * `useInAppBrowser` opens the URL in a fresh `WebviewWindow`
   * with the BeanfunClient session cookies pre-seeded — full WPF
   * `new WebBrowser(uri).Show()` parity. The system-browser
   * fallback inside `useInAppBrowser` only fires for URLs outside
   * `*.beanfun.com`, which is no longer reachable from this
   * component after the F2 VideoReport removal.
   */
  (event: 'open-web-browser', url: string): void
  /** Ask the parent to open the EquipCalculator dialog (P12.5 D5). */
  (event: 'open-equip-calculator'): void
  /** Ask the parent to open the CoreCalculator dialog (P12.5 D4). */
  (event: 'open-core-calculator'): void
}>()

const { t } = useI18n()

function handleClose(): void {
  emit('update:visible', false)
}

/**
 * Pre-flight guard mirroring `MainWindow.runGame` L1727-1738:
 * if the configured game path is empty or doesn't resolve to an
 * actual file, the WPF flow surfaces `MsgCantFindGame` and
 * aborts. We don't have a frontend `File.Exists` (Tauri's
 * `@tauri-apps/plugin-fs` is gated behind permissions we don't
 * grant the SPA), so we lean on the backend service to tell us
 * the path is bad — but we still short-circuit on empty here so
 * the user gets feedback without an unnecessary IPC round-trip.
 */
async function handleRecycling(): Promise<void> {
  if (props.gamePath === '') {
    ElMessage.warning(t('MsgCantFindGame'))
    return
  }

  // WPF: MessageBox.Show(MsgRecycling, "", YesNo). ElMessageBox.confirm
  // returns a rejected promise on Cancel/close — match the WPF
  // "if (result != Yes) return" by catching and short-circuiting.
  try {
    await ElMessageBox.confirm(t('MsgRecycling'), '', {
      type: 'warning',
      confirmButtonText: t('Yes'),
      cancelButtonText: t('No'),
    })
  } catch {
    return
  }

  const result = await safeInvoke(commands.cleanMapleGameCache(props.gamePath))
  if (!result.ok) {
    /*
     * Backend pre-flight failures (missing dir, file-as-path,
     * read_dir failure) come back as typed `maple_cache.*` codes.
     * Surface the (translated where available) message — the
     * `errors.maple_cache.*` namespace in `messages.ts` D8 will
     * map them to friendly copy; until then `result.error.message`
     * is the typed Rust-side description (safe to show).
     */
    ElMessage.error(result.error.message)
    return
  }

  const report = result.data
  if (report.errors.length === 0) {
    void ElMessageBox.alert(t('MsgRecyclingDone'), '', { type: 'success' })
    return
  }

  /*
   * Partial-success path. WPF would have shown the success
   * toast unconditionally (silent failure); we append the
   * per-item error list so the user knows a locked DLL or
   * permission-denied entry survived the sweep.
   *
   * `dangerouslyUseHTMLString: true` is *off* by design — the
   * names come from the user's filesystem and could in theory
   * contain HTML-meaningful chars; a plain-text join with
   * `white-space: pre-line` handles every safe escape via the
   * `customStyle` injection in `customClass`.
   */
  const errorList = report.errors.join('\n')
  void ElMessageBox.alert(
    `${t('MsgRecyclingDone')}\n\n${t('mapleTools.recyclingErrors')}\n${errorList}`,
    '',
    {
      type: 'warning',
      customClass: 'maple-tools-recycling-alert',
    },
  )
}

async function handlePlayerReport(): Promise<void> {
  if (props.loginRegion === 'HK') {
    /*
     * WPF L26-27: alert + still navigates regardless. We mirror
     * the "alert blocks until OK, then proceed" sequence so the
     * user has read the advisory before the browser dialog
     * opens on top.
     */
    try {
      await ElMessageBox.alert(t('MsgPlayerReport'), '', { type: 'info' })
    } catch {
      // ElMessageBox.alert resolves on OK and rejects on outside-click
      // close — both branches should still navigate, mirroring WPF
      // (the alert's only purpose is to inform).
    }
  }
  emit('open-web-browser', PLAYER_REPORT_URL)
}

function handleEquipCalculator(): void {
  emit('open-equip-calculator')
}

function handleCoreCalculator(): void {
  emit('open-core-calculator')
}

function handleVisibleChange(value: boolean): void {
  emit('update:visible', value)
}
</script>

<template>
  <el-dialog
    :model-value="visible"
    :width="420"
    :close-on-click-modal="true"
    :close-on-press-escape="true"
    :show-close="false"
    align-center
    append-to-body
    destroy-on-close
    class="maple-tools-dialog"
    data-test="maple-tools-dialog"
    @update:model-value="handleVisibleChange"
  >
    <template #header>
      <div class="maple-tools__header">
        <div class="maple-tools__header-meta">
          <el-icon class="maple-tools__header-icon" :size="20">
            <Setting />
          </el-icon>
          <div class="maple-tools__header-text">
            <span class="maple-tools__header-title" data-test="maple-tools-title">
              {{ t('ToolBox') }}
            </span>
            <span class="maple-tools__header-subtitle">
              {{ t('mapleTools.subtitle') }}
            </span>
          </div>
        </div>
        <button
          type="button"
          class="maple-tools__header-close"
          :title="t('Cancel')"
          data-test="maple-tools-close"
          @click="handleClose"
        >
          <el-icon><CircleClose /></el-icon>
        </button>
      </div>
    </template>

    <div class="maple-tools__body">
      <el-button
        class="maple-tools__button bf-btn-secondary"
        data-test="maple-tools-recycling"
        @click="handleRecycling"
      >
        <el-icon><Delete /></el-icon>
        <span>{{ t('Recycling') }}</span>
      </el-button>
      <el-button
        class="maple-tools__button bf-btn-secondary"
        data-test="maple-tools-player-report"
        @click="handlePlayerReport"
      >
        <el-icon><Document /></el-icon>
        <span>{{ t('PlayerReport') }}</span>
      </el-button>
      <el-button
        class="maple-tools__button bf-btn-secondary"
        data-test="maple-tools-equip-calculator"
        @click="handleEquipCalculator"
      >
        <el-icon><Pointer /></el-icon>
        <span>{{ t('EquipStarForceCaculator') }}</span>
      </el-button>
      <el-button
        class="maple-tools__button bf-btn-secondary"
        data-test="maple-tools-core-calculator"
        @click="handleCoreCalculator"
      >
        <el-icon><Pointer /></el-icon>
        <span>{{ t('PerfectCoreCaculator') }}</span>
      </el-button>
    </div>
  </el-dialog>
</template>

<style scoped>
.maple-tools__header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.75rem;
}

.maple-tools__header-meta {
  display: inline-flex;
  align-items: center;
  gap: 0.625rem;
  min-width: 0;
}

.maple-tools__header-icon {
  color: var(--bf-primary);
  flex-shrink: 0;
}

.maple-tools__header-text {
  display: flex;
  flex-direction: column;
  min-width: 0;
}

.maple-tools__header-title {
  font-size: 0.9375rem;
  font-weight: 700;
  color: var(--bf-on-surface);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.maple-tools__header-subtitle {
  font-size: 0.75rem;
  color: var(--bf-on-surface-variant);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.maple-tools__header-close {
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

.maple-tools__header-close:hover {
  background: color-mix(in srgb, var(--bf-danger) 80%, transparent);
  color: var(--bf-on-danger);
}

.maple-tools__body {
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
  padding: 0.5rem 0.25rem;
}

/*
 * Each tool button stretches the dialog width and keeps the
 * icon + label inline. Element Plus's default `el-button` would
 * shrink to content; the override mirrors WPF's
 * `<Button Margin="5"/>` inside a `StackPanel` (each child
 * fills the panel width).
 */
.maple-tools__button {
  width: 100%;
  justify-content: flex-start;
  gap: 0.5rem;
}
</style>

<style>
/*
 * Unscoped — `ElMessageBox.alert` mounts outside this component's
 * scoped style root so a `:scoped` rule wouldn't apply. Targets
 * the `customClass` value passed in `handleRecycling` to render
 * the per-item error list with preserved newlines.
 */
.maple-tools-recycling-alert .el-message-box__message {
  white-space: pre-line;
  word-break: break-word;
}
</style>
