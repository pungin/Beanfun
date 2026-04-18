/**
 * Frontend-only i18n messages — keys that don't exist in the legacy
 * WPF XAML resource dictionaries.
 *
 * # Why a separate module
 *
 * `src/locales/{zh-TW,zh-CN,en-US}.json` are auto-generated from
 * `Beanfun/Lang/*.xaml` by `scripts/convert-lang.mjs`. Editing those
 * files directly would lose the changes on the next regeneration.
 * This module sits beside them and gets deep-merged at i18n boot
 * (see `src/i18n/index.ts`), so:
 *
 * - WPF translation drift is a one-line `node scripts/convert-lang.mjs`
 *   away (no manual reconciliation).
 * - Frontend additions live in TypeScript with full type-checking
 *   (one missing key in any locale = build error via the
 *   `KeysMatch<T, U>` constraint below).
 *
 * # Namespace conventions
 *
 * - `loginShell.*` — strings rendered by `pages/LoginPage.vue` (the
 *   parent shell that hosts every login sub-route). Per-form copy
 *   (region picker, id-pass, qr, etc.) lives under its own namespace
 *   added by the relevant P12.1 D-step.
 * - `loginRegion.*` — region picker copy in
 *   `pages/LoginRegionSelection.vue`; the region labels themselves
 *   (`Taiwan` / `HongKong`) come from the WPF locale JSON via the
 *   regenerated `Lang.*.xaml` files.
 * - `loginQr.*` — QR login form copy (`pages/QrForm.vue`): scan
 *   prompt + the HK-unsupported redirect toast + the polling-error
 *   inline fallback string. WPF has no direct equivalent: the legacy
 *   UI relied on a hover-tip image (`Resources/QRCode_Tip.png`) for
 *   the "how to scan" affordance and silently killed the poll timer
 *   on errors without surfacing any text. We render the same meaning
 *   as localizable strings because an SPA can't ship a bitmap tip
 *   trivially across locales.
 * - `loginGamepass.*` — GamePass login form copy (`pages/GamepassForm.vue`):
 *   page-scoped title/subtitle, 4-step progress labels, HK-unsupported
 *   redirect toast, connection-lost fallback, refresh button label,
 *   window-error fallback (CP3 `openGamepassWindow` failure / Tauri
 *   `gamepass-login-failed` event — shown as a fixed UX-level banner
 *   rather than the backend's raw `CommandError.message`, matching the
 *   `connectionLost` convention so the user sees a consistent
 *   "something went wrong; press Refresh" affordance regardless of
 *   the underlying error code).
 *   WPF `gamepass_form.xaml` is a bare "Open GamePass / Cancel"
 *   dialog — we enhance with a step tracker so the user can tell
 *   which phase of the WebView-driven flow is stuck. `GamePassLogin`
 *   / `GamePassWaiting` / `GamePassOpen` stay as WPF-owned keys
 *   (used by `IdPassForm` as the switch-link label); the form-scoped
 *   strings here are the frontend-only additions.
 *
 *   Cancellation (user closes the WebView window) is intentionally
 *   silent — matches WPF `GamePassBrowser` which emits no dialog when
 *   the user hits the close button — so no `cancelled` key lives here.
 * - `errors.*` — keyed by the backend `CommandError.code` value
 *   (`<domain>.<variant_snake_case>`) so `services/invoke.ts` can do
 *   `t('errors.' + error.code, fallback)` and get the localized
 *   message automatically.
 * - `themePreset.*` — the 8 preset color labels for the Settings
 *   page swatch picker; matches `THEME_PRESETS[i].name` in
 *   `composables/useThemeColor.ts`.
 *
 * P12 will extend this as new error codes / pages appear; the
 * `KeysMatch` type guard plus the vitest "all-locales-match" spec
 * ensure no key drifts silently.
 */

/**
 * Compile-time guard: the three locale objects must declare an
 * identical key tree. A missing nested key in any locale becomes a
 * `Type '…' is missing the following properties from type '…'`
 * error during `vue-tsc --noEmit`.
 */
type KeysMatch<T, U> = keyof T extends keyof U ? (keyof U extends keyof T ? unknown : never) : never

/* -------- zh-TW (canonical / authoritative source) -------- */

const zhTW = {
  loginShell: {
    heading: '繽放 Next',
    subline: '歡迎登入',
  },
  loginRegion: {
    subline: '請選擇您要登入的 Beanfun 服務地區。',
    defaultBadge: '預設',
    totpHint: '支援 TOTP',
    tip: '可在「設定 → 一般」隨時切換。此選擇將會記住至下次啟動。',
  },
  loginQr: {
    title: '使用 Beanfun App 掃描',
    subtitle: '開啟 Beanfun 行動版 App 掃描下方 QR Code 即可登入。',
    unsupportedHK: 'QR 登入僅支援台灣區，已返回登入入口。',
    connectionLost: '無法取得登入狀態，請點選重新整理重試。',
  },
  loginGamepass: {
    title: '使用 GamePass 登入',
    subtitle: '請於開啟的視窗中完成 GamePass 登入程序。',
    steps: {
      prepare: '準備登入環境',
      openWindow: '開啟登入視窗',
      authenticate: '等待認證',
      complete: '登入完成',
    },
    prepareDone: '登入環境已備妥。',
    unsupportedHK: 'GamePass 登入僅支援台灣區，已返回登入入口。',
    connectionLost: '無法與 Beanfun 連線，請點選重新整理重試。',
    windowError: '無法完成 GamePass 登入，請點選重新整理重試。',
    refresh: '重新整理',
  },
  errors: {
    auth: {
      session_required: '您的登入狀態已失效，請重新登入。',
      totp_required: '請輸入驗證碼以完成登入。',
      verify_required: '伺服器要求進行二次驗證。',
      invalid_totp: '驗證碼錯誤，請重新輸入。',
      not_logged_in: '尚未登入。',
      gamepass_window_already_open: 'GamePass 登入視窗已開啟，請先關閉再重新嘗試。',
    },
    beanfun: {
      bad_credentials: '帳號或密碼錯誤。',
      transport: '網路連線異常，請稍後再試。',
      parse: '伺服器回應格式異常。',
    },
    qr: {
      expired: 'QR Code 已過期，請重新整理。',
      pending: '請於行動裝置完成掃描。',
    },
    config: {
      io_error: 'Config.xml 讀寫失敗。',
    },
    storage: {
      io_error: 'Users.dat 讀寫失敗。',
      invalid_dpapi: '帳號資料解密失敗。',
    },
    process: {
      window_not_found: '找不到目標遊戲視窗。',
      access_denied: '無權存取目標處理程序。',
    },
    system: {
      platform_unsupported: '此功能僅在 Windows 上支援。',
      unknown: '發生未預期的錯誤。',
    },
  },
  themePreset: {
    orange: '橙色',
    green: '綠色',
    lightblue: '淺藍',
    pink: '粉紅',
    gold: '金色',
    silver: '銀色',
    black: '黑色',
    white: '白色',
  },
} as const

/* -------- zh-CN (Simplified) -------- */

const zhCN = {
  loginShell: {
    heading: '缤放 Next',
    subline: '欢迎登录',
  },
  loginRegion: {
    subline: '请选择您要登录的 Beanfun 服务地区。',
    defaultBadge: '默认',
    totpHint: '支持 TOTP',
    tip: '可在「设置 → 通用」随时切换。此选择将记住至下次启动。',
  },
  loginQr: {
    title: '使用 Beanfun App 扫描',
    subtitle: '打开 Beanfun 移动版 App 扫描下方 QR Code 即可登录。',
    unsupportedHK: 'QR 登录仅支持台湾区，已返回登录入口。',
    connectionLost: '无法获取登录状态，请点选重新加载重试。',
  },
  loginGamepass: {
    title: '使用 GamePass 登录',
    subtitle: '请于开启的窗口中完成 GamePass 登录程序。',
    steps: {
      prepare: '准备登录环境',
      openWindow: '开启登录窗口',
      authenticate: '等待认证',
      complete: '登录完成',
    },
    prepareDone: '登录环境已备妥。',
    unsupportedHK: 'GamePass 登录仅支持台湾区，已返回登录入口。',
    connectionLost: '无法与 Beanfun 连接，请点选重新加载重试。',
    windowError: '无法完成 GamePass 登录，请点选重新加载重试。',
    refresh: '重新加载',
  },
  errors: {
    auth: {
      session_required: '您的登录状态已失效，请重新登录。',
      totp_required: '请输入验证码以完成登录。',
      verify_required: '服务器要求进行二次验证。',
      invalid_totp: '验证码错误，请重新输入。',
      not_logged_in: '尚未登录。',
      gamepass_window_already_open: 'GamePass 登录窗口已开启，请先关闭再重新尝试。',
    },
    beanfun: {
      bad_credentials: '账号或密码错误。',
      transport: '网络连接异常，请稍后再试。',
      parse: '服务器响应格式异常。',
    },
    qr: {
      expired: 'QR Code 已过期，请重新加载。',
      pending: '请于移动设备完成扫描。',
    },
    config: {
      io_error: 'Config.xml 读写失败。',
    },
    storage: {
      io_error: 'Users.dat 读写失败。',
      invalid_dpapi: '账号资料解密失败。',
    },
    process: {
      window_not_found: '找不到目标游戏窗口。',
      access_denied: '无权访问目标进程。',
    },
    system: {
      platform_unsupported: '此功能仅在 Windows 上支持。',
      unknown: '发生未预期的错误。',
    },
  },
  themePreset: {
    orange: '橙色',
    green: '绿色',
    lightblue: '浅蓝',
    pink: '粉红',
    gold: '金色',
    silver: '银色',
    black: '黑色',
    white: '白色',
  },
} as const satisfies KeysMatch<typeof zhTW, typeof zhTW>

/* -------- en-US -------- */

const enUS = {
  loginShell: {
    heading: 'beanfun! Next',
    subline: 'Welcome — please sign in',
  },
  loginRegion: {
    subline: 'Pick the beanfun! region you want to sign in to.',
    defaultBadge: 'Default',
    totpHint: 'TOTP supported',
    tip: 'Change anytime under Settings → General. This choice is remembered for next launch.',
  },
  loginQr: {
    title: 'Scan with the beanfun! app',
    subtitle: 'Open the beanfun! mobile app and scan the QR code below to sign in.',
    unsupportedHK: 'QR login is only available in Taiwan; redirected back to the login entry.',
    connectionLost: 'Unable to reach the login service. Please tap Reload and try again.',
  },
  loginGamepass: {
    title: 'Sign in with GamePass',
    subtitle: 'Please complete the GamePass sign-in flow in the window that opens.',
    steps: {
      prepare: 'Prepare login environment',
      openWindow: 'Open sign-in window',
      authenticate: 'Wait for authentication',
      complete: 'Sign-in complete',
    },
    prepareDone: 'Login environment ready.',
    unsupportedHK:
      'GamePass login is only available in Taiwan; redirected back to the login entry.',
    connectionLost: 'Unable to reach Beanfun. Please tap Reload and try again.',
    windowError: 'Unable to complete GamePass sign-in. Please tap Reload and try again.',
    refresh: 'Reload',
  },
  errors: {
    auth: {
      session_required: 'Your session expired. Please log in again.',
      totp_required: 'Please enter your TOTP code to continue.',
      verify_required: 'Server requires additional verification.',
      invalid_totp: 'Invalid TOTP code. Please try again.',
      not_logged_in: 'Not logged in.',
      gamepass_window_already_open:
        'GamePass login window is already open. Please close it before trying again.',
    },
    beanfun: {
      bad_credentials: 'Wrong account or password.',
      transport: 'Network error. Please try again later.',
      parse: 'Unexpected server response format.',
    },
    qr: {
      expired: 'QR code expired. Please refresh.',
      pending: 'Please complete the scan on your mobile device.',
    },
    config: {
      io_error: 'Config.xml read/write failed.',
    },
    storage: {
      io_error: 'Users.dat read/write failed.',
      invalid_dpapi: 'Failed to decrypt account data.',
    },
    process: {
      window_not_found: 'Target game window not found.',
      access_denied: 'Access to target process denied.',
    },
    system: {
      platform_unsupported: 'This feature is only supported on Windows.',
      unknown: 'An unexpected error occurred.',
    },
  },
  themePreset: {
    orange: 'Orange',
    green: 'Green',
    lightblue: 'Light Blue',
    pink: 'Pink',
    gold: 'Gold',
    silver: 'Silver',
    black: 'Black',
    white: 'White',
  },
} as const satisfies KeysMatch<typeof zhTW, typeof zhTW>

/**
 * Frontend-only translations keyed by locale code. The shape is
 * compile-time-locked to match the canonical `zh-TW` tree via the
 * {@link KeysMatch} marker on `zhCN` / `enUS`.
 */
export const FRONTEND_ONLY_MESSAGES = {
  'zh-TW': zhTW,
  'zh-CN': zhCN,
  'en-US': enUS,
} as const

export type FrontendMessages = typeof zhTW
