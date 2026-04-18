<script setup lang="ts">
/**
 * AdvanceCheck (CAPTCHA + extra-auth) verify page.
 *
 * # WPF parity
 *
 * Mirrors `Beanfun/Pages/VerifyPage.xaml(.cs)`. The WPF page renders:
 *
 * 1. `t_Verify` text box for the auth answer (e.g. last 4 of ID,
 *    email last 4, etc.) with a localized `AuthInfoNeed` placeholder.
 * 2. `checkBoxRememberVerify` to remember the answer for next time.
 * 3. `labelAuthType` ("提示您進階驗證資料為：" + server-supplied
 *    `lblAuthType` value) so the user knows which question they're
 *    answering.
 * 4. `t_Code` text box for the CAPTCHA characters with a
 *    `CaptchaCodeNeed` placeholder.
 * 5. `imageCaptcha` — clickable to refresh (`Button_Click_1` → re-fetch).
 * 6. "AuthConfirm" button → `Button_Click` → `verifyWorker.RunWorkerAsync()`.
 *
 * The Vue port preserves every observable behaviour:
 *
 * | WPF                                              | Vue                                                       |
 * |--------------------------------------------------|-----------------------------------------------------------|
 * | `t_Verify` placeholder `AuthInfoNeed`            | `ElInput v-model="verifyCode"` `:placeholder="t('AuthInfoNeed')"` |
 * | `t_Code` placeholder `CaptchaCodeNeed`           | `ElInput v-model="captchaCode"`  same key                |
 * | `imageCaptcha` clickable refresh                 | `<button @click="refreshCaptcha">` wraps the `<img>`      |
 * | `Button_Click` empty-input MessageBox + return  | `submit()` early-returns with `ElMessage.error(...)`      |
 * | `IsDefault="True"` Enter submits                 | `<el-form @submit.prevent="submit">` + `native-type=submit` |
 * | `verifyWorker_DoWork` outcome dispatch          | `submit()` switches on `VerifySubmit.result`              |
 * | success → `do_Login` re-runs `loginRegular`      | success → `router.push('/login/id-pass')` + success toast |
 * | wrong captcha → `WrongCaptcha` MessageBox + reload | toast + `refreshCaptcha()`                              |
 * | wrong auth info → `WrongAuthInfo` MessageBox + reload | toast + `refreshCaptcha()`                            |
 * | server alert (`alert('…')`) → MessageBox(msg)    | toast(msg) + `refreshCaptcha()`                           |
 * | `Image_MouseLeftButtonDown` (return_page=loginPage) | Top-left "Back" link → `/login/id-pass`               |
 *
 * # Why the success branch routes back to `/login/id-pass`
 *
 * P10.2 Q6 = A: backend never stashes the user's password across the
 * verify round-trip (no-secrets-over-IPC policy — see
 * `commands/state.rs::PendingVerify` rationale). A successful
 * `submitVerify` clears the server-side AdvanceCheck challenge but
 * does not auto-resume login; the SPA must drop the user back at the
 * id-pass form so they can re-enter credentials. The server-side
 * AdvanceCheck pass is tracked by IP/device fingerprint, not cookies,
 * so the second login attempt succeeds without another verify prompt.
 *
 * This is one extra password prompt vs. WPF's `do_Login()` (which
 * re-uses the cached credentials in-memory), but it removes a
 * plaintext-credentials window that the Tauri port deliberately does
 * not want. We surface a success toast on the redirect so the user
 * understands why they're back at the login form.
 *
 * # Remember-verify checkbox (UI-only for D8)
 *
 * `checkBoxRememberVerify` persists `t_Verify.Text` to `Config.xml`
 * via the WPF `Config.Remember_Verify` property (`MainWindow` L1357).
 * The Tauri `config` plugin doesn't yet expose a `verify_info` key,
 * and persisting auth answers needs a separate "is this safe" review
 * (the answer is often a national-ID fragment). D8 renders the
 * checkbox for parity but does **not** wire persistence — same
 * pattern as `IdPassForm`'s `RememberPassword` checkbox in D3, which
 * also rides on D9's account-store integration. The Vue spec
 * pins the UI-only behaviour so we don't accidentally rely on
 * persistence elsewhere.
 *
 * # Refresh strategy
 *
 * WPF refreshes the captcha image only — `samplecaptcha` (the
 * captcha id) stays the same; the server returns a different
 * rendering on each GET. We do the same: `auth.getVerifyCaptcha()`
 * re-fetches the image without re-running `getVerifyPageInfo`. The
 * captcha input field is cleared on refresh so a stale value
 * doesn't get auto-submitted.
 *
 * On full failure of `getVerifyPageInfo` (e.g. session expired,
 * AdvanceCheck.aspx returned bad HTML), `wrapCommand` already toasted
 * the backend error code; we flip an inline `loadFailed` banner so
 * the user has a "Retry" affordance without another duplicate toast.
 * Same UX shape as `QrForm.connectionLost`.
 */

import { computed, onMounted, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useRouter } from 'vue-router'
import { ElButton, ElCheckbox, ElForm, ElFormItem, ElIcon, ElInput, ElMessage } from 'element-plus'
import { ArrowLeft } from '@element-plus/icons-vue'

import { AUTH_ACTIONS, useAuthStore } from '../stores/auth'
import { CommandInvocationError } from '../services/invoke'

defineOptions({ name: 'VerifyPage' })

const { t } = useI18n()
const router = useRouter()
const auth = useAuthStore()

const verifyCode = ref('')
const captchaCode = ref('')
const remember = ref(false)
const lblAuthType = ref('')
const captchaImage = ref<string | null>(null)
const loadFailed = ref(false)

const submitting = computed(() => auth.pendingAction === AUTH_ACTIONS.SubmitVerify)
const refreshing = computed(
  () =>
    auth.pendingAction === AUTH_ACTIONS.GetVerifyPageInfo ||
    auth.pendingAction === AUTH_ACTIONS.GetVerifyCaptcha,
)

onMounted(() => {
  void bootstrap()
})

/**
 * Two-step boot: page-info → captcha image. WPF runs both inline in
 * `do_Login → ParseAccountLogin → getVerifyPageInfo → reLoadVerifyPage`
 * (`MainWindow.xaml.cs` L1483-1499). Splitting them in the SPA matches
 * the `auth.getVerifyPageInfo` / `auth.getVerifyCaptcha` IPC boundary
 * — page info is server-state-bearing (viewstate / form-action /
 * samplecaptcha), captcha is just bytes.
 */
async function bootstrap(): Promise<void> {
  loadFailed.value = false
  try {
    const page = await auth.getVerifyPageInfo(auth.advanceCheckUrl)
    lblAuthType.value = page.lbl_auth_type
    await loadCaptcha()
  } catch (error) {
    /*
     * Backend errors are already toasted via `wrapCommand`; the inline
     * banner gives the user a "Retry" affordance without re-toasting.
     * Any non-CommandInvocationError (Promise.reject from withGuard
     * concurrency, etc.) bubbles through the same banner so we never
     * leave the page in a half-rendered state.
     */
    void error
    loadFailed.value = true
  }
}

async function loadCaptcha(): Promise<void> {
  const captcha = await auth.getVerifyCaptcha()
  captchaImage.value = captcha.image_base64
}

/**
 * Refresh just the captcha bitmap — the captcha id (`samplecaptcha`)
 * stays the same on the backend `PendingVerify` slot, only the
 * rendering changes per GET. Mirrors WPF `Button_Click_1`
 * (`VerifyPage.xaml.cs` L40-44).
 */
async function refreshCaptcha(): Promise<void> {
  if (refreshing.value || submitting.value) return
  captchaCode.value = ''
  try {
    await loadCaptcha()
    loadFailed.value = false
  } catch (error) {
    void error
    captchaImage.value = null
    loadFailed.value = true
  }
}

/**
 * Retry the entire bootstrap (page-info + captcha) when the inline
 * banner is showing. Equivalent to the user backing out and re-
 * entering the page; we keep the route stable so the URL doesn't
 * flicker.
 */
async function retryBootstrap(): Promise<void> {
  if (refreshing.value || submitting.value) return
  await bootstrap()
}

async function submit(): Promise<void> {
  /*
   * WPF `Button_Click` (`VerifyPage.xaml.cs` L26-35) hard-stops on
   * empty inputs with localized MessageBox before kicking the worker.
   * We mirror with toast early-returns; the input fields stay focused
   * for retry.
   */
  if (!verifyCode.value.trim()) {
    ElMessage.error(t('MsgAuthInfoEmpty'))
    return
  }
  if (!captchaCode.value.trim()) {
    ElMessage.error(t('MsgCaptchaCodeEmpty'))
    return
  }
  if (submitting.value) return

  try {
    const outcome = await auth.submitVerify(verifyCode.value.trim(), captchaCode.value.trim())
    switch (outcome.result) {
      case 'success':
        ElMessage.success(t('loginVerify.success'))
        await router.push('/login/id-pass')
        return
      case 'wrong_captcha':
        ElMessage.error(t('WrongCaptcha'))
        await refreshCaptcha()
        return
      case 'wrong_auth_info':
        ElMessage.error(t('WrongAuthInfo'))
        await refreshCaptcha()
        return
      case 'server_message':
        /*
         * WPF surfaces the server-supplied alert text verbatim
         * (`MessageBox.Show(msg.Replace("\\n","\n").Replace("\\r","\r"))`,
         * `MainWindow.xaml.cs` L2659). Server messages are already
         * localized to the account's region and may name specific
         * email addresses or actions, so we render them as-is rather
         * than mapping to a UI-localized fallback.
         */
        ElMessage.error(outcome.message)
        await refreshCaptcha()
        return
    }
  } catch (error) {
    /*
     * `wrapCommand` already toasted the structured error. Refresh the
     * captcha so a follow-up retry has a fresh image — same recovery
     * shape as the explicit wrong_captcha branch.
     */
    if (error instanceof CommandInvocationError) {
      await refreshCaptcha()
      return
    }
    throw error
  }
}

function goBack(): void {
  void router.push('/login/id-pass')
}
</script>

<template>
  <el-form class="verify-page" label-position="top" @submit.prevent="submit">
    <button
      type="button"
      class="verify-page__back"
      :aria-label="t('Back')"
      data-test="verify-back"
      @click="goBack"
    >
      <el-icon><ArrowLeft /></el-icon>
      <span>{{ t('Back') }}</span>
    </button>

    <header class="verify-page__header">
      <h3 class="verify-page__title">{{ t('loginVerify.title') }}</h3>
      <p class="verify-page__subtitle">{{ t('loginVerify.subtitle') }}</p>
    </header>

    <p v-if="lblAuthType" class="verify-page__auth-type" data-test="verify-auth-type">
      <span class="verify-page__auth-tip">{{ t('YourAuthInfoTip') }}</span>
      <span class="verify-page__auth-label">{{ lblAuthType }}</span>
    </p>

    <el-form-item class="verify-page__item">
      <el-input
        v-model="verifyCode"
        size="large"
        :placeholder="t('AuthInfoNeed')"
        :disabled="loadFailed"
        autocomplete="one-time-code"
        data-test="verify-input"
      />
    </el-form-item>

    <div class="verify-page__remember">
      <el-checkbox v-model="remember" :label="t('Remember')" data-test="verify-remember" />
    </div>

    <div class="verify-page__captcha">
      <button
        type="button"
        class="verify-page__captcha-image"
        :title="t('RefreshCaptcha')"
        :aria-label="t('RefreshCaptcha')"
        :disabled="!captchaImage || refreshing || submitting"
        data-test="verify-captcha-image"
        @click="refreshCaptcha"
      >
        <img
          v-if="captchaImage"
          :src="captchaImage"
          :alt="t('RefreshCaptcha')"
          class="verify-page__captcha-bitmap"
          data-testid="verify-captcha-bitmap"
        />
        <div v-else class="verify-page__captcha-placeholder" />
      </button>
    </div>

    <el-form-item class="verify-page__item">
      <el-input
        v-model="captchaCode"
        size="large"
        :placeholder="t('CaptchaCodeNeed')"
        :disabled="loadFailed"
        autocomplete="off"
        data-test="verify-captcha-input"
      />
    </el-form-item>

    <p v-if="loadFailed" class="verify-page__error" data-test="verify-load-failed">
      {{ t('LoadCaptchaFailed') }}
    </p>

    <el-button
      v-if="loadFailed"
      class="verify-page__retry"
      size="large"
      data-test="verify-retry"
      :loading="refreshing"
      @click="retryBootstrap"
    >
      {{ t('RefreshCaptcha') }}
    </el-button>

    <el-button
      v-else
      type="primary"
      size="large"
      class="verify-page__submit"
      native-type="submit"
      data-test="verify-submit"
      :loading="submitting"
    >
      {{ t('AuthConfirm') }}
    </el-button>
  </el-form>
</template>

<style scoped>
.verify-page {
  display: flex;
  flex-direction: column;
  gap: 0.75rem;
}

.verify-page__back {
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

.verify-page__back:hover,
.verify-page__back:focus-visible {
  background-color: rgba(84, 68, 58, 0.08);
  color: #2c1d14;
  outline: none;
}

.verify-page__header {
  text-align: center;
}

.verify-page__title {
  margin: 0;
  font-size: 1rem;
  font-weight: 700;
  color: #1f1a16;
}

.verify-page__subtitle {
  margin: 0.375rem 0 0;
  font-size: 0.8125rem;
  color: #54443a;
}

.verify-page__auth-type {
  margin: 0;
  padding: 0.625rem 0.875rem;
  border-radius: 8px;
  background: color-mix(in srgb, var(--el-color-primary, #ff8201) 14%, transparent);
  font-size: 0.8125rem;
  color: #1f1a16;
  display: flex;
  flex-wrap: wrap;
  gap: 0.25rem 0.5rem;
}

.verify-page__auth-tip {
  font-weight: 500;
  color: #54443a;
}

.verify-page__auth-label {
  font-weight: 700;
}

.verify-page__remember {
  margin: -0.25rem 0;
}

.verify-page__captcha {
  display: flex;
  justify-content: center;
}

.verify-page__captcha-image {
  display: block;
  padding: 0.25rem;
  border: 1px solid rgba(0, 0, 0, 0.08);
  background: #ffffff;
  border-radius: 8px;
  cursor: pointer;
  transition:
    border-color 0.15s ease,
    box-shadow 0.15s ease;
}

.verify-page__captcha-image:hover:not(:disabled) {
  border-color: var(--el-color-primary, #ff8201);
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.08);
}

.verify-page__captcha-image:disabled {
  cursor: not-allowed;
  opacity: 0.65;
}

.verify-page__captcha-bitmap {
  display: block;
  width: 200px;
  height: 45px;
  object-fit: contain;
  image-rendering: pixelated;
}

.verify-page__captcha-placeholder {
  width: 200px;
  height: 45px;
  background: repeating-linear-gradient(
    45deg,
    rgba(0, 0, 0, 0.04),
    rgba(0, 0, 0, 0.04) 6px,
    transparent 6px,
    transparent 12px
  );
  border-radius: 4px;
}

.verify-page__error {
  margin: 0;
  padding: 0.625rem 0.875rem;
  border-radius: 8px;
  background: color-mix(in srgb, var(--el-color-danger, #f56c6c) 14%, transparent);
  color: var(--el-color-danger, #f56c6c);
  font-size: 0.8125rem;
  text-align: center;
}

.verify-page__submit,
.verify-page__retry {
  width: 100%;
  font-weight: 700;
  margin-top: 0.25rem;
}
</style>
