/**
 * UI store — bridges Config.xml (via {@link useConfigStore}) and the
 * UI-side concerns that don't belong on disk (modal stack, global
 * loading flag, etc.).
 *
 * # Scope (P11 Q4 = A: 4-store layout)
 *
 * Manages everything the user sees toggled in the Settings page that
 * isn't auth/account/game-specific:
 *
 * | UI getter           | Config.xml key                | Default     |
 * |---------------------|-------------------------------|-------------|
 * | `themeColor`        | `ThemeColor`                  | `#FF8201`   |
 * | `language`          | `Language`                    | `zh-TW`     |
 * | `minimizeToTray`    | `minimize_to_tray`            | `false`     |
 * | `disableHwAccel`    | `disableHardwareAcceleration` | `false`     |
 * | `updateChannel`     | `updateChannel`               | `Stable`    |
 *
 * The Config keys mirror the legacy WPF names verbatim so backend
 * `Config.xml` stays compatible with old installs (P10.2 design).
 *
 * # Apply hooks
 *
 * Theme color is applied via {@link useThemeColor.setPrimaryColor}
 * directly — no chicken-and-egg with Vue/Pinia init since the
 * composable is a pure function. Locale, however, requires a
 * vue-i18n instance which only exists after D10's `i18n/index.ts`
 * setup; the store therefore exposes
 * {@link registerLocaleApplier} for the boot wire-up to plug the
 * actual `i18n.global.locale.value = …` mutation in. Until a
 * locale applier is registered, `setLanguage` only writes to
 * Config.xml — the UI keeps the old locale visually until next
 * boot. This trade-off keeps the store testable in isolation
 * (D7) without depending on D10's i18n wiring.
 *
 * # Why not persist via `pinia-plugin-persistedstate` (P11 Q5 = B)
 *
 * The single source of truth is `Config.xml` on disk; the store
 * only caches what `useConfigStore.loadAll()` already loads at
 * boot. Persisting the same keys to localStorage would create a
 * sync conflict ("which copy wins on next launch?") for zero
 * observable speed-up.
 */

import { computed, ref } from 'vue'
import { defineStore } from 'pinia'

import { useConfigStore } from './config'
import { setPrimaryColor, DEFAULT_PRIMARY_COLOR } from '../composables/useThemeColor'

/** Locale codes the app supports — matches the three JSON files in `src/locales/`. */
export type AppLocale = 'zh-TW' | 'zh-CN' | 'en-US'
export const SUPPORTED_LOCALES: readonly AppLocale[] = ['zh-TW', 'zh-CN', 'en-US'] as const
export const DEFAULT_LOCALE: AppLocale = 'zh-TW'

/** Update channel literal — mirrors backend `Channel` enum. */
export type UpdateChannel = 'Stable' | 'Development'
export const DEFAULT_UPDATE_CHANNEL: UpdateChannel = 'Stable'

/**
 * Config keys touched by the UI store. Centralized so a future
 * rename (e.g. WPF schema migration) is a one-line edit.
 *
 * Matches the WPF `ConfigAppSettings.{Get,Set}Value(...)` callsites in
 * `Beanfun/MainWindow.xaml.cs` and `Beanfun/Pages/Settings.xaml.cs`.
 */
export const UI_CONFIG_KEYS = {
  ThemeColor: 'ThemeColor',
  Language: 'Language',
  MinimizeToTray: 'minimize_to_tray',
  DisableHardwareAcceleration: 'disableHardwareAcceleration',
  UpdateChannel: 'updateChannel',
} as const

type LocaleApplier = (locale: AppLocale) => void

let localeApplier: LocaleApplier | null = null

/**
 * Register the function that pushes a new locale into vue-i18n.
 * `main.ts` calls this once after constructing the i18n instance.
 *
 * Pass `null` to clear (mainly for tests).
 */
export function registerLocaleApplier(applier: LocaleApplier | null): void {
  localeApplier = applier
}

const isAppLocale = (value: string | undefined): value is AppLocale =>
  value !== undefined && (SUPPORTED_LOCALES as readonly string[]).includes(value)

const isUpdateChannel = (value: string | undefined): value is UpdateChannel =>
  value === 'Stable' || value === 'Development'

const parseBool = (value: string | undefined, fallback: boolean): boolean => {
  if (value === undefined) return fallback
  if (value === 'true' || value === 'True') return true
  if (value === 'false' || value === 'False') return false
  return fallback
}

const stringifyBool = (value: boolean): string => (value ? 'true' : 'false')

export const useUiStore = defineStore('ui', () => {
  const config = useConfigStore()

  /** Pure UI in-memory state (not persisted to Config.xml). */
  const globalLoading = ref(false)
  const currentDialog = ref<string | null>(null)

  /* ---------- reactive getters bound to config.entries ---------- */

  const themeColor = computed<string>(
    () => config.get(UI_CONFIG_KEYS.ThemeColor) ?? DEFAULT_PRIMARY_COLOR,
  )

  const language = computed<AppLocale>(() => {
    const raw = config.get(UI_CONFIG_KEYS.Language)
    return isAppLocale(raw) ? raw : DEFAULT_LOCALE
  })

  const minimizeToTray = computed<boolean>(() =>
    parseBool(config.get(UI_CONFIG_KEYS.MinimizeToTray), false),
  )

  const disableHwAccel = computed<boolean>(() =>
    parseBool(config.get(UI_CONFIG_KEYS.DisableHardwareAcceleration), false),
  )

  const updateChannel = computed<UpdateChannel>(() => {
    const raw = config.get(UI_CONFIG_KEYS.UpdateChannel)
    return isUpdateChannel(raw) ? raw : DEFAULT_UPDATE_CHANNEL
  })

  /* ---------- setters: write to Config.xml + apply side-effects ---------- */

  async function setThemeColor(hex: string): Promise<void> {
    await config.set(UI_CONFIG_KEYS.ThemeColor, hex)
    setPrimaryColor(hex)
  }

  async function setLanguage(locale: AppLocale): Promise<void> {
    await config.set(UI_CONFIG_KEYS.Language, locale)
    if (localeApplier) localeApplier(locale)
  }

  async function setMinimizeToTray(value: boolean): Promise<void> {
    await config.set(UI_CONFIG_KEYS.MinimizeToTray, stringifyBool(value))
  }

  async function setDisableHwAccel(value: boolean): Promise<void> {
    await config.set(UI_CONFIG_KEYS.DisableHardwareAcceleration, stringifyBool(value))
  }

  async function setUpdateChannel(value: UpdateChannel): Promise<void> {
    await config.set(UI_CONFIG_KEYS.UpdateChannel, value)
  }

  /* ---------- boot hook ---------- */

  /**
   * Apply every persisted UI setting once. Called by `App.vue`
   * `onMounted` after `config.loadAll()` resolves so the visible UI
   * matches the user's last-saved preferences from the very first
   * frame.
   *
   * Errors thrown by individual appliers (e.g. `setPrimaryColor`
   * with a corrupt hex) are caught and logged so a single bad
   * Config value can't soft-brick boot — fall back to defaults
   * for that one setting and continue.
   */
  function applyAll(): void {
    try {
      setPrimaryColor(themeColor.value)
    } catch (err) {
      console.error('[ui.applyAll] failed to apply themeColor; falling back to default', err)
      setPrimaryColor(DEFAULT_PRIMARY_COLOR)
    }

    if (localeApplier) {
      try {
        localeApplier(language.value)
      } catch (err) {
        console.error('[ui.applyAll] locale applier threw; keeping previous locale', err)
      }
    }
  }

  return {
    globalLoading,
    currentDialog,

    themeColor,
    language,
    minimizeToTray,
    disableHwAccel,
    updateChannel,

    setThemeColor,
    setLanguage,
    setMinimizeToTray,
    setDisableHwAccel,
    setUpdateChannel,

    applyAll,
  }
})

/**
 * Test-only: clear the locale applier registry between test cases
 * (Vitest module isolation does not reload `ui.ts` between `it`
 * blocks within the same file, so registrations leak otherwise).
 */
export function __resetUiAppliersForTesting(): void {
  localeApplier = null
}
