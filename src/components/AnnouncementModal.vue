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
  <div v-if="visible" class="announcement" data-testid="announcement">
    <div class="announcement__card" role="dialog" aria-modal="true">
      <h2 class="announcement__title">{{ t('announcement.title') }}</h2>
      <p class="announcement__intro">{{ t('announcement.intro') }}</p>
      <ul class="announcement__list">
        <li><strong>Beanfun</strong>：{{ t('announcement.beanfun') }}</li>
        <li><strong>MapleLink</strong>：{{ t('announcement.maplelink') }}</li>
      </ul>
      <div class="announcement__links">
        <a
          class="announcement__link"
          data-testid="announcement-maplelink"
          @click="open(MAPLELINK_URL)"
        >
          MapleLink ↗
        </a>
        <a class="announcement__link" data-testid="announcement-issue" @click="open(ISSUE_323_URL)">
          {{ t('announcement.moreInfoLink') }} ↗
        </a>
      </div>
      <button
        class="announcement__btn"
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
    class="announcement-chip"
    type="button"
    data-testid="announcement-chip"
    @click="reopen"
  >
    📢 {{ t('announcement.reopen') }}
  </button>
</template>

<style scoped>
.announcement {
  position: fixed;
  inset: 0;
  z-index: 3000;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 28px;
  background: rgba(0, 0, 0, 0.55);
  backdrop-filter: blur(4px);
}

.announcement__card {
  width: 100%;
  max-width: 520px;
  max-height: calc(100vh - 56px);
  overflow-y: auto;
  padding: 28px 28px 22px;
  border-radius: 16px;
  border: 1px solid rgba(255, 255, 255, 0.55);
  background: #fffdfa;
  color: #1f1a16;
  box-shadow: 0 18px 48px rgba(0, 0, 0, 0.35);
}

.announcement__title {
  margin: 0 0 14px;
  font-size: 1.125rem;
  font-weight: 800;
}

.announcement__intro {
  margin: 0 0 14px;
  font-size: 0.9375rem;
  line-height: 1.7;
  color: #40342b;
}

.announcement__list {
  margin: 0 0 16px;
  padding-left: 1.15rem;
  display: flex;
  flex-direction: column;
  gap: 10px;
  font-size: 0.9375rem;
  line-height: 1.65;
  color: #40342b;
}

.announcement__links {
  display: flex;
  flex-wrap: wrap;
  gap: 8px 18px;
  margin-bottom: 22px;
}

.announcement__link {
  cursor: pointer;
  font-size: 0.875rem;
  font-weight: 700;
  color: var(--bf-primary, #954a00);
  text-decoration: none;
}

.announcement__link:hover {
  text-decoration: underline;
}

.announcement__btn {
  width: 100%;
  padding: 0.7rem 1rem;
  border: none;
  border-radius: 10px;
  font-size: 0.9375rem;
  font-weight: 700;
  color: #fff;
  background: var(--el-color-primary, #ff8201);
  cursor: pointer;
}

.announcement__btn:disabled {
  opacity: 0.55;
  cursor: not-allowed;
}

/* Small, unobtrusive re-open affordance (bottom-left corner). */
.announcement-chip {
  position: fixed;
  left: 12px;
  bottom: 12px;
  z-index: 2000;
  padding: 5px 11px;
  border: 1px solid rgba(0, 0, 0, 0.1);
  border-radius: 999px;
  font-size: 0.75rem;
  font-weight: 700;
  color: var(--bf-primary, #954a00);
  background: rgba(255, 255, 255, 0.9);
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.16);
  cursor: pointer;
  opacity: 0.85;
}

.announcement-chip:hover {
  opacity: 1;
}
</style>
