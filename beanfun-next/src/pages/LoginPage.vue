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

defineOptions({ name: 'LoginPage' })

const { t } = useI18n()

const heading = computed(() => t('loginShell.heading'))
const subline = computed(() => t('loginShell.subline'))
</script>

<template>
  <main class="login-shell">
    <section class="login-shell__panel glass-panel">
      <header class="login-shell__header">
        <h1 class="login-shell__title">{{ heading }}</h1>
        <p class="login-shell__subline">{{ subline }}</p>
      </header>
      <div class="login-shell__body">
        <RouterView />
      </div>
    </section>
  </main>
</template>

<style scoped>
.login-shell {
  box-sizing: border-box;
  min-height: 100vh;
  display: grid;
  place-items: center;
  padding: 2rem;
  background:
    radial-gradient(
      1200px 800px at 10% -10%,
      color-mix(in srgb, var(--el-color-primary, #ff8201) 30%, transparent),
      transparent 60%
    ),
    radial-gradient(
      900px 700px at 110% 110%,
      color-mix(in srgb, var(--el-color-primary, #ff8201) 22%, transparent),
      transparent 55%
    ),
    linear-gradient(180deg, #f7f1ec 0%, #ece1d6 100%);
}

.login-shell__panel {
  width: 100%;
  max-width: 560px;
  border-radius: 12px;
  overflow: hidden;
}

.glass-panel {
  background: rgba(255, 255, 255, 0.55);
  backdrop-filter: blur(30px) saturate(1.4);
  -webkit-backdrop-filter: blur(30px) saturate(1.4);
  border: 1px solid rgba(255, 255, 255, 0.6);
  box-shadow:
    inset 0 1px 0 rgba(255, 255, 255, 0.7),
    0 10px 30px rgba(0, 0, 0, 0.08),
    0 2px 6px rgba(0, 0, 0, 0.04);
}

.login-shell__header {
  padding: 2rem 2rem 1rem;
  border-bottom: 1px solid rgba(255, 255, 255, 0.4);
  text-align: center;
}

.login-shell__title {
  margin: 0;
  font-size: 1.5rem;
  font-weight: 800;
  letter-spacing: -0.01em;
  color: #1f1a16;
}

.login-shell__subline {
  margin: 0.5rem 0 0;
  font-size: 0.875rem;
  color: #54443a;
}

.login-shell__body {
  padding: 2rem;
  min-height: 240px;
}
</style>
