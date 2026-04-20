<script setup lang="ts">
/**
 * Login shell — the WPF `LoginPage.xaml` equivalent.
 *
 * # Why a shell instead of one big page?
 *
 * The WPF original is a near-empty `<Frame>` that hosts one of three
 * forms (`id_pass_form`, `qr_form`, `gamepass_form`) plus the modal
 * sub-flows (TOTP, captcha, wait spinner). Mirroring that structure
 * with `<RouterView />` keeps each form a self-contained route:
 *
 * - `/login/region`    — region picker (TW / HK), entry point
 * - `/login/id-pass`   — account+password form
 * - `/login/qr`        — QR-code login
 * - `/login/gamepass`  — GamePass login (opens Tauri WebviewWindow)
 * - `/login/totp`      — TOTP challenge
 * - `/login/wait`      — pending callback / handshake spinner
 * - `/login/verify`    — captcha + verify code
 *
 * Children are added route-by-route in P12.1 D2-D8; this D1 commit
 * only ships the shell + the empty `<RouterView />` slot so each
 * subsequent D-step has a stable mount point.
 *
 * # Title bar
 *
 * Currently relies on the OS-provided window decoration (Tauri
 * default). Migrating to a custom drag-region title bar (matching the
 * mockup's `title-bar` div) is a P12.4 concern — it touches every
 * page, not just login, so it lives in the Settings / shell-overhaul
 * D-step rather than here.
 */

import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import iconUrl from '../assets/icon-outline.png'

defineOptions({ name: 'LoginPage' })

const { t } = useI18n()

const heading = computed(() => t('loginShell.heading'))
const subline = computed(() => t('loginShell.subline'))
</script>

<template>
  <main class="login-shell bf-mica-bg">
    <section class="login-shell__panel bf-glass-panel">
      <header class="login-shell__header">
        <div class="login-shell__brand">
          <img :src="iconUrl" alt="Beanfun" class="login-shell__icon" />
          <h1 class="login-shell__title">{{ heading }}</h1>
        </div>
        <p class="login-shell__subline">{{ subline }}</p>
      </header>
      <div class="login-shell__body">
        <RouterView />
      </div>
    </section>
  </main>
</template>

<style scoped>
/*
 * P12.2 D1 refactor: page-level chrome (background gradient + glass
 * surface) moved to project-wide utility classes
 * (`bf-mica-bg`, `bf-glass-panel`) declared in `src/styles/utilities.css`.
 * Only LoginPage-specific layout (size/spacing) stays here so adding
 * a second top-level page (P12.2 AccountList) reuses the same
 * primitives instead of copy-pasting the rgba/blur soup.
 */
.login-shell {
  box-sizing: border-box;
  min-height: 100vh;
  display: grid;
  place-items: center;
  padding: 2rem;
}

.login-shell__panel {
  width: 100%;
  max-width: 560px;
  overflow: hidden;
}

.login-shell__header {
  padding: 2rem 2rem 1rem;
  border-bottom: 1px solid rgba(255, 255, 255, 0.4);
  text-align: center;
}

.login-shell__brand {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 0.5rem;
}

.login-shell__icon {
  height: 36px;
  width: auto;
  flex-shrink: 0;
}

.login-shell__title {
  margin: 0;
  font-size: 1.5rem;
  font-weight: 800;
  letter-spacing: -0.01em;
  color: var(--bf-on-surface);
}

.login-shell__subline {
  margin: 0.5rem 0 0;
  font-size: 0.875rem;
  color: var(--bf-on-surface-variant);
}

.login-shell__body {
  padding: 2rem;
  min-height: 240px;
}
</style>
