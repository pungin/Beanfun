<script setup lang="ts">
/**
 * QR-code login form — WPF `Beanfun/Pages/qr_form.xaml(.cs)` equivalent.
 *
 * # WPF parity
 *
 * Mirrors the three core interactions of the legacy form:
 *
 * - **QR display + 2s polling**: `MainWindow.xaml.cs` wires a
 *   `DispatcherTimer` (`qrCheckLogin`, L112) at a 2-second interval
 *   (L161 `TimeSpan.FromSeconds(2)`) and calls
 *   `BeanfunClient.QRCodeCheckLoginStatus` each tick. We port the
 *   cadence verbatim via {@link QR_POLL_INTERVAL_MS}.
 * - **Refresh button**: `qr_form.xaml.cs::btn_Refresh_QRCode_Click`
 *   calls `App.MainWnd.refreshQRCode()`. We call
 *   `auth.loginQrStart(region)` — backend issues a fresh client + new
 *   QR challenge, overwriting any prior `pending_qr` slot (see
 *   `commands/auth.rs::login_qr_start` side-effect docs).
 * - **Back button**: `qr_form.xaml.cs::btn_back_Click` flips
 *   `App.LoginMethod = Regular` and re-runs `loginMethodChanged()`,
 *   which lands the user on the regular id-pass form (the QR mode is
 *   peer to id-pass within the same region, not above region picking).
 *   We mirror that with `router.push('/login/id-pass')` — staying in
 *   the saved region, just switching the login mode. We do NOT push
 *   `/login?pick=1`; the button is "返回一般登入" (back to regular
 *   login), not "重選區域" (re-pick region).
 *
 * # Polling strategy (Q2 + Q10)
 *
 * `setTimeout`-recursive scheduling rather than `setInterval`:
 * `setInterval` fires on a fixed wall-clock, so a slow round-trip
 * (LAN hiccup, debug pause) can queue multiple overlapping ticks that
 * all hit `auth.loginQrCheck` — which rejects concurrent calls via
 * `withGuard`. The recursive pattern runs one tick at a time:
 * `await check → schedule next tick`. Zero overlap by construction,
 * no try/catch gymnastics.
 *
 * # Error handling (Q11)
 *
 * The auth store's `loginQrCheck` returns `SafeResult<QrStatus>`
 * instead of throwing — on a transport / parse failure the polling
 * loop halts and shows an inline "connection lost" message. WPF did
 * the same silently (`qrCheckLogin_Tick` L2358-2359 disables the
 * timer for any `res != 0`; no MessageBox). The inline fallback is
 * our SPA equivalent of "timer stopped"; the user hits Refresh to
 * restart the flow.
 *
 * A `loginQrStart` failure (network flake, backend 5xx) surfaces its
 * own toast via `wrapCommand`, and we flip the same inline flag so
 * the user has a clear recovery path without another toast on top.
 *
 * # Region pre-flight guard (Q5)
 *
 * QR login is **TW-only** — backend returns `auth.qr_unsupported_region`
 * for HK. We short-circuit on mount by reading the persisted
 * `loginRegion` config key (same source IdPassForm / RegionSelection
 * use) so we don't round-trip just to find out; HK → info toast +
 * redirect to `/login`.
 *
 * # Deeplink copy (Q6)
 *
 * WPF `btn_CopyDeeplink_Click` pops a MessageBox if `deeplink` is
 * null. Modern UX: the button is disabled when there's no deeplink
 * so the affordance is zero-click obvious. Otherwise
 * `navigator.clipboard.writeText` + success toast; clipboard API
 * failure → `CopyFailed` toast (same resource key WPF used).
 *
 * # P12.4 followup-A: GameStart button
 *
 * WPF `qr_form.xaml.cs::btn_StartGame_Click` (L84-87) is a
 * 3-line `App.MainWnd.runGame()` call — same parity surface as
 * `id-pass_form.xaml.cs` L297-300. The SPA delegates to the
 * shared `useGameLauncher` composable so QR + IdPass both go
 * through the same restoration + launch chain. Snapshot
 * absent → `GameSelected` toast (`useGameLauncher` internal),
 * matching WPF's behaviour when `service_code` is empty.
 */

import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useRouter } from 'vue-router'
import { ElButton, ElMessage } from 'element-plus'

import { AUTH_ACTIONS, useAuthStore } from '../stores/auth'
import { useConfigStore } from '../stores/config'
import { CommandInvocationError } from '../services/invoke'
import type { LoginRegion } from '../types/bindings'
import { useGameLauncher } from '../composables/useGameLauncher'

defineOptions({ name: 'QrForm' })

/**
 * Polling cadence — mirrors WPF `MainWindow.InitializeComponent`
 * L161 (`qrCheckLogin.Interval = TimeSpan.FromSeconds(2)`). Keep
 * this as a module-level `const` rather than a configurable prop:
 * the cadence is a server-side rate-limit concern (beanfun portal
 * throttles `QRLogin/CheckLoginStatus` aggressively below 1s) so
 * per-instance overrides would be a footgun.
 */
const QR_POLL_INTERVAL_MS = 2000

const { t } = useI18n()
const router = useRouter()
const auth = useAuthStore()
const config = useConfigStore()
/*
 * P12.4 followup-A D8 — GameStart parity (WPF
 * `qr_form.xaml.cs::btn_StartGame_Click` L84-87). Composable
 * does its own `restoreLastSelected(config)` so this file
 * doesn't have to know about the persistence shape.
 */
const launcher = useGameLauncher()

/**
 * Inline "connection lost" banner flag. Flipped true when the poll
 * loop hits an error (either `loginQrStart` throw or
 * `loginQrCheck` safe-result error) and reset false whenever we
 * re-enter `doStart`. The user recovers by hitting Refresh.
 */
const connectionLost = ref(false)

/**
 * Disposal sentinel. Set on route change / unmount to short-circuit
 * any in-flight async work that would otherwise mutate the new
 * route's state (e.g. a late `loginQrCheck` resolving while the user
 * is already on `/login` triggering a bogus `router.push`).
 *
 * Kept as a plain `let` (no reactivity needed): we only read it in
 * async continuations, never in the template.
 */
let disposed = false

/**
 * Handle to the currently-scheduled next-tick timer. `null` when no
 * poll is in flight or scheduled — the recursive loop sets this at
 * schedule time and nulls it again at tick fire, so
 * {@link clearPollTimer} is idempotent.
 */
let pollTimeoutId: ReturnType<typeof setTimeout> | null = null

/**
 * Match `IdPassForm.readRegion` verbatim so region-source drift
 * between the two forms is impossible: if the picker + id-pass pair
 * sees region `X`, QR sees the same `X`.
 */
function readRegion(): LoginRegion {
  const stored = config.get('loginRegion')
  return stored === 'HK' ? 'HK' : 'TW'
}

function clearPollTimer(): void {
  if (pollTimeoutId !== null) {
    clearTimeout(pollTimeoutId)
    pollTimeoutId = null
  }
}

function schedulePoll(): void {
  if (disposed) return
  clearPollTimer()
  pollTimeoutId = setTimeout(runPollTick, QR_POLL_INTERVAL_MS)
}

/**
 * One polling round-trip. Structured as `await check → branch → (schedule | stop)`
 * so only one `loginQrCheck` is ever in flight at a time.
 */
async function runPollTick(): Promise<void> {
  pollTimeoutId = null
  if (disposed) return

  const result = await auth.loginQrCheck()
  if (disposed) return

  if (!result.ok) {
    /*
     * Dev-side diagnostic (zero UX impact): `safeInvoke` path
     * deliberately skips the `[invoke]` console.error that
     * `wrapCommand` would emit (see `services/invoke.ts::surfaceCommandError`),
     * so without this line the error code + details vanish into
     * the inline banner and there's no way to distinguish
     * `auth.qr_json_parse_failed` / `auth.server_rejected` /
     * `network.http_failed` from the UI alone. Kept in
     * production builds too — console.error is cheap and the
     * info is strictly useful when a user reports "紅框出來了".
     */
    console.error('[qr-form] loginQrCheck failed', result.error)
    connectionLost.value = true
    return
  }

  const status = result.data
  if (status.status === 'approved') {
    disposed = true
    await router.push('/accounts')
    return
  }
  if (status.status === 'expired') {
    await doStart()
    return
  }
  schedulePoll()
}

/**
 * Mint (or re-mint) the QR challenge and kick off the polling loop.
 *
 * Called from three sites: initial mount, user-driven refresh, and
 * automatic refresh on `expired` (WPF `qrCheckLogin_Tick` L2364-2367
 * calls `refreshQRCode()` on `res == -2`).
 */
async function doStart(): Promise<void> {
  if (disposed) return
  clearPollTimer()
  connectionLost.value = false
  try {
    await auth.loginQrStart(readRegion())
    if (disposed) return
    schedulePoll()
  } catch (e) {
    if (disposed) return
    /*
     * `CommandInvocationError` — backend refused the start (network /
     * region mismatch / parse). The toast already fired inside
     * `wrapCommand`; we flip the inline flag so the user has a clear
     * "press Refresh to retry" target without a second overlapping
     * toast.
     *
     * Any other `Error` (e.g. `withGuard` "already in progress" when
     * the user double-clicks Refresh) is a benign concurrency signal
     * — the first call is still running and will populate the
     * challenge. Silent is correct here.
     */
    if (e instanceof CommandInvocationError) {
      connectionLost.value = true
    }
  }
}

onMounted(async () => {
  const region = readRegion()
  if (region !== 'TW') {
    ElMessage.info(t('loginQr.unsupportedHK'))
    disposed = true
    await router.push({ path: '/login', query: { pick: '1' } })
    return
  }
  await doStart()
})

onBeforeUnmount(() => {
  disposed = true
  clearPollTimer()
})

const bitmap = computed(() => auth.qrChallenge?.bitmap_base64 ?? null)
const deeplink = computed(() => auth.qrChallenge?.deeplink ?? null)
const showEnlarged = ref(false)
const canCopyDeeplink = computed(() => {
  const link = deeplink.value
  return typeof link === 'string' && link.length > 0
})

const isStarting = computed(() => auth.pendingAction === AUTH_ACTIONS.LoginQrStart)

async function copyQrImage(): Promise<void> {
  const dataUrl = bitmap.value
  if (!dataUrl) return
  try {
    const res = await fetch(dataUrl)
    const blob = await res.blob()
    await navigator.clipboard.write([new ClipboardItem({ [blob.type]: blob })])
    ElMessage.success(t('loginQr.copyQrSuccess'))
  } catch {
    ElMessage.error(t('CopyFailed'))
  }
}

async function copyDeeplink(): Promise<void> {
  const link = deeplink.value
  if (!link) {
    /*
     * Defensive: `canCopyDeeplink` should have disabled the button
     * already. Keeping the WPF `CopyDeeplinkNotReady` toast here as a
     * fallback in case the button-disabled path is bypassed (e.g.
     * programmatic trigger during a race window where the auth store
     * clears `qrChallenge`).
     */
    ElMessage.error(t('CopyDeeplinkNotReady'))
    return
  }
  try {
    await navigator.clipboard.writeText(link)
    ElMessage.success(t('CopyDeeplinkSuccess'))
  } catch {
    ElMessage.error(t('CopyFailed'))
  }
}

async function refresh(): Promise<void> {
  await doStart()
}

async function goBack(): Promise<void> {
  disposed = true
  clearPollTimer()
  await router.push('/login/id-pass')
}

/**
 * GameStart button — fires the shared launcher chain. See file
 * docblock "P12.4 followup-A: GameStart button" for the WPF
 * parity rationale; the composable handles snapshot
 * restoration + the empty-state `GameSelected` toast on its
 * own, so this handler is a one-liner.
 *
 * Fire-and-forget — every failure path is toasted inside the
 * composable / its IPC `wrapCommand` calls.
 */
function handleGameStart(): void {
  void launcher.runGame()
}
</script>

<template>
  <section class="qr-form">
    <header class="qr-form__header">
      <h3 class="qr-form__title">{{ t('loginQr.title') }}</h3>
      <p class="qr-form__subtitle">{{ t('loginQr.subtitle') }}</p>
    </header>

    <div class="qr-form__display" :data-loading="isStarting && !bitmap">
      <img
        v-if="bitmap"
        :src="bitmap"
        :alt="t('loginQr.title')"
        class="qr-form__bitmap"
        data-testid="qr-bitmap"
        @contextmenu.stop.prevent="copyQrImage"
      />
      <div v-else class="qr-form__placeholder" />
    </div>

    <div v-if="bitmap" class="qr-form__qr-actions">
      <button
        type="button"
        class="qr-form__qr-btn"
        data-testid="qr-enlarge"
        @click="showEnlarged = true"
      >
        <span class="material-symbols-outlined">zoom_in</span>
        {{ t('loginQr.enlarge') }}
      </button>
      <button
        type="button"
        class="qr-form__qr-btn"
        data-testid="qr-copy-image"
        @click="copyQrImage"
      >
        <span class="material-symbols-outlined">content_copy</span>
        {{ t('loginQr.copyQr') }}
      </button>
    </div>

    <!-- Enlarged QR dialog -->
    <div v-if="showEnlarged" class="qr-form__overlay" @click="showEnlarged = false">
      <div class="qr-form__enlarged" @click.stop>
        <img :src="bitmap!" :alt="t('loginQr.title')" class="qr-form__enlarged-img" />
        <button type="button" class="qr-form__enlarged-close" @click="showEnlarged = false">
          <span class="material-symbols-outlined">close</span>
        </button>
      </div>
    </div>

    <p v-if="connectionLost" class="qr-form__error" data-testid="qr-connection-lost">
      {{ t('loginQr.connectionLost') }}
    </p>

    <el-button
      class="qr-form__deeplink"
      size="default"
      :disabled="!canCopyDeeplink"
      data-testid="qr-copy-deeplink"
      @click="copyDeeplink"
    >
      {{ t('CopyDeeplink') }}
    </el-button>

    <div class="qr-form__actions">
      <el-button class="qr-form__back" size="default" data-testid="qr-back" @click="goBack">
        {{ t('BackRegularLogin') }}
      </el-button>
      <el-button
        class="qr-form__refresh"
        type="primary"
        size="default"
        :loading="isStarting"
        data-testid="qr-refresh"
        @click="refresh"
      >
        {{ t('RefreshQRCode') }}
      </el-button>
    </div>

    <el-button
      class="qr-form__game-start"
      size="default"
      data-testid="qr-game-start"
      @click="handleGameStart"
    >
      {{ t('GameStart') }}
    </el-button>
  </section>
</template>

<style scoped>
.qr-form {
  display: flex;
  flex-direction: column;
  gap: 1rem;
  align-items: stretch;
  box-sizing: border-box;
  max-width: 100%;
  overflow: hidden;
}

.qr-form__header {
  text-align: center;
}

.qr-form__title {
  margin: 0;
  font-size: 1rem;
  font-weight: 700;
  /*
   * Issue #272 — was hardcoded `#1f1a16` (the light-mode WPF dark
   * brown). In dark mode the page background flips to ~#1c1b1f
   * but this dark-on-dark text became unreadable. Route through
   * `--bf-on-surface` so the token bridge in `design-tokens.css`
   * (`#221a11` light / `#e6e1e5` dark) handles the contrast flip
   * for free, matching every other glass-panel heading.
   */
  color: var(--bf-on-surface);
}

.qr-form__subtitle {
  margin: 0.375rem 0 0;
  font-size: 0.8125rem;
  /*
   * Issue #272 — same fix as the title. The literal `#54443a`
   * was the light-mode WPF "muted body copy" color; in dark
   * mode it would render as a barely-visible smudge against the
   * dark glass background. `--bf-on-surface-variant` flips to
   * `#c9c5ca` in dark mode, mirroring the contrast tier the
   * mockups use for secondary text.
   */
  color: var(--bf-on-surface-variant);
}

.qr-form__display {
  align-self: center;
  width: 220px;
  height: 220px;
  display: grid;
  place-items: center;
  padding: 0.75rem;
  background: #ffffff;
  border-radius: 12px;
  box-shadow:
    inset 0 0 0 1px rgba(0, 0, 0, 0.05),
    0 6px 16px rgba(0, 0, 0, 0.06);
}

.qr-form__bitmap {
  width: 100%;
  height: 100%;
  image-rendering: pixelated;
  display: block;
}

.qr-form__placeholder {
  width: 100%;
  height: 100%;
  background: repeating-linear-gradient(
    45deg,
    rgba(0, 0, 0, 0.04),
    rgba(0, 0, 0, 0.04) 6px,
    transparent 6px,
    transparent 12px
  );
  border-radius: 8px;
}

.qr-form__error {
  margin: 0;
  padding: 0.625rem 0.875rem;
  border-radius: 8px;
  background: color-mix(in srgb, var(--el-color-danger, #f56c6c) 14%, transparent);
  color: var(--el-color-danger, #f56c6c);
  font-size: 0.8125rem;
  text-align: center;
}

.qr-form__deeplink {
  width: 100%;
  font-weight: 600;
}

.qr-form__actions {
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
  gap: 0.75rem;
}

.qr-form__actions :deep(.el-button) {
  width: 100%;
  margin-left: 0;
}

.qr-form__back,
.qr-form__refresh {
  font-weight: 700;
}

.qr-form__game-start {
  width: 100%;
  font-weight: 700;
}

.qr-form__qr-actions {
  display: flex;
  justify-content: center;
  gap: 0.75rem;
}

.qr-form__qr-btn {
  appearance: none;
  background: rgba(255, 255, 255, 0.6);
  border: 1px solid rgba(0, 0, 0, 0.08);
  border-radius: 8px;
  padding: 0.375rem 0.75rem;
  display: inline-flex;
  align-items: center;
  gap: 0.375rem;
  cursor: pointer;
  font: inherit;
  font-size: 0.75rem;
  font-weight: 500;
  color: var(--bf-on-surface-variant, #54443a);
  transition: background 150ms ease;
}
.qr-form__qr-btn .material-symbols-outlined {
  font-size: 16px;
}
.qr-form__qr-btn:hover {
  background: rgba(255, 255, 255, 0.9);
}

.qr-form__overlay {
  position: fixed;
  inset: 0;
  z-index: 10000;
  background: rgba(0, 0, 0, 0.5);
  display: grid;
  place-items: center;
}

.qr-form__enlarged {
  position: relative;
  background: #fff;
  border-radius: 16px;
  padding: 1.5rem;
  box-shadow: 0 20px 60px rgba(0, 0, 0, 0.3);
}

.qr-form__enlarged-img {
  width: 360px;
  height: 360px;
  image-rendering: pixelated;
  display: block;
}

.qr-form__enlarged-close {
  position: absolute;
  top: 0.5rem;
  right: 0.5rem;
  appearance: none;
  background: rgba(0, 0, 0, 0.06);
  border: none;
  width: 32px;
  height: 32px;
  border-radius: 50%;
  display: grid;
  place-items: center;
  cursor: pointer;
  color: #333;
  transition: background 150ms ease;
}
.qr-form__enlarged-close:hover {
  background: rgba(0, 0, 0, 0.12);
}
</style>
