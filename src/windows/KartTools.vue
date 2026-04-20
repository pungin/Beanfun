<script setup lang="ts">
/**
 * KartRider tools dialog (P12.5 D3).
 *
 * # WPF parity
 *
 * Mirrors `Beanfun/Windows/KartTools.xaml(.cs)` exactly:
 *
 * - `Window` titled `{DynamicResource ToolBox}` (shared with
 *   `MapleTools.xaml` — both tool dialogs reuse the same title
 *   resource, which is why we render `t('ToolBox')` here too
 *   instead of inventing a kart-specific title key).
 * - Single section header `ConvoyOperation` + horizontal
 *   splitter → six hyperlinks grouped into three column
 *   `StackPanel`s (DockPanel layout):
 *     - **Column 1** — `ConvoyManage` / `ConvoyRank`
 *     - **Column 2** — `ConvoySearch` / `RiderSearch`
 *     - **Column 3** — `CreateConvoy` / `LeaveConvoy`
 * - Each hyperlink opens the corresponding KartRider guild
 *   `.aspx` page on `tw.beanfun.com` via a new `WebBrowser`
 *   window. The SPA emits `open-web-browser` upward and lets
 *   the parent dispatch through `useInAppBrowser` (followup-B
 *   B7), which spawns a native `tauri::WebviewWindow` per click
 *   with the logged-in `BeanfunClient` cookies pre-seeded.
 *
 * # All six URLs land on the `tw.beanfun.com` allowlist host
 *
 * Every URL is on `tw.beanfun.com`, which sits inside the
 * backend `web_browser::ALLOWED_HOSTS` allowlist (followup-B
 * B2). The in-app browser window therefore renders the page
 * embedded with the user's session cookies — WPF parity for
 * `new WebBrowser(uri).Show()`. No system-browser fallback is
 * triggered for these six URLs.
 *
 * # WPF deviations (intentional)
 *
 * - **Modal vs new Window**: same rationale as the rest of
 *   `windows/*.vue` — the SPA renders the KartTools dialog
 *   in-page via `el-dialog`. The spawned in-app browser is a
 *   real OS-level `tauri::WebviewWindow` (followup-B B2) so it
 *   coexists alongside the modal exactly as WPF's two
 *   independent `Window` instances did.
 * - **No drag-to-move** (`MouseLeftButtonDown="DragMove"`):
 *   meaningless inside a modal; omitted, mirrors every other
 *   `windows/*.vue`.
 * - **`<Hyperlink>` → `<el-button>`**: WPF used WPF-specific
 *   `<Hyperlink>` runs inside `<TextBlock>` for the link
 *   styling. The SPA standardises on the same `bf-btn-secondary`
 *   button shape MapleTools uses, so both Tool dialogs feel
 *   visually consistent (DRY). The semantics — single-click
 *   navigates to the URL — are identical.
 * - **Mockup chrome dropped**: `mockups/KartTools.html`
 *   reimagines the dialog as a leaderboard / stats panel with
 *   tabs (我的紀錄 / 車輛配置 / 賽道排行 / 賽季統計) and a per-track
 *   ranking table — none of which exist in WPF or any backend
 *   payload. Per the user-approved P12.5 stance, mockup
 *   features that don't map to WPF behaviour are dropped.
 *
 * # URL hard-coding
 *
 * URLs ported verbatim from `KartTools.xaml.cs` so the Tauri
 * build hits the exact same Beanfun guild endpoints WPF does.
 * Note `maneger_data.aspx` is the original misspelling on
 * Beanfun's side (`manager_data` would 404) — preserved
 * literally; same applies to the case mismatch on
 * `kartrider/guild/rank.aspx` (lowercase `kartrider`) vs the
 * other five URLs (Pascal-cased `KartRider`).
 */

import { useI18n } from 'vue-i18n'
import { ElButton, ElDialog, ElIcon } from 'element-plus'
import { CircleClose, Link, Setting } from '@element-plus/icons-vue'

defineOptions({ name: 'KartToolsDialog' })

/**
 * Single source of truth for the (i18n key, URL) pairs the
 * dialog renders. Keeping it as a module-level readonly array
 * (instead of inlining six handlers in the template) means:
 *
 * - The WPF parity table lives in one place — when Beanfun
 *   moves a guild page tomorrow, exactly one URL changes.
 * - The template loops over it via `v-for`, so adding /
 *   removing a tool action is a one-line list edit (matches
 *   how `MapleTools.vue`'s five buttons would scale if WPF
 *   ever added a sixth).
 * - Tests can iterate the same array to assert each button
 *   renders the right label and emits the right URL without
 *   duplicating the data fixture.
 *
 * The `colIndex` field keeps the WPF column grouping (3 cols
 * × 2 rows) explicit so the template can still render in
 * column-major visual order even when the array is row-major.
 * Without it, swapping the `grid-auto-flow` direction would
 * silently re-order the actions with no visible compile-time
 * signal.
 */
interface KartToolAction {
  /** i18n key reused verbatim from the WPF locale tree. */
  readonly labelKey: string
  /** Exact URL ported from `KartTools.xaml.cs`. */
  readonly url: string
  /** `data-test` suffix and stable key for `v-for`. */
  readonly testId: string
}

const KART_TOOLS_ACTIONS: readonly KartToolAction[] = [
  {
    labelKey: 'ConvoyManage',
    url: 'https://tw.beanfun.com/KartRider/guild/maneger_data.aspx',
    testId: 'convoy-manage',
  },
  {
    labelKey: 'ConvoyRank',
    url: 'https://tw.beanfun.com/kartrider/guild/rank.aspx',
    testId: 'convoy-rank',
  },
  {
    labelKey: 'ConvoySearch',
    url: 'https://tw.beanfun.com/KartRider/guild/rank_team_in.aspx',
    testId: 'convoy-search',
  },
  {
    labelKey: 'RiderSearch',
    url: 'https://tw.beanfun.com/KartRider/guild/search_member.aspx',
    testId: 'rider-search',
  },
  {
    labelKey: 'CreateConvoy',
    url: 'https://tw.beanfun.com/KartRider/guild/create.aspx',
    testId: 'create-convoy',
  },
  {
    labelKey: 'LeaveConvoy',
    url: 'https://tw.beanfun.com/KartRider/guild/leave_guild_Member.aspx',
    testId: 'leave-convoy',
  },
] as const

defineProps<{
  /**
   * Two-way visibility binding (`v-model:visible`). Same
   * convention as every other `windows/*.vue`.
   */
  visible: boolean
}>()

const emit = defineEmits<{
  (event: 'update:visible', next: boolean): void
  /**
   * Ask the parent to open the in-app browser at `url`. All six
   * URLs are on `tw.beanfun.com` (inside the backend
   * `web_browser::ALLOWED_HOSTS` allowlist), so the parent's
   * `useInAppBrowser` dispatch spawns a native
   * `tauri::WebviewWindow` with the user's session cookies
   * pre-seeded — full WPF parity, no system-browser fallback.
   */
  (event: 'open-web-browser', url: string): void
}>()

const { t } = useI18n()

function handleClose(): void {
  emit('update:visible', false)
}

function handleAction(action: KartToolAction): void {
  emit('open-web-browser', action.url)
}

function handleVisibleChange(value: boolean): void {
  emit('update:visible', value)
}
</script>

<template>
  <el-dialog
    :model-value="visible"
    :width="520"
    :close-on-click-modal="true"
    :close-on-press-escape="true"
    :show-close="false"
    align-center
    append-to-body
    destroy-on-close
    class="kart-tools-dialog"
    data-test="kart-tools-dialog"
    @update:model-value="handleVisibleChange"
  >
    <template #header>
      <div class="kart-tools__header">
        <div class="kart-tools__header-meta">
          <el-icon class="kart-tools__header-icon" :size="20">
            <Setting />
          </el-icon>
          <div class="kart-tools__header-text">
            <span class="kart-tools__header-title" data-test="kart-tools-title">
              {{ t('ToolBox') }}
            </span>
            <span class="kart-tools__header-subtitle">
              {{ t('kartTools.subtitle') }}
            </span>
          </div>
        </div>
        <button
          type="button"
          class="kart-tools__header-close"
          :title="t('Cancel')"
          data-test="kart-tools-close"
          @click="handleClose"
        >
          <el-icon><CircleClose /></el-icon>
        </button>
      </div>
    </template>

    <div class="kart-tools__body">
      <!--
        WPF section header: `ConvoyOperation` label + thin
        horizontal splitter. We keep the same shape (label +
        rule) so the dialog reads as a single grouped panel
        rather than a bare button cluster.
      -->
      <div class="kart-tools__section-header">
        <span class="kart-tools__section-label" data-test="kart-tools-section-label">
          {{ t('ConvoyOperation') }}
        </span>
        <span class="kart-tools__section-rule" aria-hidden="true" />
      </div>

      <!--
        3-column × 2-row grid with column-major flow so the
        rendered order matches the WPF DockPanel:
          col 1: ConvoyManage / ConvoyRank
          col 2: ConvoySearch / RiderSearch
          col 3: CreateConvoy / LeaveConvoy
        See KART_TOOLS_ACTIONS docblock for why the array stays
        row-major in code.
      -->
      <div class="kart-tools__grid">
        <el-button
          v-for="action in KART_TOOLS_ACTIONS"
          :key="action.labelKey"
          class="kart-tools__button bf-btn-secondary"
          :data-test="`kart-tools-${action.testId}`"
          @click="handleAction(action)"
        >
          <el-icon><Link /></el-icon>
          <span>{{ t(action.labelKey) }}</span>
        </el-button>
      </div>
    </div>
  </el-dialog>
</template>

<style scoped>
.kart-tools__header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.75rem;
}

.kart-tools__header-meta {
  display: inline-flex;
  align-items: center;
  gap: 0.625rem;
  min-width: 0;
}

.kart-tools__header-icon {
  color: var(--bf-primary);
  flex-shrink: 0;
}

.kart-tools__header-text {
  display: flex;
  flex-direction: column;
  min-width: 0;
}

.kart-tools__header-title {
  font-size: 0.9375rem;
  font-weight: 700;
  color: var(--bf-on-surface);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.kart-tools__header-subtitle {
  font-size: 0.75rem;
  color: var(--bf-on-surface-variant);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.kart-tools__header-close {
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

.kart-tools__header-close:hover {
  background: color-mix(in srgb, var(--bf-danger) 80%, transparent);
  color: var(--bf-on-danger);
}

.kart-tools__body {
  display: flex;
  flex-direction: column;
  gap: 0.625rem;
  padding: 0.5rem 0.25rem;
}

.kart-tools__section-header {
  display: flex;
  align-items: center;
  gap: 0.5rem;
}

.kart-tools__section-label {
  font-size: 0.8125rem;
  font-weight: 600;
  color: var(--bf-on-surface-variant);
  flex-shrink: 0;
}

.kart-tools__section-rule {
  flex: 1;
  height: 1px;
  background: color-mix(in srgb, var(--bf-outline-variant) 50%, transparent);
}

/*
 * WPF DockPanel + 3 StackPanels = 3 columns, 2 rows, each
 * column flows top-to-bottom. `grid-auto-flow: column` makes
 * the row-major source array render in WPF's visual order
 * without re-ordering the keyed `v-for` items (which would
 * thrash the diff and break test selectors that key off the
 * same source order).
 */
.kart-tools__grid {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  grid-template-rows: repeat(2, auto);
  grid-auto-flow: column;
  gap: 0.5rem;
}

.kart-tools__button {
  width: 100%;
  justify-content: flex-start;
  gap: 0.5rem;
}
</style>
