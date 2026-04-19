<script setup lang="ts">
/**
 * In-app web browser dialog (P12.4 D8) — minimal skeleton that
 * defers actual page rendering to the user's system browser.
 *
 * # WPF parity (degraded — see decision below)
 *
 * Mirrors the surface of `Beanfun/Windows/WebBrowser.xaml(.cs)`:
 *
 * - WPF opens a `Window` (`Height=550 Width=850`) hosting a
 *   WebView2 control with cookies copied from the logged-in
 *   `bfClient` so that pages on `tw.beanfun.com` /
 *   `hk.beanfun.com` render with the user's session.
 * - The address bar (`t_URI`) shows the current navigation target
 *   (read-only — WPF used it as an indicator, not an editor).
 *
 * # P12.4 deviation: skip WebView2 + cookie injection
 *
 * The Tauri equivalent of WPF's WebView2 + cookie injection is a
 * dedicated `WebviewWindow` plus a backend cookie-sync command.
 * That requires:
 *
 * 1. A new Tauri-side `WebviewWindowBuilder` instance per call site
 *    (so each opened page has isolated history and the close button
 *    actually works).
 * 2. A backend command exposing the live `BeanfunClient` cookies
 *    over IPC, plus a permissions allowlist update so the new
 *    window is allowed to receive them.
 * 3. A pre-navigation hook to inject the cookies before the first
 *    request to the target host.
 *
 * P12.4's own surfaces (Settings / About) never open an in-app
 * browser. The real consumers are P12.5's MapleTools / KartTools
 * (which open `tw.beanfun.com/KartRider/...` Aspx pages requiring
 * an authenticated session) plus the AccountList's already-shipped
 * inline external-link buttons (which already call
 * `commands.openUrl` directly — no regression from this skeleton).
 *
 * Decision: ship a **placeholder skeleton** in P12.4 that always
 * routes to `commands.openUrl` (system browser); the user's default
 * browser already has whatever beanfun cookies were set there. The
 * full `WebviewWindow` + cookie sync lands in P13 if a P12.5
 * consumer turns out to actually need the embedded webview.
 *
 * # API
 *
 * Component is rendered as a child of any caller via `v-model`:
 *
 * ```vue
 * <WebBrowser v-model:visible="open" :url="url" />
 * ```
 *
 * Behaviour on open:
 *
 * - For URLs hosted on a domain we know requires the logged-in
 *   beanfun cookie ({@link URL_NEEDS_COOKIE_HOSTS}), the dialog
 *   skips the iframe attempt entirely and calls `commands.openUrl`
 *   immediately, surfacing a `webBrowser.cookieRequired` toast so
 *   the user knows why the page popped out into their default
 *   browser instead of the embedded surface. The dialog itself
 *   still appears so the user can copy the URL or re-open in the
 *   external browser.
 * - For other URLs the dialog shows an `<iframe>` for visual
 *   parity with WPF's WebView2 chrome, plus an "Open externally"
 *   button that re-routes through `commands.openUrl` (handy when
 *   the embedded host blocks framing via `X-Frame-Options`).
 *
 * # Why a `safeOpenUrl` wrapper instead of inline `await commands.openUrl`
 *
 * `commands.openUrl` rejects on the OS-level `ShellExecuteW` call
 * (e.g. no default browser configured). The wrapper centralises
 * the toast on failure — every call site otherwise duplicates the
 * same error-mapping branch.
 */

import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { ElButton, ElDialog, ElIcon, ElInput, ElMessage } from 'element-plus'
import { CircleClose, Link } from '@element-plus/icons-vue'

import { commands } from '../types/bindings'
import { safeInvoke } from '../services/invoke'

defineOptions({ name: 'WebBrowserDialog' })

const props = withDefaults(
  defineProps<{
    /**
     * Controls dialog visibility — typically driven by the parent
     * via `v-model:visible`. Mirrors the same prop convention every
     * other `windows/*.vue` dialog in the SPA uses.
     */
    visible: boolean
    /**
     * Target URL to display. Empty string is treated as a no-op
     * render (the dialog still mounts but the iframe / address bar
     * stay empty) so the parent can pre-mount the dialog and lazily
     * populate the URL after a backend round-trip.
     */
    url?: string
  }>(),
  {
    url: '',
  },
)

const emit = defineEmits<{
  (event: 'update:visible', value: boolean): void
}>()

const { t } = useI18n()

/**
 * Hosts known to require the logged-in beanfun cookie. Pages on
 * these hosts render their full content only when the request
 * carries the session cookie set during login. We can't satisfy
 * that constraint inside an iframe (cookies on these hosts are
 * `SameSite=Lax|Strict` and the SPA's WebView origin is `tauri://`
 * — the cookie jar is partitioned), so we shortcut straight to
 * `commands.openUrl` which uses the user's default browser (where
 * the cookie was likely set during a prior native browser login).
 *
 * Listed eagerly here rather than queried from the backend
 * `BeanfunClient` because:
 *
 * 1. The list is static (WPF hard-codes the same two hosts via
 *    `tw.beanfun.com` / `hk.beanfun.com` in every constructed URL).
 * 2. A backend round-trip per render would be wasteful for what is
 *    in effect a constant.
 * 3. P13's full WebviewWindow port will likely replace this entire
 *    component — adding a backend dependency now would be premature
 *    coupling that the rewrite would have to undo.
 */
const URL_NEEDS_COOKIE_HOSTS: ReadonlySet<string> = new Set(['tw.beanfun.com', 'hk.beanfun.com'])

const cookieRequired = computed<boolean>(() => {
  if (props.url === '') return false
  try {
    const host = new URL(props.url).host
    return URL_NEEDS_COOKIE_HOSTS.has(host)
  } catch {
    /*
     * `new URL(...)` throws on a malformed URL. Defensive false:
     * an invalid URL won't satisfy the `commands.openUrl` allowlist
     * either, so the open button will surface the real error to the
     * user when they click it instead of us silently mis-classifying.
     */
    return false
  }
})

async function safeOpenUrl(url: string): Promise<void> {
  if (url === '') return
  const result = await safeInvoke(commands.openUrl(url))
  if (!result.ok) {
    ElMessage.error(result.error.message)
  }
}

function handleClose(): void {
  emit('update:visible', false)
}

async function handleOpenExternally(): Promise<void> {
  await safeOpenUrl(props.url)
}

/**
 * On dialog open with a cookie-required URL, surface the toast and
 * pop the page out into the system browser immediately. Watching
 * `visible` (rather than running on mount) so the parent can
 * pre-mount with `visible: false` and the side effect only fires
 * when the user actually opens the dialog.
 */
function handleVisibleChange(value: boolean): void {
  if (!value) {
    emit('update:visible', false)
    return
  }
  emit('update:visible', true)
  if (cookieRequired.value && props.url !== '') {
    ElMessage.info(t('webBrowser.cookieRequired'))
    void safeOpenUrl(props.url)
  }
}
</script>

<template>
  <el-dialog
    :model-value="visible"
    :title="t('webBrowser.title')"
    width="850px"
    align-center
    destroy-on-close
    data-test="web-browser-dialog"
    @update:model-value="handleVisibleChange"
  >
    <div class="web-browser">
      <div class="web-browser__address-bar">
        <el-icon class="web-browser__address-icon"><Link /></el-icon>
        <el-input
          :model-value="url"
          readonly
          class="web-browser__address-input"
          data-test="web-browser-url"
        />
      </div>

      <!--
        For cookie-required hosts we don't even attempt the iframe —
        the placeholder explains why and the action button re-opens
        in the external browser. For other hosts we attempt the
        iframe; if framing is blocked the user still has the
        external-open affordance below.
      -->
      <div
        v-if="cookieRequired"
        class="web-browser__placeholder"
        data-test="web-browser-placeholder"
      >
        <p class="web-browser__placeholder-text">
          {{ t('webBrowser.cookieRequired') }}
        </p>
      </div>
      <iframe
        v-else-if="url !== ''"
        :src="url"
        class="web-browser__frame"
        data-test="web-browser-frame"
        sandbox="allow-same-origin allow-scripts allow-forms allow-popups"
        referrerpolicy="no-referrer-when-downgrade"
      />
      <div v-else class="web-browser__placeholder">
        <p class="web-browser__placeholder-text">
          {{ t('webBrowser.empty') }}
        </p>
      </div>
    </div>

    <template #footer>
      <el-button data-test="web-browser-close" @click="handleClose">
        <el-icon><CircleClose /></el-icon>
        <span>{{ t('Cancel') }}</span>
      </el-button>
      <el-button
        type="primary"
        :disabled="url === ''"
        data-test="web-browser-open-externally"
        @click="handleOpenExternally"
      >
        {{ t('webBrowser.openExternally') }}
      </el-button>
    </template>
  </el-dialog>
</template>

<style scoped>
.web-browser {
  display: flex;
  flex-direction: column;
  gap: 0.625rem;
  height: 480px;
}

.web-browser__address-bar {
  display: flex;
  align-items: center;
  gap: 0.375rem;
  flex-shrink: 0;
}

.web-browser__address-icon {
  color: var(--bf-on-surface-variant);
}

.web-browser__address-input {
  flex: 1;
}

.web-browser__frame {
  flex: 1;
  border: 1px solid color-mix(in srgb, var(--bf-outline-variant) 30%, transparent);
  border-radius: var(--bf-radius-button);
  background: var(--bf-surface);
}

.web-browser__placeholder {
  flex: 1;
  display: grid;
  place-items: center;
  border: 1px dashed color-mix(in srgb, var(--bf-outline-variant) 40%, transparent);
  border-radius: var(--bf-radius-button);
  padding: 1.5rem;
}

.web-browser__placeholder-text {
  margin: 0;
  text-align: center;
  font-size: 0.875rem;
  color: var(--bf-on-surface-variant);
  white-space: pre-line;
}
</style>
