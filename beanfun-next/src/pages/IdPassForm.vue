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
 *
 * # P12.4 followup-A: RegisterAccount / ForgotPassword / GameStart
 *
 * The three remaining WPF buttons land here in P12.4-followup-A
 * after the P12 acceptance gap surfaced them missing:
 *
 * - **RegisterAccount** (WPF `id-pass_form.xaml` L73 +
 *   `RegAcc_Click` L39-52) → opens region-aware signup URL via
 *   {@link useInAppBrowser}, which calls the backend
 *   `open_in_app_browser` IPC to spawn a fresh
 *   [`tauri::WebviewWindow`] with the logged-in `BeanfunClient`
 *   cookies pre-seeded — WPF parity for `new WebBrowser(uri)
 *   .Show()`. URLs outside the backend host allowlist fall back
 *   to `commands.openUrl` (system browser).
 * - **ForgotPassword** (WPF L627 + `FindPwd_Click` L54-66) →
 *   same composable, region-aware `forgot_pwd.aspx`.
 * - **GameStart** (WPF L655 + `btn_StartGame_Click` L297-300) →
 *   delegates to `useGameLauncher().runGame()`, which calls
 *   `game.restoreLastSelected()` to re-hydrate the launch
 *   subset from the persisted Config.xml snapshot (the post-
 *   logout LoginPage has no in-memory game store state). When
 *   the snapshot is absent / corrupt → `GameSelected` toast,
 *   matching WPF's behaviour when `service_code` is empty.
 *
 * URLs centralised in `src/constants/login.ts`
 * (`LOGIN_EXTERNAL_URLS`) so a Beanfun URL move tomorrow is a
 * one-line change. The in-app browser window itself is built by
 * the backend `web_browser::open_in_app_browser` command (no
 * frontend dialog mount needed — followup-B replaced the old
 * self-mounted `WebBrowser.vue` placeholder).
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

import { computed, onMounted, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useRouter } from 'vue-router'
import { ElButton, ElCheckbox, ElForm, ElFormItem, ElIcon, ElInput, ElMessage } from 'element-plus'
import { ArrowLeft, Lock, User } from '@element-plus/icons-vue'

import { useAccountStore } from '../stores/account'
import { useAuthStore, AUTH_ACTIONS, type LoginIntent } from '../stores/auth'
import { useConfigStore } from '../stores/config'
import { LOGIN_EXTERNAL_URLS, LOGIN_METHOD, type LoginExternalUrlKind } from '../constants/login'
import type { LoginRegion } from '../types/bindings'
import { useGameLauncher } from '../composables/useGameLauncher'
import { useInAppBrowser } from '../composables/useInAppBrowser'

defineOptions({ name: 'IdPassForm' })

/**
 * `Config.xml` key tracking the most recently logged-in account id.
 * Mirrors WPF `MainWindow.xaml.cs` L1340 / L1347
 * (`ConfigAppSettings.SetValue("AccountID", accountId)`) — used by
 * mount-time prefill (this file) and the WPF startup
 * `loginMethodInit` flow (re-implemented across the account store
 * + this prefill).
 */
const CONFIG_KEY_LAST_ACCOUNT_ID = 'AccountID'

const { t } = useI18n()
const router = useRouter()
const auth = useAuthStore()
const accountStore = useAccountStore()
const config = useConfigStore()
/*
 * P12.4 followup-A D7 — game-launcher composable for the
 * GameStart button (WPF `btn_StartGame_Click` /
 * `id-pass_form.xaml.cs` L297-300). The composable internally
 * calls `game.restoreLastSelected(config)` so the launch can
 * proceed even when the in-memory game store is empty (post-
 * logout LoginPage state). See `useGameLauncher` docblock for
 * the WPF parity table.
 */
const launcher = useGameLauncher()
/*
 * P12.4 followup-B B6 — in-app browser composable for the
 * RegisterAccount / ForgotPassword buttons (WPF
 * `id-pass_form.xaml.cs` `RegAcc_Click` / `FindPwd_Click` →
 * `new WebBrowser(uri).Show()`). The composable funnels through
 * the backend `open_in_app_browser` IPC which builds a fresh
 * `WebviewWindow` per call and pre-seeds the logged-in
 * `BeanfunClient` cookies; it falls back to `commands.openUrl`
 * (system browser) when the URL is outside the backend host
 * allowlist. See `useInAppBrowser` docblock for the full table.
 */
const inAppBrowser = useInAppBrowser()

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

/**
 * Mount-time prefill — replays WPF `loginMethodChanged`
 * (`MainWindow.xaml.cs` L1054-1092) which checks for a stored
 * record matching `(App.LoginRegion, t_AccountID.Text)` and
 * fills `t_Password.Password` / `checkBox_RememberPWD` /
 * `checkBox_AutoLogin` when the record carries a saved password.
 *
 * Differences from WPF:
 *
 * - WPF reads the account from `t_AccountID.Text`, which is
 *   pre-populated by `loginMethodInit` from `LastLoginAccountID`
 *   (Config) — we cut out the middle-man and read
 *   `LastLoginAccountID` directly. The visible behaviour is
 *   identical for the boot-into-id-pass case, which is the only
 *   path that exists in the SPA today (P12.2 D8 lands the
 *   ManageAccount-style account dropdown).
 * - VerifyPage prefill (WPF L1078-1091) is **not** done here —
 *   the SPA splits IdPassForm and VerifyPage into separate
 *   mounted components, so each owns its own prefill. See
 *   `VerifyPage.vue::onMounted`.
 *
 * `findStoredAccount` is a synchronous local-cache lookup (no
 * IPC); the cache is populated by `App.vue`'s boot
 * `account.loadAccounts()` step. Soft-fails to "no prefill" when
 * the cache hasn't loaded (boot in progress / load failed).
 */
onMounted(() => {
  prefillFromStoredRecord()
})

function prefillFromStoredRecord(): void {
  const region = readRegion()
  const lastAccountId = config.get(CONFIG_KEY_LAST_ACCOUNT_ID)
  if (!lastAccountId) return
  const stored = accountStore.findStoredAccount(region, lastAccountId)
  if (!stored) return

  account.value = stored.account_id
  /*
   * Only prefill password / remember / autoLogin when a non-empty
   * password was saved. WPF L1067 short-circuits on `pwd != "" &&
   * pwd != null`; matching keeps the form clean for users who
   * deliberately unchecked Remember last time (the empty
   * password row is still in `Users.dat` for `account_id`
   * round-trip but should not auto-fill anything).
   */
  if (stored.password) {
    password.value = stored.password
    remember.value = true
    autoLogin.value = stored.auto_login
  }
}

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

/* --------------- P12.4 followup-A: external URLs + GameStart --------------- */

/**
 * Generic external-URL handler. Single dispatch site for
 * RegisterAccount + ForgotPassword keeps the per-button click
 * stub a one-liner and centralises the
 * `LOGIN_EXTERNAL_URLS[kind][region]` lookup so adding a new
 * `kind` (e.g. "support" / "news") later touches only the
 * constants table + a new template button — no new handler
 * function needed.
 *
 * Delegates to {@link useInAppBrowser} for the open-with-fallback
 * chain (replaces the old self-mounted `WebBrowser.vue` dialog
 * that always degraded to system browser; followup-B reinstates
 * the WPF `new WebBrowser(uri).Show()` in-app behaviour).
 */
function openExternalUrl(kind: LoginExternalUrlKind): void {
  const region = readRegion()
  void inAppBrowser.open(LOGIN_EXTERNAL_URLS[kind][region])
}

function handleRegisterAccount(): void {
  openExternalUrl('register')
}

function handleForgotPassword(): void {
  openExternalUrl('forgotPwd')
}

/**
 * GameStart button — mirrors WPF `btn_StartGame_Click`
 * (`id-pass_form.xaml.cs` L297-300) which calls
 * `App.MainWnd.runGame()` with no credential args, relying on
 * the persisted `MainWindow` instance state (`service_code` /
 * `game_exe` / Settings `t_GamePath`) to drive the launch.
 *
 * The composable internally calls `restoreLastSelected` to
 * patch the empty post-logout game store from the persisted
 * Config.xml snapshot; absent / corrupt snapshot → `GameSelected`
 * toast (same UX WPF surfaces when `service_code` is empty
 * because the user never selected a game in this process).
 *
 * Fire-and-forget — the composable surfaces every failure path
 * via `wrapCommand` toast / inline ElMessage. Wrapping in
 * `try/catch` here would only double-toast.
 */
function handleGameStart(): void {
  void launcher.runGame()
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

  /*
   * Snapshot the form state into the auth store *before* firing
   * the IPC so that downstream login-flow pages (LoginTotp,
   * VerifyPage) can read it back regardless of which branch the
   * `loginRegular` call resolves on (success / pendingTotp /
   * pendingVerify / throw). See `auth.ts::LoginIntent` docblock
   * for the full lifecycle rationale — single-shot, overwrite
   * always, cleared on save / clearSession.
   */
  const intent: LoginIntent = {
    region: readRegion(),
    accountId: account.value.trim(),
    password: password.value,
    rememberPassword: remember.value,
    autoLogin: autoLogin.value,
  }
  auth.setLoginIntent(intent)

  try {
    const session = await auth.loginRegular(intent.region, intent.accountId, intent.password)
    if (session) {
      // Full success — persist credentials per WPF `OnLoginCompleted`
      // → `SaveLoginCredentials` (L1308-1313, L1334-1363) before
      // navigating to the post-login landing page.
      await persistAfterFullSuccess(intent)
      await router.push('/accounts')
      return
    }
    /*
     * `null` means the auth store flipped a continuation flag
     * instead of throwing. Inspect both flags rather than assuming
     * mutual exclusion so a future server change that sets both
     * doesn't silently route to the wrong screen.
     *
     * The `loginIntent` slot stays populated for the downstream
     * page to consume — the persistence call moves to that page's
     * success path (LoginTotp on TOTP success, IdPassForm second
     * mount on verify-then-id-pass).
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
    // correct the credentials and retry. We deliberately leave
    // `loginIntent` populated — a retry submission overwrites it
    // anyway, and clearing here would erase the user's password
    // before they can see the error toast.
  }
}

/**
 * Run the WPF `SaveLoginCredentials` (L1334-1363) sequence after
 * a fully-successful Regular login (no further pending flags):
 *
 * 1. Pull any stashed verify code from `auth.verifyIntent` (set
 *    by VerifyPage on the prior verify round-trip; absent for a
 *    first-pass success).
 * 2. Call `account.saveLoginCredentials` — atomic upsert with
 *    GamePass / QR skip rules, see action docblock.
 * 3. `config.set('AccountID', accountId)` so the next boot's
 *    prefill (and `LastLoginAccountID` consumers in P12.2 D8+)
 *    points at this row.
 * 4. Single-shot consume: clear both intent slots so a follow-up
 *    sign-in starts fresh.
 *
 * `method` is hard-pinned to `LOGIN_METHOD.Regular` because this
 * file owns the Regular flow (TOTP / QR / GamePass live in
 * sibling components, each with its own persist call site).
 *
 * Failures inside `saveLoginCredentials` / `config.set` already
 * surface a toast via `wrapCommand`; we still navigate because
 * the *login* itself succeeded — the persistence is a
 * convenience, not a blocker.
 */
async function persistAfterFullSuccess(intent: LoginIntent): Promise<void> {
  try {
    await accountStore.saveLoginCredentials({
      region: intent.region,
      accountId: intent.accountId,
      password: intent.password,
      rememberPassword: intent.rememberPassword,
      verify: auth.verifyIntent?.code ?? '',
      rememberVerify: auth.verifyIntent?.remember ?? false,
      method: LOGIN_METHOD.Regular,
      autoLogin: intent.autoLogin,
    })
    await config.set(CONFIG_KEY_LAST_ACCOUNT_ID, intent.accountId)
  } catch (err) {
    /*
     * `wrapCommand` inside the store already toasted; just log so
     * the dev console has context for why the next boot might not
     * prefill. Login itself is unaffected.
     */
    console.error('[IdPassForm] persistAfterFullSuccess failed', err)
  } finally {
    auth.clearLoginIntent()
    auth.clearVerifyIntent()
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
      <div class="id-pass-form__inline-links">
        <button
          type="button"
          class="id-pass-form__inline-link"
          data-test="id-pass-register"
          @click="handleRegisterAccount"
        >
          {{ t('RegisterAccount') }}
        </button>
        <button
          type="button"
          class="id-pass-form__inline-link"
          data-test="id-pass-forgot-password"
          @click="handleForgotPassword"
        >
          {{ t('ForgotPassword') }}
        </button>
      </div>
    </div>

    <div class="id-pass-form__primary-actions">
      <el-button
        type="primary"
        size="large"
        class="id-pass-form__submit"
        native-type="submit"
        :loading="submitting"
      >
        {{ t('Login') }}
      </el-button>
      <el-button
        size="large"
        class="id-pass-form__game-start"
        data-test="id-pass-game-start"
        @click="handleGameStart"
      >
        {{ t('GameStart') }}
      </el-button>
    </div>

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

.id-pass-form__inline-links {
  margin-left: auto;
  display: inline-flex;
  align-items: center;
  gap: 0.75rem;
}

.id-pass-form__inline-link {
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

.id-pass-form__inline-link:hover,
.id-pass-form__inline-link:focus-visible {
  color: #7a4a20;
  outline: none;
}

.id-pass-form__primary-actions {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 0.75rem;
  margin-top: 0.5rem;
}

.id-pass-form__submit,
.id-pass-form__game-start {
  width: 100%;
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
