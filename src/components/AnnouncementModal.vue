<script setup lang="ts">
/**
 * One-shot project announcement modal (dual-line development notice,
 * issue #323).
 *
 * # Behaviour (per the request)
 *
 * - **Per-update, first-launch only.** Shows when the current app version
 *   differs from the version the user last acknowledged (persisted in
 *   Config.xml under {@link SEEN_KEY}). A brand-new install has no stored
 *   value, so it shows once; after an update the stored value is stale, so
 *   it shows again. Same version already acknowledged → never shows.
 * - **Forced 60-second read.** The only dismiss control is disabled until a
 *   {@link READ_SECONDS}-second countdown elapses, so the notice can't be
 *   clicked through instantly.
 * - **Not a persistent page banner.** It is a single modal, and the only
 *   way to close it is the "read + don't show again" button — which
 *   persists the current version so it won't pop again until the next
 *   update. There is no X / Esc / click-outside close, and nothing is left
 *   pinned to any page.
 *
 * Mounted once at the app root ({@link App.vue}) so it overlays whatever
 * route is active on launch.
 */

import { onBeforeUnmount, onMounted, ref } from 'vue'
import { useI18n } from 'vue-i18n'

import { commands } from '../types/bindings'
import { safeInvoke } from '../services/invoke'
import { useConfigStore } from '../stores/config'

/** Seconds the user must wait before the dismiss button enables. */
const READ_SECONDS = 60
/** Config.xml key holding the last app version the user acknowledged. */
const SEEN_KEY = 'announcementSeenVersion'
const MAPLELINK_URL = 'https://github.com/lshw54/maplelink'
const ISSUE_294_URL = 'https://github.com/pungin/Beanfun/issues/294'

defineOptions({ name: 'AnnouncementModal' })

const { t } = useI18n()
const config = useConfigStore()

const visible = ref(false)
const remaining = ref(READ_SECONDS)
let timer: ReturnType<typeof setInterval> | null = null
let appVersion = ''

function stopTimer(): void {
  if (timer !== null) {
    clearInterval(timer)
    timer = null
  }
}

onMounted(async () => {
  // `commands.version` never returns a Result — a throw is an IPC-bridge
  // failure, in which case we simply don't show the notice (non-critical).
  try {
    const info = await commands.version()
    appVersion = info.app
  } catch {
    return
  }
  if (!appVersion) return
  // Already acknowledged for this exact version → stay hidden.
  if (config.get(SEEN_KEY) === appVersion) return

  visible.value = true
  timer = setInterval(() => {
    if (remaining.value > 0) remaining.value -= 1
    if (remaining.value <= 0) stopTimer()
  }, 1000)
})

onBeforeUnmount(stopTimer)

async function open(url: string): Promise<void> {
  await safeInvoke(commands.openUrl(url))
}

async function dismiss(): Promise<void> {
  if (remaining.value > 0) return // forced-read not finished
  visible.value = false
  stopTimer()
  // Persist so it won't pop again until the next app version. Best-effort:
  // `config.set` already handles a read-only Config.xml gracefully.
  await config.set(SEEN_KEY, appVersion)
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
        <a class="announcement__link" data-testid="announcement-issue" @click="open(ISSUE_294_URL)">
          {{ t('announcement.moreInfoLink') }} ↗
        </a>
      </div>
      <button
        class="announcement__btn"
        type="button"
        :disabled="remaining > 0"
        data-testid="announcement-dismiss"
        @click="dismiss"
      >
        {{
          remaining > 0
            ? t('announcement.reading', { seconds: remaining })
            : t('announcement.dismiss')
        }}
      </button>
    </div>
  </div>
</template>

<style scoped>
.announcement {
  position: fixed;
  inset: 0;
  z-index: 3000;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 24px;
  background: rgba(0, 0, 0, 0.55);
  backdrop-filter: blur(4px);
}

.announcement__card {
  width: 100%;
  max-width: 440px;
  max-height: calc(100vh - 48px);
  overflow-y: auto;
  padding: 24px 24px 20px;
  border-radius: 14px;
  border: 1px solid rgba(255, 255, 255, 0.55);
  background: #fffdfa;
  color: #1f1a16;
  box-shadow: 0 18px 48px rgba(0, 0, 0, 0.35);
}

.announcement__title {
  margin: 0 0 12px;
  font-size: 1.0625rem;
  font-weight: 800;
}

.announcement__intro {
  margin: 0 0 12px;
  font-size: 0.875rem;
  line-height: 1.65;
  color: #40342b;
}

.announcement__list {
  margin: 0 0 14px;
  padding-left: 1.15rem;
  display: flex;
  flex-direction: column;
  gap: 8px;
  font-size: 0.875rem;
  line-height: 1.6;
  color: #40342b;
}

.announcement__links {
  display: flex;
  flex-wrap: wrap;
  gap: 8px 18px;
  margin-bottom: 18px;
}

.announcement__link {
  cursor: pointer;
  font-size: 0.8125rem;
  font-weight: 700;
  color: var(--bf-primary, #954a00);
  text-decoration: none;
}

.announcement__link:hover {
  text-decoration: underline;
}

.announcement__btn {
  width: 100%;
  padding: 0.625rem 1rem;
  border: none;
  border-radius: 10px;
  font-size: 0.875rem;
  font-weight: 700;
  color: #fff;
  background: var(--el-color-primary, #ff8201);
  cursor: pointer;
}

.announcement__btn:disabled {
  opacity: 0.55;
  cursor: not-allowed;
}
</style>
