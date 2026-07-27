<script setup lang="ts">
/**
 * Announcement overlay, banner and archive.
 *
 * # Levels (registry: `src/constants/announcement.ts`)
 *
 * - **`info`** — closable at once; closing counts as read.
 * - **`pinned`** — locked for its countdown, after which the × and the
 *   backdrop both work; closing counts as read.
 * - **`blocking`** — same countdown, but only the acknowledge button
 *   counts as read; leaving by × or backdrop brings the notice back on
 *   the next launch.
 *
 * # Reaching a notice again
 *
 * The banner names the newest announcement and opens **it**, not a
 * list — one click, one layer. Its × dismisses the strip for good,
 * which is only reasonable because the text stays reachable: the
 * archive (Settings → announcements) lists every notice ever published,
 * and opening one shows exactly the same body, since both render
 * {@link AnnouncementBody} keyed by id.
 *
 * # Robust read state
 *
 * Acknowledged ids live in both Config.xml and WebView localStorage;
 * either store counts, so hand-wiping one doesn't re-force a read.
 * Values written by pre-registry builds (a lone id, or an app version
 * like `"6.0.5"`) are folded in by `parseSeenIds`, so the format change
 * re-forces nobody.
 *
 * Mounted once at the app root ({@link App.vue}) so it overlays
 * whatever route is active.
 */

import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { LogicalSize } from '@tauri-apps/api/dpi'

import { setWindowFitSuspended } from '../services/windowFit'
import { announcementListOpen, closeAnnouncementList } from '../services/announcementUi'
import { useConfigStore } from '../stores/config'
import {
  ANNOUNCEMENTS,
  ANNOUNCEMENT_BANNER_KEY,
  ANNOUNCEMENT_SEEN_KEY,
  LATEST_ANNOUNCEMENT,
  LEGACY_ANNOUNCEMENT_SEEN_KEY,
  closingCountsAsRead,
  countdownFor,
  parseIdList,
  parseSeenIds,
  pendingAnnouncement,
  serializeIds,
  type AnnouncementDef,
  type CloseIntent,
} from '../constants/announcement'
import AnnouncementBody from './AnnouncementBody.vue'

/** Logical size the window grows to while a notice is shown. */
const BIG_W = 640
const BIG_H = 720

defineOptions({ name: 'AnnouncementModal' })

const { t } = useI18n()
const config = useConfigStore()

/** Config loaded → the banner may render. */
const ready = ref(false)
const visible = ref(false)

/** The announcement currently in the overlay. */
const current = ref<AnnouncementDef>(LATEST_ANNOUNCEMENT)
/** Acknowledged ids, hydrated from both stores on mount. */
const seenIds = ref<Set<string>>(new Set())
/** Ids whose banner strip the user dismissed. */
const bannerDismissed = ref<Set<string>>(new Set())

/** Seconds left before the acknowledge button unlocks (0 = unlocked). */
const remaining = ref(0)
/** `true` while the countdown holds every close affordance shut. */
const locked = computed(() => remaining.value > 0)

/** The archive dialog's open flag — Settings raises it from outside. */
const listOpen = announcementListOpen()

/** Every announcement, newest first, with its read state. */
const archive = computed(() =>
  ANNOUNCEMENTS.map((def) => ({ def, read: seenIds.value.has(def.id) })),
)

/**
 * The strip under the title bar names the newest notice and is up
 * unless the user dismissed it — and never while something else is
 * already on screen.
 */
const bannerVisible = computed(
  () =>
    ready.value &&
    !visible.value &&
    !listOpen.value &&
    !bannerDismissed.value.has(LATEST_ANNOUNCEMENT.id),
)

/**
 * Reserve a strip under the title bar while the banner is up so the
 * `position: fixed` banner never overlaps the page content. Toggles a
 * global body class picked up by the unscoped style block below; the
 * router's content-fit resizer then re-measures the window.
 */
watch(
  bannerVisible,
  (open) => {
    if (typeof document !== 'undefined') {
      document.body.classList.toggle('bf-ann-banner-open', open)
    }
  },
  { immediate: true },
)

let timer: ReturnType<typeof setInterval> | null = null
// Window size captured before growing, restored on close.
let savedSize: Awaited<ReturnType<ReturnType<typeof getCurrentWindow>['innerSize']>> | null = null

function stopTimer(): void {
  if (timer !== null) {
    clearInterval(timer)
    timer = null
  }
}

async function growWindow(): Promise<void> {
  // Best-effort: never let a window API hiccup block the notice.
  try {
    const win = getCurrentWindow()
    savedSize = await win.innerSize()
    // Hold the window at a fixed larger size — suspend the router's
    // content-fit resizer first, or it snaps straight back.
    setWindowFitSuspended(true)
    await win.setSize(new LogicalSize(BIG_W, BIG_H))
  } catch {
    savedSize = null
  }
}

async function restoreWindow(): Promise<void> {
  try {
    if (savedSize) await getCurrentWindow().setSize(savedSize)
  } catch {
    /* best-effort */
  }
  savedSize = null
  setWindowFitSuspended(false)
}

/** Read an id list from localStorage, tolerating a hostile store. */
function readLocal(key: string): string | null {
  try {
    return localStorage.getItem(key)
  } catch {
    return null
  }
}

/** Write an id list to both stores; localStorage is best-effort. */
async function persistIds(key: string, ids: Set<string>): Promise<void> {
  const serialized = serializeIds(ids)
  try {
    localStorage.setItem(key, serialized)
  } catch {
    /* localStorage unavailable — Config.xml below still records it */
  }
  await config.set(key, serialized)
}

/**
 * Hydrate the acknowledged-id set from both stores plus the legacy
 * single-value key. Either store alone counts, so wiping one by hand
 * doesn't re-force anything.
 */
function loadSeenIds(): Set<string> {
  const merged = parseSeenIds(
    config.get(ANNOUNCEMENT_SEEN_KEY),
    config.get(LEGACY_ANNOUNCEMENT_SEEN_KEY),
  )
  for (const id of parseSeenIds(
    readLocal(ANNOUNCEMENT_SEEN_KEY),
    readLocal(LEGACY_ANNOUNCEMENT_SEEN_KEY),
  )) {
    merged.add(id)
  }
  return merged
}

function loadBannerDismissed(): Set<string> {
  const merged = parseIdList(config.get(ANNOUNCEMENT_BANNER_KEY))
  for (const id of parseIdList(readLocal(ANNOUNCEMENT_BANNER_KEY))) merged.add(id)
  return merged
}

/** Record `def` as read in both stores. */
async function markSeen(def: AnnouncementDef): Promise<void> {
  if (seenIds.value.has(def.id)) return
  const next = new Set(seenIds.value)
  next.add(def.id)
  seenIds.value = next
  await persistIds(ANNOUNCEMENT_SEEN_KEY, next)
}

/** Open `def`, applying its level's countdown when asked. */
async function openOverlay(def: AnnouncementDef, withCountdown: boolean): Promise<void> {
  closeAnnouncementList()
  current.value = def
  visible.value = true
  remaining.value = withCountdown ? countdownFor(def) : 0
  stopTimer()
  if (remaining.value > 0) {
    timer = setInterval(() => {
      if (remaining.value > 0) remaining.value -= 1
      if (remaining.value <= 0) stopTimer()
    }, 1000)
  }
  await growWindow()
}

/**
 * Close the overlay. `intent` decides whether this counts as read —
 * only `blocking` cares (see `closingCountsAsRead`), which is what
 * makes it the one level that can return tomorrow.
 */
async function closeOverlay(intent: CloseIntent): Promise<void> {
  if (locked.value) return // countdown still running
  const def = current.value
  visible.value = false
  stopTimer()
  await restoreWindow()
  if (closingCountsAsRead(def, intent)) await markSeen(def)
}

/** Open a notice for review — never a countdown, always closable. */
function openForReview(def: AnnouncementDef): void {
  void openOverlay(def, false)
}

/** Dismiss the banner strip for the newest notice, for good. */
async function dismissBanner(): Promise<void> {
  const next = new Set(bannerDismissed.value)
  next.add(LATEST_ANNOUNCEMENT.id)
  bannerDismissed.value = next
  await persistIds(ANNOUNCEMENT_BANNER_KEY, next)
}

/**
 * Resolve once the config store has finished loading Config.xml into
 * its cache (or after a safety timeout). Checking the read state before
 * this reads an *empty* cache — so an acknowledged notice looks unread
 * and re-opens on every launch.
 */
function waitForConfigLoaded(timeoutMs = 5000): Promise<void> {
  if (config.loaded) return Promise.resolve()
  return new Promise((resolve) => {
    let stop = () => {}
    const timer = setTimeout(() => {
      stop()
      resolve()
    }, timeoutMs)
    stop = watch(
      () => config.loaded,
      (isLoaded) => {
        if (isLoaded) {
          stop()
          clearTimeout(timer)
          resolve()
        }
      },
    )
  })
}

onMounted(async () => {
  await waitForConfigLoaded()
  seenIds.value = loadSeenIds()
  bannerDismissed.value = loadBannerDismissed()
  ready.value = true
  const pending = pendingAnnouncement(seenIds.value)
  if (pending) await openOverlay(pending, true)
})

onBeforeUnmount(() => {
  stopTimer()
  if (typeof document !== 'undefined') {
    document.body.classList.remove('bf-ann-banner-open')
  }
})
</script>

<template>
  <!-- Overlay. The backdrop closes it too, unless the countdown holds
       it shut (and for `blocking` that exit deliberately doesn't count
       as read). -->
  <div v-if="visible" class="ann" data-testid="announcement" @click.self="closeOverlay('dismiss')">
    <div class="ann__card" role="dialog" aria-modal="true">
      <header class="ann__head">
        <h2 class="ann__title">{{ t(current.titleKey) }}</h2>
        <button
          v-if="!locked"
          type="button"
          class="ann__close"
          :aria-label="t('announcement.close')"
          data-testid="announcement-close"
          @click="closeOverlay('dismiss')"
        >
          ×
        </button>
      </header>

      <AnnouncementBody :id="current.id" />

      <button
        class="ann__btn"
        type="button"
        :disabled="locked"
        data-testid="announcement-dismiss"
        @click="closeOverlay('acknowledge')"
      >
        {{ locked ? t('announcement.reading', { seconds: remaining }) : t('announcement.dismiss') }}
      </button>
    </div>
  </div>

  <!-- Archive: subject, date, chevron. Opening one shows the same body
       the overlay does. -->
  <div
    v-else-if="listOpen"
    class="ann"
    data-testid="announcement-list"
    @click.self="closeAnnouncementList()"
  >
    <div class="ann__card" role="dialog" aria-modal="true">
      <header class="ann__head">
        <h2 class="ann__title">{{ t('announcement.historyTitle') }}</h2>
        <button
          type="button"
          class="ann__close"
          :aria-label="t('announcement.close')"
          data-testid="announcement-list-close"
          @click="closeAnnouncementList()"
        >
          ×
        </button>
      </header>

      <ul class="ann-list">
        <li v-for="entry in archive" :key="entry.def.id">
          <button
            type="button"
            class="ann-list__row"
            :data-testid="`announcement-history-${entry.def.id}`"
            @click="openForReview(entry.def)"
          >
            <span class="ann-list__text">
              <span class="ann-list__subject">{{ t(entry.def.titleKey) }}</span>
              <span class="ann-list__date">{{ entry.def.date }}</span>
            </span>
            <span class="ann-list__chevron" aria-hidden="true">›</span>
          </button>
        </li>
      </ul>
    </div>
  </div>

  <!-- Banner: names the newest notice, opens IT (no intermediate list),
       and can be dismissed for good. -->
  <div v-else-if="bannerVisible" class="ann-banner" data-testid="announcement-banner">
    <button
      class="ann-banner__open"
      type="button"
      data-testid="announcement-banner-open"
      @click="openForReview(LATEST_ANNOUNCEMENT)"
    >
      <span class="ann-banner__text">{{ t(LATEST_ANNOUNCEMENT.titleKey) }}</span>
      <span class="ann-banner__cta">{{ t('announcement.reopen') }} ›</span>
    </button>
    <button
      class="ann-banner__close"
      type="button"
      :aria-label="t('announcement.bannerDismiss')"
      :title="t('announcement.bannerDismiss')"
      data-testid="announcement-banner-dismiss"
      @click="dismissBanner"
    >
      ×
    </button>
  </div>
</template>

<style scoped>
/* Theme-aware via the app's --bf-* design tokens (redefined under
   [data-theme="dark"]), so the card follows light / dark automatically. */
.ann {
  position: fixed;
  inset: 0;
  z-index: 3000;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 28px;
  background: rgba(0, 0, 0, 0.5);
  backdrop-filter: blur(6px);
}

.ann__card {
  width: 100%;
  max-width: 520px;
  max-height: calc(100vh - 56px);
  overflow-y: auto;
  padding: 26px 28px 22px;
  border-radius: var(--bf-radius-panel, 14px);
  border: 1px solid var(--bf-outline-variant, rgba(128, 128, 128, 0.25));
  background: var(--bf-surface-container, #f4f4f4);
  color: var(--bf-on-surface, #1f1a16);
  box-shadow: 0 20px 56px rgba(0, 0, 0, 0.4);
}

.ann__head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  margin-bottom: 14px;
}

.ann__title {
  margin: 0;
  font-size: 1.15rem;
  font-weight: 800;
  letter-spacing: 0.01em;
}

.ann__close {
  flex: 0 0 auto;
  width: 28px;
  height: 28px;
  display: grid;
  place-items: center;
  padding: 0;
  border: none;
  border-radius: 8px;
  background: transparent;
  color: var(--bf-on-surface-variant, #54443a);
  font-size: 1.25rem;
  line-height: 1;
  cursor: pointer;
  transition: background 150ms ease;
}

.ann__close:hover {
  background: color-mix(in srgb, var(--bf-on-surface, #000) 10%, transparent);
}

.ann__btn {
  width: 100%;
  padding: 0.72rem 1rem;
  border: none;
  border-radius: var(--bf-radius-button, 10px);
  font-size: 0.92rem;
  font-weight: 700;
  color: #fff;
  background: var(--el-color-primary, #ff8201);
  cursor: pointer;
  transition: filter 0.15s ease;
}

.ann__btn:hover:not(:disabled) {
  filter: brightness(1.06);
}

.ann__btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

/* --------------- archive list --------------- */

.ann-list {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.ann-list__row {
  width: 100%;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 12px 14px;
  border: 1px solid var(--bf-outline-variant, rgba(128, 128, 128, 0.25));
  border-radius: var(--bf-radius-card, 10px);
  background: color-mix(in srgb, var(--bf-on-surface, #000) 4%, transparent);
  color: inherit;
  font: inherit;
  text-align: left;
  cursor: pointer;
  transition: background 150ms ease;
}

.ann-list__row:hover {
  background: color-mix(in srgb, var(--bf-on-surface, #000) 9%, transparent);
}

.ann-list__text {
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.ann-list__subject {
  font-size: 0.88rem;
  font-weight: 700;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.ann-list__date {
  font-size: 0.72rem;
  color: var(--bf-on-surface-variant, #54443a);
}

.ann-list__chevron {
  flex: 0 0 auto;
  font-size: 1.1rem;
  color: var(--bf-on-surface-variant, #54443a);
}

/* --------------- banner --------------- */

/* Full-width strip, pinned just under the 40px custom title bar. */
.ann-banner {
  position: fixed;
  top: 40px;
  left: 0;
  right: 0;
  z-index: 2000;
  display: flex;
  align-items: center;
  height: 30px;
  padding: 0 6px 0 12px;
  background: color-mix(
    in srgb,
    var(--el-color-primary, #ff8201) 16%,
    var(--bf-surface-container, #eee)
  );
  border-bottom: 1px solid var(--bf-outline-variant, rgba(128, 128, 128, 0.25));
  color: var(--bf-on-surface, #221a11);
  font-size: 0.75rem;
}

.ann-banner__open {
  flex: 1 1 auto;
  min-width: 0;
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 0;
  border: none;
  background: none;
  color: inherit;
  font: inherit;
  text-align: left;
  cursor: pointer;
}

.ann-banner__text {
  min-width: 0;
  font-weight: 700;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.ann-banner__cta {
  flex: 0 0 auto;
  font-weight: 800;
  color: var(--bf-primary, #954a00);
}

.ann-banner__close {
  flex: 0 0 auto;
  width: 22px;
  height: 22px;
  margin-left: 8px;
  display: grid;
  place-items: center;
  padding: 0;
  border: none;
  border-radius: 6px;
  background: transparent;
  color: var(--bf-on-surface-variant, #54443a);
  font-size: 1rem;
  line-height: 1;
  cursor: pointer;
}

.ann-banner__close:hover {
  background: color-mix(in srgb, var(--bf-on-surface, #000) 12%, transparent);
}
</style>

<!--
  Global (unscoped) offset. The banner is `position: fixed` under the 40px
  title bar, so on its own it floats over the page content. Every page
  roots at `[data-window-root]` with `<TitleBar class="bf-titlebar">` as
  its first child, so reserving a banner-height strip below the title bar
  pushes the content down exactly under the banner.
-->
<style>
body.bf-ann-banner-open [data-window-root] > .bf-titlebar {
  margin-bottom: 30px;
}
</style>
