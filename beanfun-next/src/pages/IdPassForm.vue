<script setup lang="ts">
/**
 * Regular (account + password) login form.
 *
 * # WPF parity
 *
 * Mirrors `Beanfun/Pages/id-pass_form.xaml(.cs)` for the **MVP**
 * controls. P12.1 D3 scope (per pre-flight Q1 = A) is the minimum
 * interactive form so the user can submit credentials and exercise
 * `auth.loginRegular`:
 *
 * - Account input → WPF `t_AccountID` (no autocomplete dropdown yet —
 *   account-history typeahead lands in P12.1 D9 / P12.2)
 * - Password input → WPF `t_Password` (PasswordBox) with show/hide
 *   toggle (Element Plus standard `show-password` affordance)
 * - Remember / AutoLogin checkboxes with the WPF coupling:
 *   - `Auto = true` ⇒ `Remember = true` (mirrors `checkBox_AutoLogin_Checked`)
 *   - `Remember = false` ⇒ `Auto = false` (mirrors `checkBox_RememberPWD_Unchecked`)
 * - Login button → submit → `auth.loginRegular(region, account, password)`
 *
 * # WPF deviations / deferrals (per Q1 = A)
 *
 * The following are deliberately **not** in D3 — see Todo.md P12.1
 * for which D-step lands each:
 *
 * - Account dropdown / typeahead / `name(account_id)` parsing /
 *   per-row delete button → D9 (page-level wiring + account store
 *   refactor)
 * - Auto-login bootstrap (autosubmit on mount when AutoLogin is set)
 *   → D9 (needs saved password from account store)
 * - "Remember" actually persisting password to Users.dat → D9
 *   (account store integration)
 * - Game icon (gbtn) + GameList dialog → P12.3
 * - Register / ForgotPassword / GameStart buttons → P12.4 (open
 *   WebBrowser)
 *
 * # D3 → D4 hotfix: navigation affordances
 *
 * After D4 landed the QR form at `/login/qr`, the live smoke test
 * surfaced that id-pass had zero navigation hooks — the user couldn't
 * (a) get back to the region picker nor (b) switch to QR login, so
 * `/login/qr` was unreachable from the UI. Added as a minimal D3 scope
 * patch (not a separate D-step entry) since both elements are part of
 * the WPF `id-pass_form.xaml` chrome:
 *
 * - Top-left "← 返回" link → `router.push('/login')` (SPA affordance;
 *   WPF had no equivalent because its region picker was a blocking
 *   dialog popped only on `LoadDataError`, not a routable page).
 * - Bottom "QR Code 便利登" link → `router.push('/login/qr')` — WPF
 *   parity for `btn_QRCode` (L736 `btn_QRCode_Click`); rendered as a
 *   text link rather than the 22×22 icon button because
 *   `@element-plus/icons-vue` ships no QR glyph and inlining an SVG
 *   path just for the tooltip is not worth the footprint.
 *
 * # D5 CP2: GamePass switch affordance
 *
 * D5 introduces `/login/gamepass` (GamepassForm). WPF
 * `id-pass_form.xaml` has `btn_GamePass` as a sibling icon of
 * `btn_QRCode` (L742-749) with `Visibility="Collapsed"` in the
 * legacy layout — the button was wired but hidden. We surface it
 * in the SPA as a parallel text link beside the QR switch so HK
 * users see both routes at the same visual hierarchy; a backend
 * region guard + an in-form preflight (see `GamepassForm.vue`)
 * keeps HK users from getting stuck on the GamePass-only flow.
 *
 * # Post-login navigation
 *
 * The auth store sets `pendingTotp` / `pendingVerify` flags when the
 * server signals a second factor. The form maps those to:
 *
 * | Auth store outcome             | router push target        | Lands in |
 * |--------------------------------|---------------------------|----------|
 * | `SessionInfo` returned (success) | `/accounts`             | P12.2   |
 * | `null` + `pendingTotp = true`   | `/login/totp`            | D6      |
 * | `null` + `pendingVerify = true` | `/login/verify`          | D8      |
 * | throws (any other error)        | stays on form (toasted)  | D3      |
 *
 * The push targets currently route through the catch-all back to
 * `/login` because D6/D8/P12.2 haven't shipped yet — that's the
 * documented Q2 = A trade-off. The wiring is correct; visual
 * verification waits for those D-steps.
 */

import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useRouter } from 'vue-router'
import { ElButton, ElCheckbox, ElForm, ElFormItem, ElIcon, ElInput, ElMessage } from 'element-plus'
import { ArrowLeft, Lock, User } from '@element-plus/icons-vue'

import { useAuthStore, AUTH_ACTIONS } from '../stores/auth'
import { useConfigStore } from '../stores/config'
import type { LoginRegion } from '../types/bindings'

defineOptions({ name: 'IdPassForm' })

const { t } = useI18n()
const router = useRouter()
const auth = useAuthStore()
const config = useConfigStore()

/**
 * Default the region from Config.xml so the picker → form handoff
 * survives a refresh. Falls back to `TW` (matches WPF
 * `App.LoginRegion` default in `App.xaml.cs`).
 */
function readRegion(): LoginRegion {
  const stored = config.get('loginRegion')
  return stored === 'HK' ? 'HK' : 'TW'
}

const account = ref('')
const password = ref('')
const remember = ref(false)
const autoLogin = ref(false)

/*
 * WPF coupling (`id-pass_form.xaml.cs` L29-37): toggling AutoLogin
 * implies Remember; unchecking Remember implies un-checking AutoLogin.
 * Express each direction as a watcher so user interaction in either
 * checkbox stays in sync with no extra branching in the click handlers.
 */
watch(autoLogin, (next) => {
  if (next) remember.value = true
})
watch(remember, (next) => {
  if (!next) autoLogin.value = false
})

const submitting = computed(() => auth.pendingAction === AUTH_ACTIONS.LoginRegular)

function goBack(): void {
  /*
   * Use explicit `push('/login')` instead of `router.back()` — browser
   * history may contain `/login/qr` or other siblings if the user
   * landed here via forward-nav, and "back" in the WPF sense always
   * means the region picker.
   */
  void router.push('/login')
}

function switchToQr(): void {
  void router.push('/login/qr')
}

function switchToGamepass(): void {
  void router.push('/login/gamepass')
}

async function submit(): Promise<void> {
  /*
   * Match WPF `btn_login_Click` validation: empty account / empty
   * password is a hard stop with a localized toast (WPF used a modal
   * MessageBox; ElMessage is the SPA equivalent that doesn't break
   * keyboard flow).
   */
  if (!account.value.trim()) {
    ElMessage.error(t('AccountNeed'))
    return
  }
  if (!password.value) {
    ElMessage.error(t('PasswordNeed'))
    return
  }

  try {
    const session = await auth.loginRegular(readRegion(), account.value.trim(), password.value)
    if (session) {
      // Full success — go to the post-login landing page (P12.2).
      await router.push('/accounts')
      return
    }
    /*
     * `null` means the auth store flipped a continuation flag
     * instead of throwing. Inspect both flags rather than assuming
     * mutual exclusion so a future server change that sets both
     * doesn't silently route to the wrong screen.
     */
    if (auth.pendingTotp) {
      await router.push('/login/totp')
      return
    }
    if (auth.pendingVerify) {
      await router.push('/login/verify')
      return
    }
  } catch {
    // The auth store already surfaced the error toast via
    // `surfaceCommandError`; staying on the form lets the user
    // correct the credentials and retry.
  }
}
</script>

<template>
  <el-form class="id-pass-form" label-position="top" @submit.prevent="submit">
    <button
      type="button"
      class="id-pass-form__back"
      :aria-label="t('Back')"
      data-test="id-pass-back"
      @click="goBack"
    >
      <el-icon><ArrowLeft /></el-icon>
      <span>{{ t('Back') }}</span>
    </button>

    <el-form-item :label="t('AcountOrEmail')" class="id-pass-form__item">
      <el-input
        v-model="account"
        size="large"
        autocomplete="username"
        :placeholder="t('AcountOrEmail')"
        clearable
      >
        <template #prefix>
          <el-icon><User /></el-icon>
        </template>
      </el-input>
    </el-form-item>

    <el-form-item :label="t('Password_')" class="id-pass-form__item">
      <el-input
        v-model="password"
        type="password"
        size="large"
        autocomplete="current-password"
        :placeholder="t('Password_')"
        show-password
      >
        <template #prefix>
          <el-icon><Lock /></el-icon>
        </template>
      </el-input>
    </el-form-item>

    <div class="id-pass-form__options">
      <el-checkbox v-model="remember" :label="t('RememberPassword')" />
      <el-checkbox v-model="autoLogin" :label="t('AutoLogin')" />
    </div>

    <el-button
      type="primary"
      size="large"
      class="id-pass-form__submit"
      native-type="submit"
      :loading="submitting"
    >
      {{ t('Login') }}
    </el-button>

    <div class="id-pass-form__switches">
      <button
        type="button"
        class="id-pass-form__switch"
        data-test="id-pass-switch-qr"
        @click="switchToQr"
      >
        {{ t('QRCodeLogin') }}
      </button>
      <button
        type="button"
        class="id-pass-form__switch"
        data-test="id-pass-switch-gamepass"
        @click="switchToGamepass"
      >
        {{ t('GamePassLogin') }}
      </button>
    </div>
  </el-form>
</template>

<style scoped>
.id-pass-form {
  display: flex;
  flex-direction: column;
  gap: 1rem;
}

.id-pass-form__back {
  align-self: flex-start;
  display: inline-flex;
  align-items: center;
  gap: 0.25rem;
  padding: 0.25rem 0.5rem;
  margin: -0.25rem 0 0 -0.5rem;
  border: 0;
  background: transparent;
  color: #54443a;
  font-size: 0.8125rem;
  font-weight: 600;
  cursor: pointer;
  border-radius: 0.25rem;
  transition:
    background-color 0.15s ease,
    color 0.15s ease;
}

.id-pass-form__back:hover,
.id-pass-form__back:focus-visible {
  background-color: rgba(84, 68, 58, 0.08);
  color: #2c1d14;
  outline: none;
}

.id-pass-form__item :deep(.el-form-item__label) {
  font-size: 0.8125rem;
  color: #54443a;
  font-weight: 600;
  margin-bottom: 0.25rem;
}

.id-pass-form__options {
  display: flex;
  align-items: center;
  gap: 1.25rem;
}

.id-pass-form__submit {
  width: 100%;
  margin-top: 0.5rem;
  font-weight: 700;
}

.id-pass-form__switches {
  display: flex;
  justify-content: center;
  align-items: center;
  gap: 1rem;
  margin-top: 0.25rem;
}

.id-pass-form__switch {
  padding: 0.25rem 0.5rem;
  border: 0;
  background: transparent;
  color: #a06a3a;
  font-size: 0.8125rem;
  font-weight: 600;
  cursor: pointer;
  text-decoration: underline;
  text-underline-offset: 0.15rem;
  transition: color 0.15s ease;
}

.id-pass-form__switch:hover,
.id-pass-form__switch:focus-visible {
  color: #7a4a20;
  outline: none;
}
</style>
