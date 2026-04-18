<script setup lang="ts">
/**
 * "Logging in, please wait…" holding page.
 *
 * # WPF parity
 *
 * Mirrors `Beanfun/Pages/LoginWait.xaml(.cs)`. The WPF page is
 * deliberately minimal:
 *
 * - `t_Info` Label showing `MsgLogging`
 * - `btn` Cancel button whose click handler aborts the in-flight
 *   `loginWorker` / `totpWorker`, disables the `bfAPPAutoLogin` poll
 *   timer, resets `t_Info` back to `MsgLogging`, and sets
 *   `return_page = loginPage` so `NavigateLoginPage()` lands the user
 *   on the regular id-pass form.
 *
 * The SPA port preserves the user-visible contract (spinner + message
 * + Cancel → id-pass) while deferring the under-the-hood bits that
 * no longer map cleanly:
 *
 * | WPF                                          | SPA port                                       |
 * |----------------------------------------------|------------------------------------------------|
 * | `t_Info.Content = MsgLogging`                | `t('MsgLogging')` reuses the WPF locale JSON   |
 * | `loginWorker.CancelAsync()`                  | No-op — Tauri commands are atomic IPC, the     |
 * | `totpWorker.CancelAsync()`                   | auth store's `withGuard` already serializes    |
 * |                                              | actions; there's no background worker to abort |
 * | `bfAPPAutoLogin.IsEnabled = false`           | Deferred — the Beanfun-app 2FA polling flow    |
 * |                                              | has not been ported yet (later D-step)         |
 * | `App.LoginMethod == QRCode`                  | Deferred — QR-method state lives in `QrForm`   |
 * |                                              | and is torn down when that component unmounts  |
 * | `return_page = loginPage` (WPF id-pass form) | `router.push('/login/id-pass')`                |
 *
 * # Why this page exists in D7 even though no D1-D6 route navigates to it yet
 *
 * P12.1 D7 is the scaffolding step for this route. Mount points that
 * land here come online in later D-steps / P12.x:
 *
 * - D9 `AutoLogin` bootstrap mirrors WPF's `OnContentRendered` path
 *   (`do_Login()` → `frame.Content = loginWaitPage` → wait for worker
 *   completion → route on). Wiring it now means D9 is a pure
 *   "navigate from App.vue on mount" one-liner instead of a page
 *   creation + wiring combo change that would be harder to review.
 * - The future `bfAPPAutoLogin` poll (WPF `MsgNeedBeanfunAuth`) will
 *   mutate the same page's message. Keeping the message key reachable
 *   via the i18n key (not hard-coded) means that integration adds a
 *   prop / query param, not a template change.
 *
 * # Visual: CSS-only conic-gradient spinner
 *
 * Beanfun's mockup (`mockups/LoginWait.html`) uses a conic-gradient
 * spinner masked to a ring — same primitive here. Avoided pulling
 * `@element-plus/icons-vue::Loading` because it ships a 24×24 SVG
 * and styling it up to spinner size costs more CSS than hand-rolling
 * the animation.
 */

import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { useRouter } from 'vue-router'
import { ElButton } from 'element-plus'

defineOptions({ name: 'LoginWait' })

const { t } = useI18n()
const router = useRouter()

/*
 * The message is computed so a future `bfAPPAutoLogin` integration
 * can swap this for a prop / query-param-driven key (`MsgLogging` vs
 * `MsgNeedBeanfunAuth`) without a template rewrite. Today D7 ships
 * with the single WPF default message — matching `LoginWait.xaml`
 * initial state.
 */
const message = computed(() => t('MsgLogging'))

function cancel(): void {
  /*
   * WPF parity: `return_page = loginPage` lands the user on whatever
   * login form was active. The SPA condenses that to `/login/id-pass`
   * — the canonical entry form — because (a) there is no active
   * background worker that needs explicit aborting and (b) form-
   * specific local state (QR polling, GamePass WebView) is torn down
   * by the form's own unmount hook, not by the wait page.
   */
  void router.push('/login/id-pass')
}
</script>

<template>
  <section class="login-wait" aria-live="polite">
    <div class="login-wait__spinner" role="status" :aria-label="message" data-test="wait-spinner" />
    <p class="login-wait__message" data-test="wait-message">{{ message }}</p>
    <el-button class="login-wait__cancel" size="large" data-test="wait-cancel" @click="cancel">
      {{ t('Cancel') }}
    </el-button>
  </section>
</template>

<style scoped>
.login-wait {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 1.25rem;
  padding: 1rem 0;
}

.login-wait__spinner {
  position: relative;
  width: 72px;
  height: 72px;
  border-radius: 50%;
  background: conic-gradient(
    from 0deg,
    transparent 0%,
    color-mix(in srgb, var(--el-color-primary, #ff8201) 60%, transparent) 35%,
    var(--el-color-primary, #ff8201) 100%
  );
  mask: radial-gradient(circle 28px at center, transparent 99%, #000 100%);
  -webkit-mask: radial-gradient(circle 28px at center, transparent 99%, #000 100%);
  animation: login-wait-spin 1.2s linear infinite;
}

@keyframes login-wait-spin {
  to {
    transform: rotate(360deg);
  }
}

@media (prefers-reduced-motion: reduce) {
  .login-wait__spinner {
    animation-duration: 6s;
  }
}

.login-wait__message {
  margin: 0;
  font-size: 0.9375rem;
  font-weight: 600;
  color: #1f1a16;
  text-align: center;
}

.login-wait__cancel {
  min-width: 8rem;
}
</style>
