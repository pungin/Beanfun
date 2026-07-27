<script setup lang="ts">
/**
 * Announcement modal + history dialog.
 *
 * # Levels (see `src/constants/announcement.ts` for the registry)
 *
 * - **`info`** — pops once until acknowledged; closable immediately.
 * - **`forced`** — pops once until acknowledged; the button is disabled
 *   for the announcement's own `forcedSeconds` countdown first.
 * - **`forcedEveryTime`** — pops on **every** launch and is never
 *   recorded as read, for notices that must be re-read each start.
 *
 * Which one pops is decided by `pendingAnnouncement`, so publishing a
 * notice is an edit to the registry, never to this component.
 *
 * # The rest of the behaviour
 *
 * - **Bigger window while shown.** The app window is normally sized for
 *   a single form; showing a notice temporarily grows it so the card
 *   isn't crammed, and restores the previous size on close.
 * - **Always reachable afterwards.** A slim permanent banner under the
 *   title bar opens the **history dialog**, which lists every
 *   announcement ever published with its read state — any of them can
 *   be re-read, always without a countdown. Settings opens the same
 *   dialog through `services/announcementUi`.
 * - **Robust "seen" state.** Acknowledged ids live in both Config.xml
 *   and WebView localStorage; either store counts, so hand-wiping one
 *   doesn't re-force a read. Values written by pre-registry builds (a
 *   lone id, or an app version like `"6.0.5"`) are folded in by
 *   `parseSeenIds`, so nobody is re-forced by the format change.
 *
 * Mounted once at the app root ({@link App.vue}) so it overlays whatever
 * route is active.
 */

import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { LogicalSize } from '@tauri-apps/api/dpi'

import { commands } from '../types/bindings'
import { safeInvoke } from '../services/invoke'
import { setWindowFitSuspended } from '../services/windowFit'
import {
  announcementListOpen,
  closeAnnouncementList,
  openAnnouncementList,
} from '../services/announcementUi'
import { useConfigStore } from '../stores/config'
import {
  ANNOUNCEMENTS,
  ANNOUNCEMENT_SEEN_KEY,
  LATEST_ANNOUNCEMENT,
  LEGACY_ANNOUNCEMENT_SEEN_KEY,
  isAcknowledgeable,
  isForcedLevel,
  parseSeenIds,
  pendingAnnouncement,
  serializeSeenIds,
  type AnnouncementDef,
} from '../constants/announcement'

/** Logical size the window grows to while a notice is shown. */
const BIG_W = 640
const BIG_H = 720

defineOptions({ name: 'AnnouncementModal' })

const { t } = useI18n()
const config = useConfigStore()

/** Config loaded → the banner may render. */
const ready = ref(false)
const visible = ref(false)

/** The announcement currently rendered in the card. */
const current = ref<AnnouncementDef>(LATEST_ANNOUNCEMENT)
/** Acknowledged ids, hydrated from both stores on mount. */
const seenIds = ref<Set<string>>(new Set())

/** Every announcement, newest first, for the history dialog. */
const history = computed(() =>
  ANNOUNCEMENTS.map((def) => ({ def, read: seenIds.value.has(def.id) })),
)

/** The history dialog's open flag lives in a shared module — Settings
 * and the title-bar banner both open it from outside this component. */
const listOpen = announcementListOpen()

/**
 * The slim re-open banner is **permanent**: whenever the notice card
 * itself is closed it sits under the title bar so the announcement is
 * always one click away (no session-dismiss). It never triggers the
 * countdown — {@link reopen} is review mode.
 */
const bannerVisible = computed(() => ready.value && !visible.value)

/**
 * Reserve a strip under the title bar while the banner is up so the
 * `position: fixed` banner never overlaps the page content (it used to
 * cover the top of the login form). Toggles a global body class picked
 * up by the unscoped style block below; the router's content-fit
 * resizer then re-measures and grows the window to include the strip.
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
/** `true` = the auto-shown, countdown-gated first read; `false` = review. */
const forced = ref(false)
const remaining = ref(0)
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

/**
 * Hydrate the acknowledged-id set from both stores plus the legacy
 * single-value key (see `parseSeenIds`). Either store alone counts, so
 * wiping one by hand doesn't re-force anything.
 */
function loadSeenIds(): Set<string> {
  let localCurrent: string | null = null
  let localLegacy: string | null = null
  try {
    localCurrent = localStorage.getItem(ANNOUNCEMENT_SEEN_KEY)
    localLegacy = localStorage.getItem(LEGACY_ANNOUNCEMENT_SEEN_KEY)
  } catch {
    /* localStorage unavailable — Config.xml still answers */
  }
  const merged = parseSeenIds(
    config.get(ANNOUNCEMENT_SEEN_KEY),
    config.get(LEGACY_ANNOUNCEMENT_SEEN_KEY),
  )
  for (const id of parseSeenIds(localCurrent, localLegacy)) merged.add(id)
  return merged
}

/**
 * Record `def` as acknowledged in BOTH stores, so it never auto-pops
 * again — and stays that way even if one store is later wiped by hand.
 * `forcedEveryTime` notices are never recorded: they must return on the
 * next launch.
 */
async function markSeen(def: AnnouncementDef): Promise<void> {
  if (!isAcknowledgeable(def)) return
  const next = new Set(seenIds.value)
  next.add(def.id)
  seenIds.value = next
  const serialized = serializeSeenIds(next)
  try {
    localStorage.setItem(ANNOUNCEMENT_SEEN_KEY, serialized)
  } catch {
    /* localStorage unavailable — Config.xml below still records it */
  }
  await config.set(ANNOUNCEMENT_SEEN_KEY, serialized)
}

/** Auto-open `def` with its level's countdown. */
async function openPending(def: AnnouncementDef): Promise<void> {
  current.value = def
  visible.value = true
  forced.value = isForcedLevel(def)
  remaining.value = forced.value ? def.forcedSeconds : 0
  await growWindow()
  stopTimer()
  if (remaining.value > 0) {
    timer = setInterval(() => {
      if (remaining.value > 0) remaining.value -= 1
      if (remaining.value <= 0) stopTimer()
    }, 1000)
  }
}

/** Open `def` for review — never a countdown, never re-acknowledged. */
async function openForReview(def: AnnouncementDef): Promise<void> {
  closeAnnouncementList()
  current.value = def
  visible.value = true
  forced.value = false
  remaining.value = 0
  stopTimer()
  await growWindow()
}

async function close(): Promise<void> {
  if (forced.value && remaining.value > 0) return // forced-read not finished
  const def = current.value
  const wasForced = forced.value
  visible.value = false
  stopTimer()
  await restoreWindow()
  // Only the auto-shown read acknowledges; a review close leaves the
  // record alone (and `forcedEveryTime` never records at all).
  if (wasForced || !seenIds.value.has(def.id)) await markSeen(def)
}

/**
 * Resolve once the config store has finished loading Config.xml into its
 * cache (or after a safety timeout). Checking {@link isSeen} before this
 * reads an *empty* cache — so a previously-acknowledged version looks
 * unseen and the forced read re-fires on every launch. The localStorage
 * fallback in {@link isSeen} isn't enough: some WebView2 profiles don't
 * persist it across restarts, so Config.xml is the store that actually
 * survives and it must be loaded first. Mirrors the `watch(config.loaded)`
 * idiom already used in `LoginRegionSelection.vue`.
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
  // Wait for Config.xml before the seen-check, or a still-empty cache
  // makes an acknowledged announcement look unseen and re-forces its
  // read every launch. See {@link waitForConfigLoaded}.
  await waitForConfigLoaded()
  seenIds.value = loadSeenIds()
  ready.value = true
  const pending = pendingAnnouncement(seenIds.value)
  if (pending) await openPending(pending)
})

onBeforeUnmount(() => {
  stopTimer()
  if (typeof document !== 'undefined') {
    document.body.classList.remove('bf-ann-banner-open')
  }
})

async function open(url: string): Promise<void> {
  await safeInvoke(commands.openUrl(url))
}
</script>

<template>
  <div v-if="visible" class="ann" data-testid="announcement">
    <div class="ann__card" role="dialog" aria-modal="true">
      <header class="ann__head">
        <h2 class="ann__title">{{ t(current.titleKey) }}</h2>
      </header>

      <p v-for="key in current.bodyKeys" :key="key" class="ann__intro">{{ t(key) }}</p>

      <!-- Bespoke two-track body kept for the #323 dual-line notice;
           later announcements render their bodyKeys as plain paragraphs. -->
      <div v-if="current.layout === 'dualLine'" class="ann__tracks">
        <div class="ann__track">
          <span class="ann__dot ann__dot--beanfun" aria-hidden="true"></span>
          <div class="ann__track-body">
            <div class="ann__track-name">Beanfun</div>
            <div class="ann__track-desc">{{ t('announcement.beanfun') }}</div>
          </div>
        </div>
        <div class="ann__track">
          <span class="ann__dot ann__dot--maple" aria-hidden="true"></span>
          <div class="ann__track-body">
            <div class="ann__track-name">MapleLink</div>
            <div class="ann__track-desc">{{ t('announcement.maplelink') }}</div>
          </div>
        </div>
      </div>

      <div v-if="current.links?.length" class="ann__links">
        <a
          v-for="link in current.links"
          :key="link.url"
          class="ann__link"
          :data-testid="`announcement-link-${link.labelKey}`"
          @click="open(link.url)"
        >
          {{ t(link.labelKey) }} ↗
        </a>
      </div>

      <button
        class="ann__btn"
        type="button"
        :disabled="forced && remaining > 0"
        data-testid="announcement-dismiss"
        @click="close"
      >
        <template v-if="forced">
          {{
            remaining > 0
              ? t('announcement.reading', { seconds: remaining })
              : t('announcement.dismiss')
          }}
        </template>
        <template v-else>{{ t('announcement.close') }}</template>
      </button>
    </div>
  </div>

  <!-- History: every announcement ever published, with its read state.
       Opened from the banner below or from Settings. -->
  <div v-if="listOpen && !visible" class="ann" data-testid="announcement-list">
    <div class="ann__card" role="dialog" aria-modal="true">
      <header class="ann__head">
        <h2 class="ann__title">{{ t('announcement.historyTitle') }}</h2>
      </header>
      <p class="ann__intro">{{ t('announcement.historyHint') }}</p>

      <ul class="ann-list">
        <li v-for="entry in history" :key="entry.def.id">
          <button
            type="button"
            class="ann-list__row"
            :data-testid="`announcement-history-${entry.def.id}`"
            @click="openForReview(entry.def)"
          >
            <span class="ann-list__title">{{ t(entry.def.titleKey) }}</span>
            <span class="ann-list__meta">
              <span class="ann-list__level" :class="`ann-list__level--${entry.def.level}`">
                {{ t(`announcement.level.${entry.def.level}`) }}
              </span>
              <span class="ann-list__state">
                {{ entry.read ? t('announcement.stateRead') : t('announcement.stateUnread') }}
              </span>
            </span>
          </button>
        </li>
      </ul>

      <button
        class="ann__btn"
        type="button"
        data-testid="announcement-list-close"
        @click="closeAnnouncementList()"
      >
        {{ t('announcement.close') }}
      </button>
    </div>
  </div>

  <div v-else-if="bannerVisible" class="ann-banner" data-testid="announcement-banner">
    <button
      class="ann-banner__open"
      type="button"
      data-testid="announcement-banner-open"
      @click="openAnnouncementList()"
    >
      <span class="ann-banner__text">{{ t(LATEST_ANNOUNCEMENT.titleKey) }}</span>
      <span class="ann-banner__cta">{{ t('announcement.reopen') }} ›</span>
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
  gap: 10px;
  margin-bottom: 14px;
}

.ann__title {
  margin: 0;
  font-size: 1.15rem;
  font-weight: 800;
  letter-spacing: 0.01em;
}

.ann__intro {
  margin: 0 0 18px;
  font-size: 0.9rem;
  line-height: 1.7;
  color: var(--bf-on-surface-variant, var(--bf-on-surface, #54443a));
}

.ann__tracks {
  display: flex;
  flex-direction: column;
  gap: 10px;
  margin-bottom: 20px;
}

.ann__track {
  display: flex;
  gap: 12px;
  padding: 12px 14px;
  border-radius: var(--bf-radius-card, 10px);
  background: color-mix(in srgb, var(--bf-on-surface, #000) 6%, transparent);
}

.ann__dot {
  flex: 0 0 auto;
  width: 10px;
  height: 10px;
  margin-top: 5px;
  border-radius: 50%;
}

.ann__dot--beanfun {
  background: #ff8201;
}

.ann__dot--maple {
  background: #3aa0ff;
}

.ann__track-name {
  font-size: 0.9rem;
  font-weight: 800;
  margin-bottom: 3px;
}

.ann__track-desc {
  font-size: 0.82rem;
  line-height: 1.6;
  color: var(--bf-on-surface-variant, var(--bf-on-surface, #54443a));
}

.ann__links {
  display: flex;
  flex-wrap: wrap;
  gap: 8px 18px;
  margin-bottom: 20px;
}

.ann__link {
  cursor: pointer;
  font-size: 0.82rem;
  font-weight: 700;
  color: var(--bf-primary, #954a00);
  text-decoration: none;
}

.ann__link:hover {
  text-decoration: underline;
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

.ann-list {
  list-style: none;
  margin: 0 0 20px;
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

.ann-list__title {
  font-size: 0.88rem;
  font-weight: 700;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.ann-list__meta {
  flex: 0 0 auto;
  display: inline-flex;
  align-items: center;
  gap: 8px;
  font-size: 0.72rem;
}

.ann-list__level {
  padding: 2px 8px;
  border-radius: 999px;
  font-weight: 700;
  background: color-mix(in srgb, var(--bf-on-surface-variant, #54443a) 14%, transparent);
  color: var(--bf-on-surface-variant, #54443a);
}

.ann-list__level--forced,
.ann-list__level--forcedEveryTime {
  background: color-mix(in srgb, var(--el-color-primary, #ff8201) 18%, transparent);
  color: var(--bf-primary, #954a00);
}

.ann-list__state {
  color: var(--bf-on-surface-variant, #54443a);
}

/* Full-width re-open banner, pinned just under the 40px custom title bar. */
.ann-banner {
  position: fixed;
  top: 40px;
  left: 0;
  right: 0;
  z-index: 2000;
  display: flex;
  align-items: center;
  height: 30px;
  padding: 0 12px;
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
</style>

<!--
  Global (unscoped) offset. The banner is `position: fixed` under the 40px
  title bar, so on its own it floats over the page content (it used to
  cover the top of the login form). Every page roots at
  `[data-window-root]` with `<TitleBar class="bf-titlebar">` as its first
  child, so reserving a banner-height strip below the title bar pushes the
  content down exactly under the banner. Scoped styles can't reach
  TitleBar in other components, hence the global rule gated by the body
  class toggled in script.
-->
<style>
body.bf-ann-banner-open [data-window-root] > .bf-titlebar {
  margin-bottom: 30px;
}
</style>
