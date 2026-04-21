<script setup lang="ts">
/**
 * GamePass login form — WPF `Beanfun/Pages/gamepass_form.xaml(.cs)`
 * equivalent.
 *
 * # WPF parity
 *
 * The legacy form is a bare "Open GamePass / Cancel" dialog whose
 * `btn_OpenGamePass_Click` handler fetches a portal session key and
 * opens `GamePassBrowser.xaml` (embedded WebView2). We split the
 * flow across two Tauri pieces:
 *
 * - This page owns the user-facing progress tracker + navigation
 *   affordances (Cancel / Refresh), mirroring the WPF form chrome.
 * - The WebView window + cookie harvesting + completion-event
 *   plumbing lives on the backend (`open_gamepass_window` cmd +
 *   `gamepass-login-success` / `gamepass-login-failed` /
 *   `gamepass-login-cancelled` events).
 *
 * Unlike the WPF form, we auto-start the flow on mount rather than
 * waiting for an "Open" button click: a user who navigates to
 * `/login/gamepass` has already expressed intent by clicking
 * "GamePass 登入" on `IdPassForm`. The extra click survived in WPF
 * only because the legacy app swapped forms in-place within the same
 * Window; in an SPA the route change *is* the click.
 *
 * # Region pre-flight guard (mirrors QrForm)
 *
 * GamePass login is **TW-only** — backend returns
 * `auth.gamepass_unsupported_region` for HK. We short-circuit on
 * mount by reading the persisted `loginRegion` config key (same
 * source IdPassForm / QrForm use) so the HK guard renders as a toast
 * + redirect without a round-trip. The backend check is defence in
 * depth; it's reachable only if a hostile caller bypasses the UI.
 *
 * # Flow (P12.1 D5b CP4 — complete)
 *
 * ```
 *   mount → HK guard → register listeners (success/failed/cancelled)
 *         → loginGamepassStart (step 0 → 1)
 *         → openGamepassWindow   (step 1 → 2)
 *         → user OAuths in the WebView window
 *         → on  gamepass-login-success  → applyGamepassSession + nav /accounts
 *         → on  gamepass-login-failed   → windowError banner, step back to 1
 *         → on  gamepass-login-cancelled → step back to 0, silent (WPF parity)
 * ```
 *
 * Listeners are registered **before** `openGamepassWindow` fires so
 * an eager `gamepass-login-success` (fast path where the harvest
 * completes before `openGamepassWindow`'s Promise resolves) is not
 * missed.
 *
 * Step 3 ("authenticate done, complete in progress") is never set
 * explicitly — the backend has no intermediate event between "window
 * opened" and "bfWebToken harvested", so the tracker jumps straight
 * from 2 (WebView open, user authenticating) to 4 (complete). The
 * transient step-3 state would be visible for under a frame given
 * the atomic success emit → nav sequence, so there's nothing to
 * animate for it.
 *
 * # Error handling (matches QrForm Q11 = B)
 *
 * Two distinct inline banners, mutually exclusive by step:
 *
 * - `connectionLost` (step 0) — `loginGamepassStart` failed:
 *   network flake, backend 5xx, region mismatch. `wrapCommand` has
 *   already surfaced the i18n'd toast; the banner adds the
 *   "press Refresh" affordance.
 * - `windowError` (step ≥1) — `openGamepassWindow` failed OR a
 *   `gamepass-login-failed` event arrived (defensive surface for
 *   WebView cookie-harvest runtime regressions). Backend has
 *   toasted via `wrapCommand`; banner is the "press Refresh"
 *   affordance.
 *
 * Any non-`CommandInvocationError` throw (e.g. `withGuard`
 * "already in progress" on double-click) is benign concurrency —
 * silent is correct.
 *
 * Cancellation (user closes the WebView) is **silent** — matches
 * WPF `GamePassBrowser` which emits no dialog on the close button.
 * The tracker resets to 0 so the next Refresh starts from scratch.
 */

import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useRouter } from 'vue-router'
import { ElButton, ElMessage, ElStep, ElSteps } from 'element-plus'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'

import { AUTH_ACTIONS, useAuthStore } from '../stores/auth'
import { useConfigStore } from '../stores/config'
import { CommandInvocationError, wrapCommand } from '../services/invoke'
import { commands } from '../types/bindings'
import type { CommandError, LoginRegion, SessionInfo } from '../types/bindings'

/**
 * Tauri event names emitted by the backend GamePass flow. Kept as
 * literal constants (not imported from `bindings.ts`) because
 * `tauri-specta` doesn't model `app.emit(...)` events — only
 * commands have auto-generated bindings. The authoritative string
 * definitions live in `src-tauri/src/commands/auth.rs` (search for
 * `GAMEPASS_SUCCESS_EVENT` etc.); a drift between Rust and Vue
 * would silently turn the GamePass flow into a black hole, so the
 * CP4 spec pins these values via `data-testid` + event-name
 * assertions.
 */
const GAMEPASS_SUCCESS_EVENT = 'gamepass-login-success'
const GAMEPASS_FAILED_EVENT = 'gamepass-login-failed'
const GAMEPASS_CANCELLED_EVENT = 'gamepass-login-cancelled'

defineOptions({ name: 'GamepassForm' })

/**
 * Step markers — kept as named constants so the template's
 * `:active="step"` binding and event-driven advances reference the
 * same symbolic values (no magic numbers).
 *
 * Element Plus `<el-steps :active>` treats the value as "how many
 * steps are complete" — `0` = none done, `1` = step 1 complete, etc.
 *
 * Step 3 intentionally has no named constant because the flow jumps
 * from `STEP_WINDOW_OPENED` (2) to `STEP_COMPLETE` (4) on success —
 * the backend has no intermediate "authenticated but not yet
 * harvested" event, so we don't fabricate one. See module docblock.
 */
const STEP_INITIAL = 0
const STEP_PREPARED = 1
const STEP_WINDOW_OPENED = 2
const STEP_COMPLETE = 4

const { t } = useI18n()
const router = useRouter()
const auth = useAuthStore()
const config = useConfigStore()

/**
 * Progress tracker. Drives `<el-steps :active>`. See step constants
 * above for the legal values.
 */
const step = ref<number>(STEP_INITIAL)

/**
 * Inline "connection lost" banner flag — step 0 only. Flipped true
 * when the session-key preflight (`loginGamepassStart`) fails:
 * transport / parse / backend 5xx. Same pattern + same semantics as
 * `QrForm.connectionLost`.
 */
const connectionLost = ref(false)

/**
 * Inline "window error" banner flag — step ≥1. Flipped true when
 * either
 *
 * 1. `openGamepassWindow` returns `CommandError` (window builder
 *    failed, label collision with a stale window, etc.), or
 * 2. A `gamepass-login-failed` event arrives (WebView cookie
 *    harvest failed for every origin on a page load — defensive
 *    surface for Tauri runtime regressions).
 *
 * Reset on every `doStart` re-entry so Refresh always starts clean.
 */
const windowError = ref(false)

/**
 * Disposal sentinel. Set on route change / unmount to short-circuit
 * any in-flight `loginGamepassStart` continuation *and* any late
 * event callback firing after the user navigated away. Same
 * rationale as `QrForm.disposed` — kept as a plain `let` because we
 * only read it in async continuations / event callbacks, never in
 * the template.
 */
let disposed = false

/**
 * Handles returned by `listen(...)` calls, collected so
 * `onBeforeUnmount` can detach them before the component dies.
 * Leaking a listener would keep the closure alive past unmount and
 * fire against a destroyed Vue tree on the next matching event.
 */
const unlistenFns: UnlistenFn[] = []

/**
 * Match `IdPassForm.readRegion` / `QrForm.readRegion` verbatim so
 * region-source drift between the three forms is impossible.
 */
function readRegion(): LoginRegion {
  const stored = config.get('loginRegion')
  return stored === 'HK' ? 'HK' : 'TW'
}

/**
 * Subscribe to the three terminal events the backend emits during a
 * GamePass login attempt. Registered once in `onMounted` **before**
 * `doStart()` invokes `openGamepassWindow`, so even an eager
 * success (backend emit arrives before our command's Promise
 * resolves) is caught.
 *
 * Each listener is a no-op when `disposed` is true — event delivery
 * can race the route change on `/accounts` since `router.push` is
 * async.
 */
async function registerEventListeners(): Promise<void> {
  const successUnlisten = await listen<SessionInfo>(GAMEPASS_SUCCESS_EVENT, async (event) => {
    if (disposed) return
    step.value = STEP_COMPLETE
    auth.applyGamepassSession(event.payload)
    disposed = true
    await router.push('/accounts')
  })
  const failedUnlisten = await listen<CommandError>(GAMEPASS_FAILED_EVENT, (event) => {
    if (disposed) return
    // Debug-aid log — the backend already produces an i18n'd toast
    // via its own `wrapCommand` pipeline when the command-side
    // failure path fires, but the event-side failure path (cookie
    // harvest) doesn't go through `wrapCommand`, so the console
    // log is the one authoritative trail an operator has.
    console.error(`[gamepass-form] ${GAMEPASS_FAILED_EVENT}`, event.payload)
    windowError.value = true
    step.value = STEP_PREPARED
  })
  const cancelledUnlisten = await listen<null>(GAMEPASS_CANCELLED_EVENT, () => {
    if (disposed) return
    // WPF parity: silent cancel. Reset to the fresh-start state so
    // Refresh re-mints a session key rather than re-opening a
    // window on a stale one.
    step.value = STEP_INITIAL
    windowError.value = false
  })
  unlistenFns.push(successUnlisten, failedUnlisten, cancelledUnlisten)
}

async function doStart(): Promise<void> {
  if (disposed) return
  connectionLost.value = false
  windowError.value = false
  step.value = STEP_INITIAL
  try {
    await auth.loginGamepassStart(readRegion())
    if (disposed) return
    step.value = STEP_PREPARED
  } catch (e) {
    if (disposed) return
    if (e instanceof CommandInvocationError) {
      connectionLost.value = true
    }
    // Non-CommandInvocationError throws (e.g. withGuard "already in
    // progress" on double-click) are benign — fall through without
    // setting a banner.
    return
  }
  // `loginGamepassStart` succeeded → pending_gamepass slot is armed.
  // Now pop the WebView window; the rest of the flow advances via
  // `gamepass-login-*` events handled by `registerEventListeners`.
  try {
    await wrapCommand(commands.openGamepassWindow())
    if (disposed) return
    step.value = STEP_WINDOW_OPENED
  } catch (e) {
    if (disposed) return
    if (e instanceof CommandInvocationError) {
      windowError.value = true
    }
    // Leave step at STEP_PREPARED so the user sees "prepared but
    // window open failed"; Refresh re-runs `loginGamepassStart`
    // which also clears `pending_gamepass` backend-side.
  }
}

onMounted(async () => {
  const region = readRegion()
  if (region !== 'TW') {
    ElMessage.info(t('loginGamepass.unsupportedHK'))
    disposed = true
    await router.push({ path: '/login', query: { pick: '1' } })
    return
  }
  // Event listeners MUST register before `doStart` invokes
  // `openGamepassWindow` — a fast backend emit (harvest completes
  // before our command Promise resolves) would otherwise be dropped.
  try {
    await registerEventListeners()
  } catch (e) {
    console.error('[gamepass-form] failed to register tauri listeners', e)
    windowError.value = true
    return
  }
  if (disposed) return
  await doStart()
})

onBeforeUnmount(() => {
  disposed = true
  // Detach listeners so late events (after the user navigated away)
  // don't mutate a destroyed Vue tree. `UnlistenFn` is sync and
  // infallible in practice, but wrap each call defensively since a
  // throw here would abort the component teardown.
  for (const unlisten of unlistenFns) {
    try {
      unlisten()
    } catch (e) {
      console.error('[gamepass-form] unlisten threw', e)
    }
  }
  unlistenFns.length = 0
})

const isStarting = computed(() => auth.pendingAction === AUTH_ACTIONS.LoginGamepassStart)

async function refresh(): Promise<void> {
  await doStart()
}

async function goBack(): Promise<void> {
  disposed = true
  // "返回一般登入" — go back to the regular id-pass form within the
  // same saved region, NOT to the region picker. Pushing
  // `/login?pick=1` would force the user to re-pick TW/HK, which the
  // button label does not promise. Mirrors WPF's
  // `LoginMethod = Regular` + `loginMethodChanged()` flow.
  await router.push('/login/id-pass')
}
</script>

<template>
  <section class="gamepass-form">
    <header class="gamepass-form__header">
      <h3 class="gamepass-form__title">{{ t('loginGamepass.title') }}</h3>
      <p class="gamepass-form__subtitle">{{ t('loginGamepass.subtitle') }}</p>
    </header>

    <el-steps
      class="gamepass-form__steps"
      :active="step"
      align-center
      finish-status="success"
      data-testid="gamepass-steps"
    >
      <el-step :title="t('loginGamepass.steps.prepare')" />
      <el-step :title="t('loginGamepass.steps.openWindow')" />
      <el-step :title="t('loginGamepass.steps.authenticate')" />
      <el-step :title="t('loginGamepass.steps.complete')" />
    </el-steps>

    <p
      v-if="step === STEP_PREPARED && !connectionLost && !windowError"
      class="gamepass-form__status"
      data-testid="gamepass-status"
    >
      {{ t('loginGamepass.prepareDone') }}
    </p>

    <p v-if="connectionLost" class="gamepass-form__error" data-testid="gamepass-connection-lost">
      {{ t('loginGamepass.connectionLost') }}
    </p>

    <p v-if="windowError" class="gamepass-form__error" data-testid="gamepass-window-error">
      {{ t('loginGamepass.windowError') }}
    </p>

    <div class="gamepass-form__actions">
      <el-button
        class="gamepass-form__back"
        size="large"
        data-testid="gamepass-back"
        @click="goBack"
      >
        {{ t('BackRegularLogin') }}
      </el-button>
      <el-button
        class="gamepass-form__refresh"
        type="primary"
        size="large"
        :loading="isStarting"
        data-testid="gamepass-refresh"
        @click="refresh"
      >
        {{ t('loginGamepass.refresh') }}
      </el-button>
    </div>
  </section>
</template>

<style scoped>
.gamepass-form {
  display: flex;
  flex-direction: column;
  gap: 1rem;
  align-items: stretch;
}

.gamepass-form__header {
  text-align: center;
}

.gamepass-form__title {
  margin: 0;
  font-size: 1rem;
  font-weight: 700;
  color: #1f1a16;
}

.gamepass-form__subtitle {
  margin: 0.375rem 0 0;
  font-size: 0.8125rem;
  color: #54443a;
}

.gamepass-form__steps {
  margin: 0.5rem 0;
}

.gamepass-form__status {
  margin: 0;
  padding: 0.625rem 0.875rem;
  border-radius: 8px;
  background: color-mix(in srgb, var(--el-color-success, #67c23a) 12%, transparent);
  color: var(--el-color-success, #67c23a);
  font-size: 0.8125rem;
  text-align: center;
}

.gamepass-form__error {
  margin: 0;
  padding: 0.625rem 0.875rem;
  border-radius: 8px;
  background: color-mix(in srgb, var(--el-color-danger, #f56c6c) 14%, transparent);
  color: var(--el-color-danger, #f56c6c);
  font-size: 0.8125rem;
  text-align: center;
}

.gamepass-form__actions {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 0.75rem;
}

.gamepass-form__back,
.gamepass-form__refresh {
  width: 100%;
  font-weight: 700;
}
</style>
