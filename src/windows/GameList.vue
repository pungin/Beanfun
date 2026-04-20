<script setup lang="ts">
/**
 * Game-picker dialog (P12.3 D5).
 *
 * # WPF parity
 *
 * Mirrors `Beanfun/Windows/GameList.xaml(.cs)`:
 *
 * - Modal picker that lists every `GameService` for the current
 *   region (filtered backend-side by the session region pinned on
 *   the [`BeanfunClient`], so the dialog mirrors WPF's
 *   `App.MainWnd.GameList[App.LoginRegion.ToLower()]` slice without
 *   the frontend having to re-filter).
 * - Each list item renders the large image (`large_image_name`) +
 *   centred name (`ServiceFamilyName`). WPF used a fixed
 *   `Image Width="152" Height="102"` inside a 1px `Border` and a
 *   `WrapPanel Width="680"` (so 4 cards per row at 170px each
 *   incl. margin); we use the same target sizing in the SPA grid
 *   so the dialog feels identical at a glance.
 * - Single-click selects a game. WPF `l_GameList_SelectionChanged`
 *   compares the click target against `App.MainWnd.service_code +
 *   service_region`; only emits / refreshes the AccountList when
 *   the selection actually changes; **always** closes the dialog
 *   regardless. We mirror both branches:
 *     - If the click matches `game.selectedGameCode`, no
 *       `selectGame` write, no `select` event — just close.
 *     - Otherwise: `game.selectGame()` writes the joined code,
 *       `select` event fires for the parent (`AccountList.vue` D8)
 *       to react (game info bar swap, account list reload), then
 *       the dialog closes.
 *
 * # WPF deviations (intentional)
 *
 * - **Modal vs new Window**: same rationale as the rest of
 *   `windows/*.vue` — SPA renders dialogs in-page via `el-dialog`.
 * - **No drag-to-move**: meaningless inside a modal layered over
 *   a backdrop; omitted (mirrors the rest of `windows/*.vue`).
 * - **4-state load machine**: WPF assumed the catalogue was
 *   already populated by an earlier `reLoadGameInfo()` call when
 *   the dialog opened; if the prior fetch had failed the user got
 *   an empty `WrapPanel` with no recovery affordance. We render
 *   `loading` / `error` / `empty` / `loaded` explicitly so the
 *   user can see *why* the grid is empty and retry without
 *   navigating back. Triggered on `false → true` transition via
 *   `game.loadGames()` (idempotent — the store short-circuits when
 *   already `loaded`, mirroring WPF's "fetch once per session"
 *   contract).
 * - **`<img alt>` text**: WPF `Image` controls have no
 *   accessibility label; we add `t('gameList.imageAlt', { name })`
 *   so screen readers announce each card. Pure progressive
 *   enhancement — visual users see the same grid as WPF.
 *
 * # Mockup conflict resolution (P12.3 plan, user-approved)
 *
 * `mockups/GameList.html` introduces three new affordances we
 * intentionally drop:
 *
 * 1. **Search bar** — would change the filtered set without
 *    backend support, leaving the dialog as the only place a
 *    search exists; not worth one-off scope.
 * 2. **Category tabs** (RPG / 競速 / 不連線 / …) — the WPF
 *    `GameService` payload has no category metadata, so the tabs
 *    would have to hard-code per-game classifications that drift
 *    from upstream. WPF's "Unconnected" branch keys off
 *    `service_code + region` literals (see
 *    {@link UNCONNECTED_GAME_CODES}) which already lives where it
 *    belongs (the AccountList router) — replicating it as a UI
 *    tab here would duplicate the source of truth.
 * 3. **`game-card:hover` lift / `selected` ring + `chip-hot`
 *    badges** — pure mockup chrome with no WPF / backend signal
 *    to drive them. The cards still render with a subtle
 *    selection ring (matches `selectedGame`), but no transform
 *    animation (WPF parity = static hover state).
 *
 * The mockup chrome we *do* preserve: glass dialog header with
 * icon + title + close button (matches every other `windows/*.vue`).
 *
 * # State / lifecycle
 *
 * Stateless beyond the v-model proxy. All catalogue / selection
 * data lives in the [`useGameStore`][store] so AccountList
 * (P12.3 D8) can read the same `selectedGame` / `selectedIni`
 * values without prop-drilling. The store's `loadState` drives
 * the 4-state branch directly; the dialog never owns its own
 * loading / error refs.
 *
 * Caller wiring (`AccountList.vue` D8):
 *
 * ```vue
 * <GameList
 *   v-model:visible="gameListOpen"
 *   :region="auth.session!.region"
 *   @select="(code, region) => onGameChanged(code, region)"
 * />
 * ```
 *
 * The `region` prop is required because [`useGameStore`] is
 * deliberately auth-store-unaware (see store docblock for the
 * "no circular import" rationale) — the parent already has
 * `auth.session.region` in scope, so passing it through is free.
 *
 * [store]: ../stores/game.ts
 */

import { computed, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { ElButton, ElDialog, ElIcon } from 'element-plus'
import { CircleClose, Refresh, VideoPlay, Warning } from '@element-plus/icons-vue'

import { useGameStore, imageUrl, gameCodeOf } from '../stores/game'
import type { GameService, LoginRegion } from '../types/bindings'

/*
 * Registered name is `GameListDialog` (not `GameList`) to satisfy
 * `vue/multi-word-component-names`. The file name stays
 * `GameList.vue` to mirror WPF `Beanfun/Windows/GameList.xaml`
 * and keep the import shape (`import GameList from
 * '../windows/GameList.vue'`) parity-aligned with the rest of
 * `windows/*.vue`.
 */
defineOptions({ name: 'GameListDialog' })

const props = defineProps<{
  /**
   * Two-way binding for whether the dialog is shown. Caller drives
   * open via `v-model:visible="..."`; the dialog drives close on
   * Esc / outside-click / explicit close button / successful
   * selection. Mirrors WPF `Window.IsOpen` plus the always-close
   * branch in `l_GameList_SelectionChanged`.
   */
  visible: boolean
  /**
   * Active session region. Used to build `<img src>` URLs for
   * each game banner via {@link imageUrl} (TW / HK have different
   * CDN bases). Required because the store is intentionally
   * auth-unaware (see store docblock); the parent already has
   * `auth.session.region` in scope so the prop is free.
   */
  region: LoginRegion
}>()

const emit = defineEmits<{
  /** Two-way binding back to the caller's `visible` ref. */
  (event: 'update:visible', next: boolean): void
  /**
   * Fired when the user picks a *different* game from the
   * currently-selected one. Payload mirrors the WPF
   * `(service_code, service_region)` pair the legacy
   * `selectedGameChanged()` consumed (`MainWindow.xaml.cs` L661).
   *
   * Re-clicking the already-selected game intentionally does
   * **not** emit (mirrors WPF's `if (service_code != ... ||
   * service_region != ...)` early-exit) so `AccountList.vue`
   * doesn't redundantly reload the account list / re-fetch
   * remain points for the same game.
   */
  (event: 'select', serviceCode: string, serviceRegion: string): void
}>()

const { t } = useI18n()
const game = useGameStore()

const visible = computed({
  get: (): boolean => props.visible,
  set: (next: boolean): void => emit('update:visible', next),
})

/*
 * Single source of truth for which 4-state branch the template
 * renders. Computed off `game.loadState` + `game.services.length`
 * so the binding is reactive without the template having to
 * spell out the boolean compounds. Mirrors the same
 * `loading | error | empty | loaded` shape `pages/AccountList.vue`
 * uses for its own list load (DRY across pages without forcing
 * a shared composable that today has only two callers).
 *
 * `idle` is folded into `loading` because the dialog always kicks
 * off `loadGames()` on first open — the user should never see an
 * idle state inside this dialog (the visible `false → true`
 * watcher triggers the fetch immediately, see below).
 */
type ViewState = 'loading' | 'error' | 'empty' | 'loaded'

const viewState = computed<ViewState>(() => {
  if (game.loadState === 'error') return 'error'
  if (game.loadState === 'loaded') {
    return game.services.length === 0 ? 'empty' : 'loaded'
  }
  return 'loading'
})

/*
 * Trigger the catalogue fetch on every `false → true` transition.
 * `loadGames()` itself is idempotent (no-op when already loaded),
 * so re-opening the dialog mid-session does not re-IPC. A force
 * refresh only runs when the user clicks Retry from the error
 * banner. Initial open from a freshly-cleared session (the
 * `clearAccountSession` bridge in `main.ts` resets `loadState`
 * to `'idle'`) fires a real network call.
 */
watch(
  visible,
  (next, prev) => {
    if (next && prev !== true) {
      void game.loadGames()
    }
  },
  { immediate: true },
)

function isSelected(s: GameService): boolean {
  return gameCodeOf(s.service_code, s.service_region) === game.selectedGameCode
}

function handlePick(s: GameService): void {
  /*
   * WPF parity: the selection-changed handler short-circuits the
   * `selectedGameChanged()` call when the user re-clicked the
   * same game, but **always** calls `this.Close()`. The selection
   * ring stays where it was; the dialog dismisses without
   * triggering a redundant account-list reload upstream.
   */
  if (!isSelected(s)) {
    game.selectGame(s.service_code, s.service_region)
    emit('select', s.service_code, s.service_region)
  }
  visible.value = false
}

function handleClose(): void {
  visible.value = false
}

function handleRetry(): void {
  /*
   * Force-refresh: the error branch implies the previous fetch
   * failed, so the cache short-circuit must be bypassed. Mirrors
   * the same `force = true` shape `AccountList.vue` uses for its
   * load-failure banner Retry button.
   */
  void game.loadGames(true)
}

function bannerUrl(s: GameService): string {
  /*
   * `large_image_name` matches the field WPF's `GameList.xaml.cs`
   * passes to `Game.image` (`game.Large_image`). Empty image name
   * resolves to the bare base URL (404 in the WebView), which
   * `<img>` will render as a broken-image icon — matching WPF's
   * empty `Image.Source` behaviour (no fallback).
   */
  return imageUrl(s.large_image_name, props.region)
}
</script>

<template>
  <el-dialog
    v-model="visible"
    :close-on-click-modal="true"
    :close-on-press-escape="true"
    :show-close="false"
    :width="720"
    align-center
    append-to-body
    class="game-list-dialog"
    data-test="game-list-dialog"
  >
    <template #header>
      <div class="game-list__header">
        <div class="game-list__header-meta">
          <el-icon class="game-list__header-icon" :size="20">
            <VideoPlay />
          </el-icon>
          <div class="game-list__header-text">
            <span class="game-list__header-title" data-test="game-list-title">
              {{ t('GameSelected') }}
            </span>
            <span class="game-list__header-subtitle">
              {{ t('gameList.subtitle') }}
            </span>
          </div>
        </div>
        <button
          type="button"
          class="game-list__header-close"
          :title="t('Cancel')"
          data-test="game-list-close"
          @click="handleClose"
        >
          <el-icon><CircleClose /></el-icon>
        </button>
      </div>
    </template>

    <div class="game-list__body bf-custom-scrollbar">
      <div
        v-if="viewState === 'loading'"
        class="game-list__state game-list__state--loading"
        data-test="game-list-loading"
        role="status"
        aria-live="polite"
      >
        <el-icon class="game-list__state-icon game-list__spinner" :size="28">
          <Refresh />
        </el-icon>
        <p class="game-list__state-text">{{ t('gameList.loading') }}</p>
      </div>

      <div
        v-else-if="viewState === 'error'"
        class="game-list__state game-list__state--error"
        data-test="game-list-error"
        role="alert"
      >
        <el-icon class="game-list__state-icon" :size="28">
          <Warning />
        </el-icon>
        <p class="game-list__state-text">
          {{ game.loadError ?? t('gameList.loadFailed') }}
        </p>
        <el-button
          class="bf-btn-secondary game-list__retry"
          data-test="game-list-retry"
          @click="handleRetry"
        >
          <el-icon><Refresh /></el-icon>
          <span>{{ t('accountList.retry') }}</span>
        </el-button>
      </div>

      <div
        v-else-if="viewState === 'empty'"
        class="game-list__state game-list__state--empty"
        data-test="game-list-empty"
      >
        <el-icon class="game-list__state-icon" :size="28">
          <VideoPlay />
        </el-icon>
        <p class="game-list__state-text">{{ t('gameList.empty') }}</p>
      </div>

      <ul v-else class="game-list__grid" data-test="game-list-grid">
        <li
          v-for="svc in game.services"
          :key="`${svc.service_code}_${svc.service_region}`"
          class="game-list__item"
          :class="{ 'game-list__item--selected': isSelected(svc) }"
          :data-test="`game-list-item-${svc.service_code}_${svc.service_region}`"
          :data-selected="isSelected(svc) ? 'true' : 'false'"
          tabindex="0"
          role="button"
          :aria-pressed="isSelected(svc) ? 'true' : 'false'"
          @click="handlePick(svc)"
          @keyup.enter="handlePick(svc)"
          @keyup.space.prevent="handlePick(svc)"
        >
          <img
            class="game-list__item-image"
            :src="bannerUrl(svc)"
            :alt="t('gameList.imageAlt', { name: svc.name })"
            loading="lazy"
            :data-test="`game-list-image-${svc.service_code}_${svc.service_region}`"
          />
          <span
            class="game-list__item-name"
            :data-test="`game-list-name-${svc.service_code}_${svc.service_region}`"
          >
            {{ svc.name }}
          </span>
        </li>
      </ul>
    </div>
  </el-dialog>
</template>

<style scoped>
.game-list__header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.75rem;
}

.game-list__header-meta {
  display: inline-flex;
  align-items: center;
  gap: 0.625rem;
  min-width: 0;
}

.game-list__header-icon {
  color: var(--bf-primary);
  flex-shrink: 0;
}

.game-list__header-text {
  display: flex;
  flex-direction: column;
  min-width: 0;
}

.game-list__header-title {
  font-size: 0.9375rem;
  font-weight: 700;
  color: var(--bf-on-surface);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.game-list__header-subtitle {
  font-size: 0.75rem;
  color: var(--bf-on-surface-variant);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.game-list__header-close {
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

.game-list__header-close:hover {
  background: color-mix(in srgb, var(--bf-danger) 80%, transparent);
  color: var(--bf-on-danger);
}

.game-list__body {
  min-height: 320px;
  max-height: 60vh;
  overflow-y: auto;
  padding: 0.25rem;
}

.game-list__state {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 0.625rem;
  min-height: 280px;
  padding: 1rem;
  text-align: center;
}

.game-list__state-icon {
  color: var(--bf-on-surface-variant);
}

.game-list__state--error .game-list__state-icon {
  color: var(--bf-danger, #ba1a1a);
}

.game-list__state-text {
  margin: 0;
  font-size: 0.8125rem;
  color: var(--bf-on-surface-variant);
  max-width: 360px;
}

.game-list__retry {
  display: inline-flex;
  align-items: center;
  gap: 0.375rem;
  margin-top: 0.5rem;
}

.game-list__spinner {
  animation: game-list-spin 1.1s linear infinite;
}

@keyframes game-list-spin {
  from {
    transform: rotate(0deg);
  }
  to {
    transform: rotate(360deg);
  }
}

.game-list__grid {
  list-style: none;
  margin: 0;
  padding: 0.5rem;
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(160px, 1fr));
  gap: 0.75rem;
}

.game-list__item {
  display: flex;
  flex-direction: column;
  align-items: stretch;
  gap: 0.375rem;
  padding: 0.375rem;
  background: var(--bf-surface-container-lowest, #fff);
  border: 1px solid var(--bf-outline-variant);
  border-radius: var(--bf-radius-input);
  cursor: pointer;
  transition:
    border-color var(--bf-motion-fast),
    box-shadow var(--bf-motion-fast);
  outline: none;
}

.game-list__item:hover,
.game-list__item:focus-visible {
  border-color: var(--bf-primary);
}

.game-list__item:focus-visible {
  box-shadow: 0 0 0 2px color-mix(in srgb, var(--bf-primary-container) 35%, transparent);
}

.game-list__item--selected {
  border-color: var(--bf-primary);
  box-shadow: 0 0 0 2px color-mix(in srgb, var(--bf-primary-container) 45%, transparent);
}

.game-list__item-image {
  display: block;
  width: 100%;
  aspect-ratio: 152 / 102;
  object-fit: cover;
  border-radius: calc(var(--bf-radius-input) - 2px);
  background: var(--bf-surface-container-low);
}

.game-list__item-name {
  font-size: 0.8125rem;
  font-weight: 600;
  color: var(--bf-on-surface);
  text-align: center;
  line-height: 1.3;
  word-break: break-word;
  padding: 0 0.25rem 0.125rem;
}
</style>
