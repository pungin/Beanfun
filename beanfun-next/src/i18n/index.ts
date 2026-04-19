/**
 * vue-i18n bootstrap.
 *
 * # Message composition
 *
 * Each locale's runtime messages = generated WPF flat keys
 * (`src/locales/{locale}.json`) + frontend-only nested keys
 * ({@link FRONTEND_ONLY_MESSAGES}). The two sources are merged at
 * boot via plain object spread because their key namespaces never
 * collide:
 *
 * - WPF keys are flat strings (`AppName`, `Login`, `MsgDeleteAccount`).
 * - Frontend-only keys are nested under reserved namespaces
 *   (`placeholder.*`, `errors.*`, `themePreset.*`) — see
 *   `i18n/messages.ts` for the rationale.
 *
 * # Locale codes
 *
 * The single source of truth for the supported locale set lives in
 * `stores/ui.ts` (`AppLocale` / `SUPPORTED_LOCALES` / `DEFAULT_LOCALE`).
 * `scripts/convert-lang.mjs::LOCALE_FILE_MAP` mirrors it. When adding
 * a fourth locale (P12+), all three locations need a synchronized
 * update — the `i18n.spec.ts` key-consistency guard fails loudly if
 * any locale's key tree drifts.
 *
 * # Why `legacy: false` (Composition API mode)
 *
 * vue-i18n 11 defaults to legacy mode for backward compatibility,
 * but the new code is all `<script setup>` Composition API; turning
 * legacy off avoids the `globalInjection` shim and gives `useI18n()`
 * proper TypeScript inference for `t(key, args)` arguments.
 *
 * # Number / date formats
 *
 * Not configured in P11 — the Settings page will need them in P12
 * (e.g. `getRemainPoint` is rendered with thousands separators in
 * the WPF UI). Add via `numberFormats` / `datetimeFormats` then.
 */

import { createI18n } from 'vue-i18n'

import zhTwGenerated from '../locales/zh-TW.json'
import zhCnGenerated from '../locales/zh-CN.json'
import enUsGenerated from '../locales/en-US.json'

import { FRONTEND_ONLY_MESSAGES } from './messages'
import { DEFAULT_LOCALE, registerLocaleApplier, type AppLocale } from '../stores/ui'
import { registerErrorTranslator } from '../services/invoke'

/**
 * Build the merged locale messages map. Generated WPF keys are
 * spread first (lower priority); frontend-only keys win on the
 * unlikely chance of a collision (we control the namespace, the
 * upstream WPF translator does not).
 */
export const i18nMessages = {
  'zh-TW': { ...zhTwGenerated, ...FRONTEND_ONLY_MESSAGES['zh-TW'] },
  'zh-CN': { ...zhCnGenerated, ...FRONTEND_ONLY_MESSAGES['zh-CN'] },
  'en-US': { ...enUsGenerated, ...FRONTEND_ONLY_MESSAGES['en-US'] },
} as const

/**
 * Build the application's vue-i18n instance.
 *
 * Exposed as a factory so:
 *
 * - `main.ts` calls it once at boot,
 * - vitest specs (D10) can build a throw-away instance per case
 *   without bleeding state between tests.
 *
 * # Fallback chain (P12.5 D11 — WPF parity fix)
 *
 * `zh-CN` falls back to `zh-TW` first, then `en-US`. This mirrors
 * WPF `Beanfun/Helper/I18n.cs` exactly: the WPF `LoadLanguage`
 * routine builds a `MergedDictionaries` stack walking up the
 * culture parent chain (`zh-Hans → zh`), so any key missing from
 * `Beanfun/Lang/zh-Hans.xaml` (currently ~30 keys including
 * `Cancel`, `Yes`, `No`, `Tools`, `Weapon`, `ScrollBlack`,
 * `PerfectCoreNeedSkills`, …) gets resolved from `zh.xaml` at
 * runtime — Simplified Chinese WPF users see Traditional Chinese
 * for those keys, never English. The vue-i18n per-locale fallback
 * map gives the SPA the same two-layer chain (`zh-CN → zh-TW →
 * en-US`) so SC users never see an English string the WPF app
 * would have shown them in Chinese.
 *
 * Other locales keep the simple `'en-US'` fallback via the
 * `default` arm (a no-op for `zh-TW` itself which is already the
 * source of all WPF-generated Chinese strings, and the natural
 * fallback chain end for `en-US`).
 *
 * `missingWarn` / `fallbackWarn` are off in production to avoid
 * console spam from any WPF key that the en-US fallback dictionary
 * happens not to define yet.
 */
export function createAppI18n() {
  const isDev = typeof import.meta !== 'undefined' && Boolean(import.meta.env?.DEV)

  return createI18n({
    legacy: false,
    locale: DEFAULT_LOCALE,
    fallbackLocale: {
      'zh-CN': ['zh-TW', 'en-US'],
      default: ['en-US'],
    },
    messages: i18nMessages,
    missingWarn: isDev,
    fallbackWarn: isDev,
  })
}

/**
 * Imperatively switch locale on a running i18n instance. Used by
 * the UI store's `setLanguage` applier and by tests.
 */
export function setLocale(i18n: ReturnType<typeof createAppI18n>, locale: AppLocale): void {
  i18n.global.locale.value = locale
}

/**
 * Wire i18n into the rest of the app:
 *
 * 1. UI store's locale applier — so `setLanguage(...)` actually
 *    flips the visible locale.
 * 2. `services/invoke.ts` error translator — so command failures
 *    surface as localized `errors.{code}` strings instead of the
 *    raw English `error.message` from the backend.
 *
 * Called once from `main.ts` after both `createAppI18n()` and the
 * Pinia plugin have been installed.
 */
export function wireI18n(i18n: ReturnType<typeof createAppI18n>): void {
  registerLocaleApplier((locale: AppLocale) => {
    setLocale(i18n, locale)
  })

  registerErrorTranslator((key, fallback) => {
    if (!i18n.global.te(key)) return fallback
    const translated = i18n.global.t(key)
    return typeof translated === 'string' ? translated : fallback
  })
}
