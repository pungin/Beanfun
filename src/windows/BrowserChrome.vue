<script setup lang="ts">
/**
 * The in-app browser's toolbar — a child webview pinned above the
 * content view (`commands/web_browser.rs`).
 *
 * # Why this exists
 *
 * Logging in once used to mean the official site was logged in too, so a
 * player could walk from the launcher into an event page without signing
 * in again. The bare browser window broke that: with no address bar the
 * only way off the page it landed on was the developer console — which
 * is what people started using (pungin/Beanfun#382), and what we do not
 * want to ship.
 *
 * # It holds no navigation state of its own
 *
 * Every value here is read back from WebView2 via `browserNavState`, so
 * the arrows and the address bar describe the page that is actually
 * loaded rather than the one we last asked for. In particular the arrows
 * are enabled from `CanGoBack` / `CanGoForward` — an arrow that is always
 * enabled is worse than no arrow.
 *
 * # One toolbar per window
 *
 * `open_in_app_browser` mints a window per click, so this component
 * addresses its own window by label (`getCurrentWindow().label`) rather
 * than relying on there being a single browser.
 */
import { onMounted, onUnmounted, ref } from 'vue'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { useI18n } from 'vue-i18n'

import ConnectionPanel from './ConnectionPanel.vue'
import { commands } from '../types/bindings'
import type { ConnectionInfo, NavState } from '../types/bindings'
import { safeInvoke } from '../services/invoke'
import { IN_APP_BROWSER_INVALID_URL_CODE } from '../composables/useInAppBrowser'

/** Matches `BAR_HEIGHT` in `commands/web_browser.rs`. */
const BAR_HEIGHT = 46

/** How tall the toolbar grows while the connection panel is open. */
const PANEL_HEIGHT = 300

/** The notice row's height, added on top of whatever else is open. */
const NOTICE_HEIGHT = 26

/**
 * How often the toolbar re-reads the content view's state.
 *
 * WebView2 has no event we can subscribe to from here, and a page can
 * move without us asking it to (redirects, in-page links). Polling is
 * the honest option; one native read a second is not measurable.
 */
const POLL_MS = 1000

const { t } = useI18n()
const windowLabel = getCurrentWindow().label

const nav = ref<NavState>({ url: '', title: '', canGoBack: false, canGoForward: false })
/** Non-null only while the user is editing the address. */
const draft = ref<string | null>(null)
const panelOpen = ref(false)
const connection = ref<ConnectionInfo | null>(null)
/**
 * Something the toolbar wants to say, shown briefly on its own row.
 *
 * Deliberately not `ElMessage`, and deliberately not an overlay either.
 * This webview is only as tall as the bar: a toast positioned against the
 * viewport covers the whole of it, and even a small pinned pill sits on
 * top of the controls. The row is real layout, and the window grows by
 * exactly its height while it is up, so nothing is ever covered.
 */
const notice = ref<string | null>(null)
let noticeTimer: ReturnType<typeof setTimeout> | undefined

/** Ask the backend for the height everything currently open needs. */
async function applyChromeHeight(): Promise<void> {
  const base = panelOpen.value ? PANEL_HEIGHT : BAR_HEIGHT
  const wanted = base + (notice.value === null ? 0 : NOTICE_HEIGHT)
  await safeInvoke(commands.browserSetChromeHeight(windowLabel, wanted))
}

function say(message: string): void {
  notice.value = message
  void applyChromeHeight()
  if (noticeTimer !== undefined) clearTimeout(noticeTimer)
  noticeTimer = setTimeout(() => {
    notice.value = null
    void applyChromeHeight()
  }, 5000)
}

let timer: ReturnType<typeof setInterval> | undefined

async function refresh(): Promise<void> {
  const result = await safeInvoke(commands.browserNavState(windowLabel))
  if (!result.ok) return
  // Leave the address alone while it is being typed into.
  if (draft.value !== null && result.data.url === nav.value.url) {
    nav.value = { ...result.data, url: nav.value.url }
    return
  }
  if (result.data.url !== nav.value.url) draft.value = null
  nav.value = result.data
}

onMounted(() => {
  void refresh()
  timer = setInterval(() => void refresh(), POLL_MS)
})

onUnmounted(() => {
  if (timer !== undefined) clearInterval(timer)
  if (noticeTimer !== undefined) clearTimeout(noticeTimer)
})

async function go(url: string): Promise<void> {
  const result = await safeInvoke(commands.browserNavigate(windowLabel, url))
  if (result.ok) {
    draft.value = null
    void refresh()
    return
  }

  if (result.error.code === IN_APP_BROWSER_INVALID_URL_CODE) {
    /*
     * Outside the host allowlist. The same answer the rest of the app
     * gives for this code — hand it to the system browser — rather than
     * a dead address bar the user has no explanation for.
     */
    say(t('inAppBrowser.fallbackToSystem'))
    const fallback = await safeInvoke(commands.openUrl(url))
    if (!fallback.ok) say(fallback.error.message)
    return
  }

  say(result.error.message || t('browserChrome.navigateFailed'))
}

function onSubmit(): void {
  const value = (draft.value ?? nav.value.url).trim()
  if (value !== '') void go(value)
}

async function openPanel(): Promise<void> {
  connection.value = null
  panelOpen.value = true
  await applyChromeHeight()
  const result = await safeInvoke(commands.browserConnectionInfo(windowLabel))
  connection.value = result.ok ? result.data : null
}

async function closePanel(): Promise<void> {
  panelOpen.value = false
  connection.value = null
  await applyChromeHeight()
}

function togglePanel(): void {
  void (panelOpen.value ? closePanel() : openPanel())
}
</script>

<template>
  <div class="browser-chrome">
    <div class="browser-chrome__bar" :style="{ height: `${BAR_HEIGHT}px` }">
      <button
        class="browser-chrome__btn"
        :disabled="!nav.canGoBack"
        :title="t('browserChrome.back')"
        @click="safeInvoke(commands.browserBack(windowLabel)).then(() => refresh())"
      >
        ‹
      </button>
      <button
        class="browser-chrome__btn"
        :disabled="!nav.canGoForward"
        :title="t('browserChrome.forward')"
        @click="safeInvoke(commands.browserForward(windowLabel)).then(() => refresh())"
      >
        ›
      </button>
      <button
        class="browser-chrome__btn"
        :title="t('browserChrome.reload')"
        @click="safeInvoke(commands.browserReload(windowLabel))"
      >
        ⟳
      </button>

      <!-- The padlock's state comes from the address alone; what the panel
           shows behind it takes a handshake, so it is fetched on demand. -->
      <button
        class="browser-chrome__btn"
        :title="t('browserChrome.connectionDetails')"
        @click="togglePanel"
      >
        {{ nav.url.startsWith('https://') ? '🔒' : '⚠' }}
      </button>

      <input
        class="browser-chrome__address"
        :value="draft ?? nav.url"
        spellcheck="false"
        :placeholder="t('browserChrome.addressPlaceholder')"
        @input="draft = ($event.target as HTMLInputElement).value"
        @focus="($event.target as HTMLInputElement).select()"
        @blur="draft = null"
        @keydown.enter="onSubmit"
        @keydown.esc="draft = null"
      />

      <button
        class="browser-chrome__btn"
        :title="t('browserChrome.openExternalHint')"
        @click="nav.url && safeInvoke(commands.openUrl(nav.url))"
      >
        ↗
      </button>
    </div>

    <div v-if="notice" class="browser-chrome__notice" :style="{ height: `${NOTICE_HEIGHT}px` }">
      {{ notice }}
    </div>

    <ConnectionPanel v-if="panelOpen" :info="connection" @close="closePanel" />
  </div>
</template>

<style scoped>
.browser-chrome {
  display: flex;
  flex-direction: column;
  width: 100%;
  height: 100%;
  overflow: hidden;
  background: var(--el-bg-color-page);
}

.browser-chrome__bar {
  display: flex;
  flex: 0 0 auto;
  align-items: center;
  gap: 4px;
  padding: 0 8px;
  border-bottom: 1px solid var(--el-border-color);
}

.browser-chrome__btn {
  display: flex;
  flex: 0 0 auto;
  align-items: center;
  justify-content: center;
  width: 28px;
  height: 28px;
  border: none;
  border-radius: 6px;
  background: transparent;
  font-size: 15px;
  line-height: 1;
  color: var(--el-text-color-primary);
  cursor: pointer;
}

.browser-chrome__btn:hover:not(:disabled) {
  background: var(--el-fill-color-light);
}

.browser-chrome__btn:disabled {
  opacity: 0.25;
  cursor: default;
}

.browser-chrome__address {
  flex: 1 1 auto;
  min-width: 0;
  height: 28px;
  padding: 0 10px;
  border: 1px solid var(--el-border-color);
  border-radius: 6px;
  background: var(--el-fill-color-blank);
  font-size: 12px;
  color: var(--el-text-color-primary);
  outline: none;
}

.browser-chrome__address:focus {
  border-color: var(--el-color-primary);
}

/* A row of its own: the window grew to make space for it, so it covers
   nothing. */
.browser-chrome__notice {
  display: flex;
  flex: 0 0 auto;
  align-items: center;
  overflow: hidden;
  padding: 0 12px;
  border-bottom: 1px solid var(--el-border-color);
  background: var(--el-fill-color-light);
  font-size: 11px;
  white-space: nowrap;
  text-overflow: ellipsis;
  color: var(--el-text-color-secondary);
}
</style>
