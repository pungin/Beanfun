<script setup lang="ts">
/**
 * Project announcement modal (dual-line development notice, issue #323).
 *
 * # Behaviour (per the request)
 *
 * - **Per-update, first-launch forced read.** On first launch after an
 *   update (current app version ≠ the version stored in Config.xml under
 *   {@link SEEN_KEY}) the notice pops automatically and its only button is
 *   disabled for a {@link READ_SECONDS}-second countdown, after which it
 *   becomes "read + don't show again" and persists the version.
 * - **Bigger window while shown.** The app's window is normally sized for a
 *   single form (e.g. the login page); showing the notice temporarily grows
 *   the OS window so the announcement isn't crammed, and restores the
 *   previous size on close.
 * - **Re-openable afterwards.** Once acknowledged (or on any later launch of
 *   the same version) a small "📢 公告" chip stays available so the user can
 *   re-open and re-read the notice at will — this time without the forced
 *   countdown. It is not a persistent full-width banner and never blocks a
 *   page.
 *
 * Mounted once at the app root ({@link App.vue}) so it overlays whatever
 * route is active.
 */

import { onBeforeUnmount, onMounted, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { LogicalSize } from '@tauri-apps/api/dpi'

import { commands } from '../types/bindings'
import { safeInvoke } from '../services/invoke'
import { setWindowFitSuspended } from '../services/windowFit'
import { useConfigStore } from '../stores/config'

/** Seconds the user must wait before the dismiss button enables. */
const READ_SECONDS = 60
/** Config.xml key holding the last app version the user acknowledged. */
const SEEN_KEY = 'announcementSeenVersion'
/** Logical size the window grows to while the notice is shown. */
const BIG_W = 640
const BIG_H = 720
const MAPLELINK_URL = 'https://github.com/lshw54/maplelink'
const ISSUE_323_URL = 'https://github.com/pungin/Beanfun/issues/323'

defineOptions({ name: 'AnnouncementModal' })

const { t } = useI18n()
const config = useConfigStore()

/** Version loaded → the re-open chip may render. */
const ready = ref(false)
const visible = ref(false)
/** `true` = the auto-shown, countdown-gated first read; `false` = review. */
const forced = ref(false)
const remaining = ref(READ_SECONDS)
let timer: ReturnType<typeof setInterval> | null = null
let appVersion = ''
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

function isSeen(): boolean {
  return config.get(SEEN_KEY) === appVersion
}

async function openForced(): Promise<void> {
  visible.value = true
  forced.value = true
  remaining.value = READ_SECONDS
  await growWindow()
  stopTimer()
  timer = setInterval(() => {
    if (remaining.value > 0) remaining.value -= 1
    if (remaining.value <= 0) stopTimer()
  }, 1000)
}

async function reopen(): Promise<void> {
  // Review mode — they've already read it, so no countdown.
  visible.value = true
  forced.value = false
  remaining.value = 0
  await growWindow()
}

async function close(): Promise<void> {
  if (forced.value && remaining.value > 0) return // forced-read not finished
  visible.value = false
  stopTimer()
  await restoreWindow()
  // Persist on the first (forced) acknowledgement so it won't auto-pop
  // again until the next version. Idempotent for review closes.
  if (!isSeen()) await config.set(SEEN_KEY, appVersion)
}

onMounted(async () => {
  // `commands.version` never returns a Result — a throw is an IPC-bridge
  // failure, in which case we simply don't show anything (non-critical).
  try {
    const info = await commands.version()
    appVersion = info.app
  } catch {
    return
  }
  if (!appVersion) return
  ready.value = true
  if (!isSeen()) await openForced()
})

onBeforeUnmount(stopTimer)

async function open(url: string): Promise<void> {
  await safeInvoke(commands.openUrl(url))
}
</script>

<template>
  <div v-if="visible" class="ann" data-testid="announcement">
    <div class="ann__card" role="dialog" aria-modal="true">
      <header class="ann__head">
        <span class="ann__badge" aria-hidden="true">📢</span>
        <h2 class="ann__title">{{ t('announcement.title') }}</h2>
      </header>

      <p class="ann__intro">{{ t('announcement.intro') }}</p>

      <div class="ann__tracks">
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

      <div class="ann__links">
        <a class="ann__link" data-testid="announcement-maplelink" @click="open(MAPLELINK_URL)">
          MapleLink ↗
        </a>
        <a class="ann__link" data-testid="announcement-issue" @click="open(ISSUE_323_URL)">
          {{ t('announcement.moreInfoLink') }} ↗
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

  <button
    v-else-if="ready"
    class="ann-chip"
    type="button"
    data-testid="announcement-chip"
    @click="reopen"
  >
    <span aria-hidden="true">📢</span> {{ t('announcement.reopen') }}
  </button>
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

.ann__badge {
  font-size: 1.35rem;
  line-height: 1;
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

/* Small, unobtrusive re-open affordance (bottom-left corner). */
.ann-chip {
  position: fixed;
  left: 12px;
  bottom: 12px;
  z-index: 2000;
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 5px 12px;
  border: 1px solid var(--bf-outline-variant, rgba(128, 128, 128, 0.25));
  border-radius: var(--bf-radius-pill, 999px);
  font-size: 0.72rem;
  font-weight: 700;
  color: var(--bf-primary, #954a00);
  background: var(--bf-surface-container, rgba(255, 255, 255, 0.92));
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.18);
  cursor: pointer;
  opacity: 0.9;
}

.ann-chip:hover {
  opacity: 1;
}
</style>
