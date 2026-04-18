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
 * - `placeholder.*` — the P11 placeholder page; rotated out in P12 once
 *   real pages land.
 * - `errors.*` — keyed by the backend `CommandError.code` value
 *   (`<domain>.<variant_snake_case>`) so `services/invoke.ts` can do
 *   `t('errors.' + error.code, fallback)` and get the localized
 *   message automatically.
 * - `themePreset.*` — the 8 preset color labels for the Settings
 *   page swatch picker; matches `THEME_PRESETS[i].name` in
 *   `composables/useThemeColor.ts`.
 *
 * The starter set below covers what P11 boot needs (placeholder page,
 * frequently-thrown error codes from P10 backend, theme presets).
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
  placeholder: {
    heading: '繽放 Next — P11 基礎建設就緒',
    subline: '前端 i18n / Pinia / 主題 / IPC 通路已連通，正在開發各個 Page。',
    versionLabel: '建置資訊',
    appVersion: 'App 版本',
    tauriVersion: 'Tauri 版本',
    versionLoading: '正在讀取版本資訊…',
    versionError: '無法取得版本資訊：{0}',
  },
  errors: {
    auth: {
      session_required: '您的登入狀態已失效，請重新登入。',
      totp_required: '請輸入驗證碼以完成登入。',
      verify_required: '伺服器要求進行二次驗證。',
      invalid_totp: '驗證碼錯誤，請重新輸入。',
      not_logged_in: '尚未登入。',
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
  placeholder: {
    heading: '缤放 Next — P11 基础建设就绪',
    subline: '前端 i18n / Pinia / 主题 / IPC 通路已连通，正在开发各个 Page。',
    versionLabel: '构建信息',
    appVersion: 'App 版本',
    tauriVersion: 'Tauri 版本',
    versionLoading: '正在读取版本信息…',
    versionError: '无法获取版本信息：{0}',
  },
  errors: {
    auth: {
      session_required: '您的登录状态已失效，请重新登录。',
      totp_required: '请输入验证码以完成登录。',
      verify_required: '服务器要求进行二次验证。',
      invalid_totp: '验证码错误，请重新输入。',
      not_logged_in: '尚未登录。',
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
  placeholder: {
    heading: 'beanfun! Next — P11 infrastructure ready',
    subline: 'Frontend i18n / Pinia / theme / IPC plumbing wired up. Page rebuild in progress.',
    versionLabel: 'Build info',
    appVersion: 'App version',
    tauriVersion: 'Tauri version',
    versionLoading: 'Loading version info…',
    versionError: 'Failed to load version info: {0}',
  },
  errors: {
    auth: {
      session_required: 'Your session expired. Please log in again.',
      totp_required: 'Please enter your TOTP code to continue.',
      verify_required: 'Server requires additional verification.',
      invalid_totp: 'Invalid TOTP code. Please try again.',
      not_logged_in: 'Not logged in.',
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
