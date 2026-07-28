<script setup lang="ts">
/**
 * Login shell — the WPF `LoginPage.xaml` equivalent.
 *
 * The glass-panel IS the window body. Tauri runs with
 * `decorations: false` + `transparent: true` so only the rounded
 * card is visible. TitleBar provides drag + minimize / close.
 */

import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { useRoute, useRouter } from 'vue-router'
import { WebviewWindow } from '@tauri-apps/api/webviewWindow'
import { ElIcon } from 'element-plus'
import { Clock, InfoFilled, Promotion, Setting } from '@element-plus/icons-vue'
import TitleBar from '../components/TitleBar.vue'
import { ROUTE_NAMES } from '../router'
import { useConfigStore } from '../stores/config'
import { useUiStore } from '../stores/ui'
import type { LoginRegion } from '../types/bindings'

defineOptions({ name: 'LoginPage' })

const { t } = useI18n()
const route = useRoute()
const router = useRouter()
const config = useConfigStore()
const ui = useUiStore()

/**
 * MapleStory Classic (懷舊服) login mode toggle — mirrors MapleLink's
 * login-page game switcher. While on, a successful HK
 * account/password or TW GamaPass login launches Classic first, then
 * proceeds to the account list as usual. Persisted via
 * `Config.xml::classicLoginMode` (see `stores/ui.ts`).
 */
function toggleClassicMode(): void {
  void ui.setClassicLoginMode(!ui.classicLoginMode)
}

const currentRegion = computed(() => (config.get('loginRegion') as LoginRegion | undefined) ?? 'TW')

/** True when we're on the region picker itself — hide the switcher there. */
const isRegionPage = computed(() => route.name === 'login-region')

/**
 * The 懷舊服 toggle belongs to the account/password form only.
 *
 * It used to ride along on every login route, so a user on the QR page
 * saw a Classic button that does nothing there and read it as "QR can
 * start Classic" — it can't: HK Classic rides an account/password
 * session and TW Classic is a separate sign-in the toggle opens from
 * the id-pass form. Showing it beside a QR code only misleads.
 */
const showClassicToggle = computed(() => route.name === ROUTE_NAMES.LoginIdPass)

async function toggleRegion(): Promise<void> {
  const next: LoginRegion = currentRegion.value === 'TW' ? 'HK' : 'TW'
  await config.set('loginRegion', next)

  // Close any open GamePass window
  try {
    const gpWin = await WebviewWindow.getByLabel('gamepass-login')
    if (gpWin) await gpWin.close()
  } catch {
    /* Window may not exist — safe to ignore */
  }

  // Navigate back to id-pass so any active QR/GamePass session is abandoned
  if (route.path !== '/login/id-pass') {
    await router.replace('/login/id-pass')
  }
}

function handleOpenSettings(): void {
  void router.push('/settings')
}

function handleOpenAbout(): void {
  void router.push('/about')
}
</script>

<template>
  <section class="login-shell bf-glass-window" data-window-root>
    <TitleBar>
      <button
        v-if="showClassicToggle"
        type="button"
        class="login-shell__region-btn login-shell__classic-btn"
        :class="{ 'login-shell__classic-btn--active': ui.classicLoginMode }"
        :title="t('classic.modeToggleTitle')"
        data-test="login-classic-mode"
        @click="toggleClassicMode"
      >
        <el-icon class="login-shell__region-icon" aria-hidden="true"><Clock /></el-icon>
        <span class="login-shell__region-label">{{ t('classic.modeToggle') }}</span>
      </button>
      <button
        v-if="!isRegionPage"
        type="button"
        class="login-shell__region-btn"
        :title="`Region: ${currentRegion}`"
        @click="toggleRegion"
      >
        <el-icon class="login-shell__region-icon" aria-hidden="true"><Promotion /></el-icon>
        <span class="login-shell__region-label">{{ currentRegion }}</span>
      </button>
      <button type="button" class="login-shell__action-btn" @click="handleOpenSettings">
        <el-icon aria-hidden="true"><Setting /></el-icon>
      </button>
      <button type="button" class="login-shell__action-btn" @click="handleOpenAbout">
        <el-icon aria-hidden="true"><InfoFilled /></el-icon>
      </button>
    </TitleBar>
    <div class="login-shell__body">
      <!--
        Issue #236 (i18n follow-up): `data-window-content` lets the
        router's ResizeObserver track the rendered child form's
        natural height. `[data-window-root]` above is locked to
        `100vh` so observing it alone never fires on a language
        switch; the wrapper below grows/shrinks with the child form's
        own content and is what actually changes when the i18n store
        swaps strings. Keeping the wrapper as a pure flow container
        (no flex sizing of its own) means the child form's existing
        layout is unchanged.
      -->
      <div class="login-shell__content" data-window-content>
        <RouterView :key="currentRegion" />
      </div>
    </div>
  </section>
</template>

<style scoped>
.login-shell {
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.login-shell__body {
  flex: 1;
  overflow-y: auto;
  overflow-x: hidden;
  padding: 1.5rem;
}

.login-shell__action-btn {
  appearance: none;
  background: transparent;
  border: none;
  width: 28px;
  height: 28px;
  display: grid;
  place-items: center;
  border-radius: 6px;
  cursor: pointer;
  color: var(--bf-on-surface-variant, #54443a);
  transition: background 150ms ease;
  padding: 0;
}
.login-shell__action-btn .el-icon {
  font-size: 18px;
}
.login-shell__action-btn:hover {
  background: rgba(0, 0, 0, 0.06);
}

.login-shell__region-btn {
  appearance: none;
  background: color-mix(in srgb, var(--bf-primary-container, #ff8201) 15%, transparent);
  border: 1px solid color-mix(in srgb, var(--bf-primary-container, #ff8201) 30%, transparent);
  height: 26px;
  display: inline-flex;
  align-items: center;
  gap: 0.25rem;
  padding: 0 0.5rem 0 0.25rem;
  border-radius: 6px;
  cursor: pointer;
  color: var(--bf-primary, #954a00);
  font: inherit;
  font-size: 11px;
  font-weight: 700;
  transition: background 150ms ease;
}
.login-shell__region-icon {
  font-size: 14px;
}
.login-shell__region-btn:hover {
  background: color-mix(in srgb, var(--bf-primary-container, #ff8201) 25%, transparent);
}

/* Classic (懷舊服) mode toggle: muted until switched on, then it
   adopts the same filled-orange look as the region chip. */
.login-shell__classic-btn {
  background: transparent;
  border-color: color-mix(in srgb, var(--bf-on-surface-variant, #54443a) 30%, transparent);
  color: var(--bf-on-surface-variant, #54443a);
}

.login-shell__classic-btn:hover {
  background: color-mix(in srgb, var(--bf-on-surface-variant, #54443a) 12%, transparent);
}

.login-shell__classic-btn--active,
.login-shell__classic-btn--active:hover {
  background: color-mix(in srgb, var(--bf-primary-container, #ff8201) 25%, transparent);
  border-color: color-mix(in srgb, var(--bf-primary-container, #ff8201) 45%, transparent);
  color: var(--bf-primary, #954a00);
}
</style>
