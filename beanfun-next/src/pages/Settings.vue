<script setup lang="ts">
/**
 * Settings page — global app preferences + per-game launcher
 * preferences (P12.4 D3 / D4 / D5 / D6).
 *
 * # WPF parity
 *
 * Mirrors `Beanfun/Pages/Settings.xaml(.cs)` 1:1:
 *
 * | WPF section / control                          | SPA equivalent                                       |
 * |------------------------------------------------|------------------------------------------------------|
 * | "AppName" section header (App-scoped)          | `<section class="settings__section">` + section header |
 * | `ManageAccount` button                         | `el-button` → `router.push('/manage-account')`       |
 * | `cb_UpdateChannel` (Stable / Development)      | `el-select` writing `'Stable' \| 'Beta'` (D3 typo fix) |
 * | `cb_Language` (zh-Hant / zh-Hans / en)         | `el-select` writing `'zh-TW' \| 'zh-CN' \| 'en-US'`   |
 * | `cb_ThemeColor` (free-form hex + WPF presets)  | `el-input` + `el-color-picker` (free-form hex; P11 preset swatches) |
 * | `LoginModePanel` (TW-only Regular / QrCode)    | `el-select` `v-if="region === 'TW'"`                 |
 * | `ask_update` checkbox                          | `el-checkbox` bound to `useUiStore.askUpdate`        |
 * | `autoStartGame` checkbox                       | `el-checkbox` bound to `useUiStore.autoStartGame`    |
 * | `minimize_to_tray` checkbox                    | `el-checkbox` bound to `useUiStore.minimizeToTray`   |
 * | `disableHardwareAcceleration` checkbox         | `el-checkbox` + `ElMessageBox.alert` on toggle (WPF L217-222) |
 * | "Game" section header                          | `<section v-if="game.selectedGame">`                 |
 * | `t_GamePath` `TextBox` (read-only + click)     | `el-input readonly` + click handler → `pickGamePath` |
 * | `tradLogin` checkbox + tooltip                 | `el-checkbox` + `el-tooltip`                         |
 * | `autoKillPatcher` checkbox + tooltip           | `el-checkbox` + `el-tooltip`                         |
 * | `skipPlayWnd` checkbox + tooltip               | `el-checkbox` + `el-tooltip`                         |
 * | `btn_Tools` button                             | `el-button` (stub — P12.5 wires real MapleTools/KartTools) |
 * | `Back` button                                  | `el-button` → `router.back()`                        |
 *
 * # Mockup conflict resolution (per Todo.md P12.4 plan)
 *
 * 1. **Layout** — WPF is one long form (App / Game stacked); mockups
 *    typically use sidebar tabs. We keep the WPF one-page form to
 *    avoid introducing a tab abstraction that no other page needs
 *    (SRP: tabbing is a navigation concern, not a settings concern).
 *    Glass panel chrome from the P11 design system is layered on
 *    top for visual polish.
 *
 * 2. **ThemeColor** — WPF used `IsEditable=True ComboBox` accepting
 *    any hex string; mockups show 6 fixed swatches. We render an
 *    `el-input` (free-form) plus an `el-color-picker` for the
 *    swatch affordance. The P11 `WPF_NAMED_COLOR_ALIASES` table in
 *    `composables/useThemeColor.ts` already accepts WPF legacy
 *    named colors (`White` / `Black` / `LightBlue` / …) so an
 *    existing `Config.xml` written by the WPF client still boots
 *    cleanly.
 *
 * 3. **GamePath picker** — WPF used `OpenFileDialog` (synchronous
 *    Win32 modal); we use `@tauri-apps/plugin-dialog`'s `open()`
 *    JS API directly (D1 decision — same precedent as
 *    `ManageAccount.vue` D9). The WPF `FileDialog_Filter` resource
 *    is a pipe-delimited C# format string; we translate the
 *    leading "exe" entry into Tauri's `filters: [{ name, extensions }]`
 *    shape (Tauri can't express WPF's "match exact filename"
 *    behaviour — `extensions` is the closest equivalent).
 *
 * 4. **DisableHardwareAcceleration restart message** — WPF showed
 *    `MessageBox.Show(..., MessageBoxImage.Information)`. SPA uses
 *    `ElMessageBox.alert` with `type: 'info'`, matching the dialog
 *    shape. Message + title resource keys reused verbatim from
 *    `MsgRestartForHardwareAccel` / `MsgRestartForHardwareAccelTitle`.
 *
 * 5. **Tools button** — WPF delegated to
 *    `accountList.btn_Tools_Click` which opens the per-game tools
 *    window (MapleTools / KartTools). P12.5 owns the real handler;
 *    until then we surface a `console.warn` stub identical to
 *    `AccountList.vue`'s `handleTools` so QA can grep one marker
 *    for both call sites.
 *
 * # Why no per-checkbox "value-changed-from-config" guard
 *
 * WPF Settings.xaml.cs guards every CheckedChanged / SelectionChanged
 * handler with `if (newValue == config.GetValue(key))` to avoid
 * re-writing the same value (and to skip the post-write side-effect
 * — e.g. `App.MainWnd.checkPlayPage.IsEnabled` — when nothing
 * actually changed). The SPA uses Pinia + `el-checkbox v-model`,
 * which only fires the setter on real value changes; Element Plus
 * does not emit `change` for programmatic re-assignments via
 * `v-model`, so the guard is unnecessary noise here. The
 * Config.xml writes themselves are idempotent (`commands.setConfig`
 * is a deterministic XML write), so even the rare pathological
 * "set to current value" path is a benign no-op.
 *
 * # Why one `useUiStore` setter per checkbox (vs. a generic K-V API)
 *
 * Type safety. `setAutoKillPatcher(true)` can't accidentally write
 * to the wrong key; a generic `set(key, value)` would lose the
 * literal-key constraint and re-introduce the WPF
 * "stringly-typed Config keys scattered across handlers" problem.
 * The store maintains the per-key WPF-default semantics in one
 * place (see `stores/ui.ts` docblock).
 *
 * # AccountList top-bar wiring (D6)
 *
 * The Settings entry button lands in `AccountList.vue` alongside
 * the existing Logout icon button (matches WPF's MainWindow
 * titlebar Settings + About icons L112-139). Both Settings and
 * About are `requiresAuth: false` because WPF allowed entering
 * Settings from the login page too (WPF `Button_Click` L85-94
 * branches on `return_page` — when null, it returns to
 * `loginPage`).
 */

import { computed, onMounted, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useRouter } from 'vue-router'
import {
  ElButton,
  ElCheckbox,
  ElColorPicker,
  ElIcon,
  ElInput,
  ElMessage,
  ElMessageBox,
  ElOption,
  ElSelect,
  ElTooltip,
} from 'element-plus'
import {
  ArrowLeft,
  FolderOpened,
  InfoFilled,
  Operation,
  Setting as SettingIcon,
  User,
} from '@element-plus/icons-vue'
import { open as openFileDialog } from '@tauri-apps/plugin-dialog'

import { useAuthStore } from '../stores/auth'
import { useConfigStore } from '../stores/config'
import { useGameStore } from '../stores/game'
import { useUiStore, type AppLocale, type LoginMethodValue, type UpdateChannel } from '../stores/ui'

defineOptions({ name: 'SettingsPage' })

const { t } = useI18n()
const router = useRouter()
const auth = useAuthStore()
const configStore = useConfigStore()
const game = useGameStore()
const ui = useUiStore()

/* --------------- D3 — App section (left half) --------------- */

/**
 * Language picker options. WPF `Settings.xaml.cs` L23-26 stores the
 * three culture names verbatim (`zh-Hant` / `zh-Hans` / `en`) with
 * ItemsSource bound to a `LanguageItem` POCO list; we mirror the
 * three options but use the SPA's locale codes (`zh-TW` / `zh-CN`
 * / `en-US`) since `useUiStore.setLanguage` types its argument as
 * {@link AppLocale} and the i18n bundle key tree uses the same
 * shape (see `i18n/index.ts` and `locales/{zh-TW,zh-CN,en-US}.json`).
 *
 * The display labels are hard-coded native-script names rather than
 * passing through `t(...)` because they should appear in the
 * source language regardless of the current UI locale (a Chinese
 * user looking at the English locale should still see "中文(繁體)"
 * for the Traditional Chinese option, not a localized translation
 * of the language name). Mirrors WPF L24-26 verbatim.
 */
const LANGUAGE_OPTIONS: ReadonlyArray<{ value: AppLocale; label: string }> = [
  { value: 'zh-TW', label: '中文(繁體)' },
  { value: 'zh-CN', label: '中文(简体)' },
  { value: 'en-US', label: 'English' },
] as const

/**
 * Update channel picker options. Display labels reuse the WPF
 * resource keys `Stable` / `Development` (the latter renders as
 * "測試版" in zh-TW); the underlying value stored to Config is
 * `'Stable'` / `'Beta'` (matching backend `Channel` enum and WPF
 * Config schema — see `stores/ui.ts::UpdateChannel` docblock).
 */
const UPDATE_CHANNEL_OPTIONS: ReadonlyArray<{ value: UpdateChannel; labelKey: string }> = [
  { value: 'Stable', labelKey: 'Stable' },
  { value: 'Beta', labelKey: 'Development' },
] as const

/**
 * Login method picker options. Values are the WPF integer literals
 * stored as strings (`'0'` Regular / `'1'` QRCode). Display labels
 * reuse WPF resource keys `Regular` / `QrCode`.
 */
const LOGIN_METHOD_OPTIONS: ReadonlyArray<{ value: LoginMethodValue; labelKey: string }> = [
  { value: '0', labelKey: 'Regular' },
  { value: '1', labelKey: 'QrCode' },
] as const

/**
 * `LoginModePanel.Visibility = TW ? Visible : Collapsed`
 * (WPF `MainWindow.xaml.cs::loginMethodChanged` L1023). Mirror the
 * same gate so HK users don't see a picker that has no QR codepath
 * (HK currently routes everything through the regular login flow).
 *
 * `auth.session?.region` resolves to `null` when the user reached
 * Settings before completing login (WPF allowed this via
 * `return_page == loginPage` branch in `Button_Click` L89). On the
 * pre-login path we hide the picker — selecting the login method
 * matters only for users who have already logged in once and want
 * to change it for next session.
 */
const showLoginModePanel = computed<boolean>(() => auth.session?.region === 'TW')

/* --------------- D3/D4 setters — `v-model` writes through store --------------- */

/**
 * Helper to convert an Element Plus `el-select` `change` event
 * into a typed setter call. Element Plus `el-select` `v-model`
 * with an enum-shaped `:value` returns the option's `value`
 * verbatim; we narrow it through the literal type the store
 * expects so an out-of-range `string` cannot silently round-trip.
 *
 * The narrowing is defensive — current `el-option` `:value`
 * bindings are already typed via the `OPTIONS` literals above —
 * but it keeps a single chokepoint if a future option list grows
 * to be loaded dynamically from backend metadata.
 */
function isAppLocaleValue(value: unknown): value is AppLocale {
  return value === 'zh-TW' || value === 'zh-CN' || value === 'en-US'
}

function isUpdateChannelValue(value: unknown): value is UpdateChannel {
  return value === 'Stable' || value === 'Beta'
}

function isLoginMethodValue(value: unknown): value is LoginMethodValue {
  return value === '0' || value === '1'
}

async function handleLanguageChange(value: AppLocale | string | number | boolean): Promise<void> {
  if (!isAppLocaleValue(value)) return
  await ui.setLanguage(value)
}

async function handleUpdateChannelChange(
  value: UpdateChannel | string | number | boolean,
): Promise<void> {
  if (!isUpdateChannelValue(value)) return
  await ui.setUpdateChannel(value)
}

async function handleLoginMethodChange(
  value: LoginMethodValue | string | number | boolean,
): Promise<void> {
  if (!isLoginMethodValue(value)) return
  await ui.setLoginMethod(value)
}

/**
 * ThemeColor change handler — wired to both the free-form
 * `el-input` (for typing hex / WPF named colors) and the
 * `el-color-picker` swatch (for visual selection).
 *
 * Empty / null values from `el-color-picker` (when the user
 * clears the swatch) are coerced back to {@link ui.themeColor}
 * to avoid persisting an empty string into Config.xml that
 * `setPrimaryColor` would then reject. Mirrors WPF
 * `cb_ThemeColor_TextChanged`'s try/catch around the
 * `changeThemeColor` call (L246-251) — a malformed hex throws
 * and we silently keep the previous value.
 */
async function handleThemeColorChange(value: string | null): Promise<void> {
  const next = (value ?? '').trim()
  if (next === '') return
  try {
    await ui.setThemeColor(next)
  } catch {
    /*
     * `setPrimaryColor` throws `RangeError` for a malformed hex
     * (and the WPF named-color alias table only covers 6 known
     * names — anything else falls through to the `parseHexColor`
     * raise). WPF L249 also `catch { }` on bad input; mirroring
     * here keeps the input field showing what the user typed
     * without persisting it (and without surfacing a red toast
     * that would be more confusing than helpful for a typed-mid-
     * input partial value like `#FF`).
     */
  }
}

function handleManageAccount(): void {
  void router.push('/manage-account')
}

/* --------------- D5 — Game section (per-game launcher prefs) --------------- */

/**
 * Reactive game-path display value. Hydrated on mount from
 * Config.xml and refreshed after `pickGamePath` returns. The
 * `el-input` is `readonly` (matches WPF `t_GamePath.IsReadOnly`)
 * — the only mutation path is the click handler.
 *
 * `null` means "not yet hydrated" (initial paint flash); empty
 * string means "no path saved" (renders as a blank field with a
 * "click to pick" affordance); non-empty string is the resolved
 * path.
 */
const gamePath = ref<string>('')

/**
 * Build the Config.xml key WPF uses for per-game-per-region
 * launcher path persistence: `<dir_value_name>.<gameCode>` (WPF
 * `MainWindow.xaml.cs::btn_SetGamePath_Click` L1011).
 *
 * Returns `null` when no game is selected / INI is missing —
 * callers gate on the null to skip the lookup entirely instead
 * of falling back to a partial key that would silently collide
 * across games. SRP: this helper is the one place the key shape
 * is constructed so a future schema change is a one-line edit.
 */
function gamePathConfigKey(): string | null {
  const ini = game.selectedIni
  const code = game.selectedGameCode
  if (!ini || code === null || ini.dir_value_name === '') return null
  return `${ini.dir_value_name}.${code}`
}

/**
 * Hydrate {@link gamePath} from Config.xml on mount and after a
 * successful pick. Mirrors WPF `t_GamePath.Text =` writes:
 *
 * - WPF `MainWindow.xaml.cs::selectedGameChanged` L668 sets the
 *   text to `ConfigAppSettings.GetValue(dir_value_name + "." + gameCode)`.
 * - The Settings page also runs the same `GetValue` indirectly by
 *   relying on the `MainWindow` having already populated it before
 *   the user navigates here.
 *
 * The SPA cannot rely on a parent component pre-populating this
 * value (Settings is a top-level route, not a child of AccountList),
 * so we read directly from the config store. No backend round-trip
 * is needed because `useConfigStore.loadAll()` runs at boot and
 * every subsequent set goes through the in-memory cache.
 */
function refreshGamePathFromConfig(): void {
  const key = gamePathConfigKey()
  gamePath.value = key === null ? '' : (configStore.get(key) ?? '')
}

/**
 * Open a native file picker to choose the game executable, then
 * persist the result to Config.xml. Mirrors WPF
 * `MainWindow.xaml.cs::btn_SetGamePath_Click` (L996-1014):
 *
 * 1. Build the file dialog filter from `FileDialog_Filter` /
 *    `FileDialog_Title` resource templates with `game_exe`
 *    interpolated. WPF used C# `string.Format` with the pipe-
 *    delimited filter syntax `OpenFileDialog.Filter` expects;
 *    Tauri's `open()` API takes `filters: [{ name, extensions }]`
 *    instead — see "WPF deviation" below.
 * 2. Show the picker. If the user cancels (Tauri returns `null`),
 *    bail without mutating Config.
 * 3. Persist the selected path to `<dir_value_name>.<gameCode>`
 *    via `configStore.set` (auto-toasts on backend failure
 *    via `wrapCommand`).
 * 4. Update the local `gamePath` ref so the input re-renders
 *    immediately (no re-mount round-trip).
 *
 * # WPF deviation: filter shape
 *
 * WPF `OpenFileDialog.Filter` accepts `"display|pattern"` pairs
 * and lets the pattern be an exact filename (e.g. `MapleStory.exe`).
 * Tauri's filter API only supports per-extension matching
 * (`extensions: ['exe']`), so we surface "exe files" to the OS
 * picker and rely on the title (`t('FileDialog_Title', [exeName])`)
 * to communicate the expected file. The user can still pick any
 * `.exe` and the WPF launcher itself will ultimately validate the
 * path at game-launch time (`commands.launchGame` does its own
 * existence check).
 *
 * # WPF deviation: skipped pre-game-name prefix in the filter
 *
 * WPF L1001-1002 prepends `accountList.gameName.Content` to the
 * filter string ("新楓之谷主程式|MapleStory.exe|..."). The Tauri
 * API doesn't support a "prefix-style label" — `name` is just the
 * dropdown entry text. We build a single combined name
 * (`<gameName> <FileDialog_Filter[name]>`) so the dropdown still
 * carries the same context, just with a slightly different layout.
 *
 * # Why not error-toast on a Config write failure here
 *
 * `configStore.set` already toasts via `wrapCommand`. Adding a
 * second toast would double-fire the user-visible noise.
 */
async function pickGamePath(): Promise<void> {
  const ini = game.selectedIni
  const code = game.selectedGameCode
  const selected = game.selectedGame
  const key = gamePathConfigKey()

  if (!ini || !selected || code === null || key === null) {
    /*
     * Defensive: the click handler is gated on
     * `v-if="game.selectedGame"` at the template level, so this
     * branch should be unreachable. Surface a structured warning
     * if it ever runs (e.g. a race where the user clicks during
     * a game-switch transition) so we know the gate slipped.
     */
    ElMessage.warning(t('GameSelected'))
    return
  }

  /*
   * Derive the file extension from the INI's `exe` field for the
   * Tauri filter. WPF passes the bare exe name through C# format;
   * we lift the extension because Tauri filters by extension. If
   * the INI ever ships an exe without an extension, fall back to
   * "exe" (the WPF launcher would similarly fail downstream).
   */
  const exeName = ini.exe.split(' ')[0] ?? ini.exe
  const dotIdx = exeName.lastIndexOf('.')
  const extension = dotIdx >= 0 ? exeName.slice(dotIdx + 1) : 'exe'

  /*
   * Build the dropdown name string. WPF resource
   * `FileDialog_Filter` is a pipe-delimited C# format string
   * (`"主程式|{0}|全部檔案 (*.*)|*.*"` in zh-TW); we strip the
   * pipes and interpolate the exe name to get a single human-
   * readable label that fits Tauri's `name` field.
   */
  const filterTemplate = t('FileDialog_Filter')
  const firstPipeIdx = filterTemplate.indexOf('|')
  const exeFilterLabel = firstPipeIdx >= 0 ? filterTemplate.slice(0, firstPipeIdx) : filterTemplate
  const exeFilterDisplay = `${selected.name} ${exeFilterLabel}`

  let picked: string | string[] | null
  try {
    picked = await openFileDialog({
      title: t('FileDialog_Title', [exeName]),
      multiple: false,
      directory: false,
      filters: [{ name: exeFilterDisplay, extensions: [extension] }],
    })
  } catch (err) {
    /*
     * `@tauri-apps/plugin-dialog::open` rejects only on plugin /
     * permission failure (not on user cancel — that resolves to
     * `null`). Surface the message so the user knows why the
     * picker didn't appear.
     */
    const msg = err instanceof Error ? err.message : String(err)
    ElMessage.error(msg)
    return
  }

  if (picked === null || Array.isArray(picked)) return

  await configStore.set(key, picked)
  gamePath.value = picked
}

/* --------------- D5 — Tools button (P12.5 stub) --------------- */

/**
 * Tools button click handler. WPF L271-275 delegates to
 * `accountList.btn_Tools_Click(null, null)` which opens the
 * per-game tools window (MapleTools / KartTools). P12.5 owns the
 * real handler; until then we surface a `console.warn` stub
 * identical to `AccountList.vue`'s `handleTools` so QA can grep
 * one marker for both call sites and so the user gets a consistent
 * "feature not yet ready" experience.
 */
function handleTools(): void {
  console.warn(
    '[Settings] Tools button — handler pending real D-step (P12.5 MapleTools/KartTools).',
  )
}

/* --------------- D4 — DisableHardwareAcceleration restart prompt --------------- */

/**
 * `disableHardwareAcceleration` toggle handler. WPF L213-222 shows
 * a `MessageBox.Show(MsgRestartForHardwareAccel,
 * MsgRestartForHardwareAccelTitle, OK, Information)` after writing
 * the new value to Config — the user has to fully restart the app
 * for the WPF software-rendering switch to take effect.
 *
 * The SPA's WebView does not directly honour the same software
 * rendering flag (Tauri uses WebView2 / WebKit which manage their
 * own GPU usage), but we still preserve the toast so the WPF
 * Config value semantics carry over for any user who shares
 * `Config.xml` between both clients (the WPF process re-reads the
 * value on next boot and applies it). Future SPA work could route
 * the flag into Tauri's CLI flags (e.g. `--disable-gpu`) — out of
 * scope for P12.4.
 */
async function handleDisableHwAccelChange(value: boolean): Promise<void> {
  await ui.setDisableHwAccel(value)
  try {
    await ElMessageBox.alert(
      t('MsgRestartForHardwareAccel'),
      t('MsgRestartForHardwareAccelTitle'),
      /*
       * `confirmButtonText` deliberately defaults to Element
       * Plus's built-in label ("確定" / "OK" depending on the
       * element-plus locale registered in `i18n/index.ts`).
       * The WPF MessageBox uses the OS-level default button text
       * which is locale-aware in the same way; reusing the ELP
       * default keeps both clients aligned without us having to
       * add a one-off `OK` resource key WPF doesn't have.
       */
      { type: 'info' },
    )
  } catch {
    /* User dismissed the alert — no-op. */
  }
}

/* --------------- D6 — Back navigation --------------- */

/**
 * Back button handler. WPF `Button_Click` (L85-94) inspects
 * `App.MainWnd.return_page`:
 *
 * - `null` or equal to `loginPage` → `App.MainWnd.NavigateLoginPage()`
 *   (return to login funnel).
 * - Otherwise → `App.MainWnd.frame.Content = return_page` (return
 *   to whatever page launched Settings).
 *
 * The SPA's vue-router maintains its own history stack — calling
 * `router.back()` returns the user to the previous route entry
 * regardless of source page (login funnel, AccountList, or a
 * future deep-linked entry). This is a strict superset of the WPF
 * branch: the WPF `frame.Content` swap was the WPF-style
 * "single-page back navigation" idiom, which `router.back()`
 * implements natively in an SPA.
 *
 * # Edge case: Settings is the entry route (no back history)
 *
 * If the user opens Settings via direct hash (`#/settings`) with
 * no prior history entry (e.g. devtools navigation), `router.back()`
 * is a no-op. We fall back to `router.push('/login')` after
 * `router.back()` to give the user *some* exit affordance instead
 * of silently dead-ending. The check is best-effort — vue-router
 * does not expose a "can go back" predicate, so we use
 * `window.history.length` as the proxy (a value of `1` means the
 * SPA was opened directly into this route).
 */
function handleBack(): void {
  if (window.history.length > 1) {
    router.back()
    return
  }
  void router.push('/login')
}

/* --------------- mount --------------- */

onMounted(() => {
  refreshGamePathFromConfig()
})
</script>

<template>
  <main class="settings bf-mica-bg">
    <div class="settings__container">
      <!-- Header -->
      <header class="settings__header">
        <div class="settings__header-icon" aria-hidden="true">
          <el-icon :size="24"><SettingIcon /></el-icon>
        </div>
        <div class="settings__header-text">
          <h1 class="settings__title bf-text-gradient">{{ t('Settings') }}</h1>
          <p class="settings__subline">{{ t('settings.subtitle') }}</p>
        </div>
      </header>

      <!-- App section -->
      <section class="settings__section bf-glass-panel" data-test="settings-app-section">
        <header class="settings__section-header">
          <el-icon><User /></el-icon>
          <span>{{ t('AppName') }}</span>
        </header>

        <div class="settings__grid settings__grid--two-col">
          <!-- Left column: Manage account + 3 selects -->
          <div class="settings__col">
            <div class="settings__row">
              <el-button
                class="bf-btn-secondary settings__inline-btn"
                data-test="settings-manage-account"
                @click="handleManageAccount"
              >
                {{ t('ManageAccount') }}
              </el-button>
            </div>

            <div class="settings__row">
              <label class="settings__label">{{ t('UpdateChannel') }}</label>
              <el-select
                :model-value="ui.updateChannel"
                class="settings__select"
                data-test="settings-update-channel"
                @change="handleUpdateChannelChange"
              >
                <el-option
                  v-for="opt in UPDATE_CHANNEL_OPTIONS"
                  :key="opt.value"
                  :value="opt.value"
                  :label="t(opt.labelKey)"
                />
              </el-select>
            </div>

            <div class="settings__row">
              <label class="settings__label">{{ t('Language') }}</label>
              <el-select
                :model-value="ui.language"
                class="settings__select"
                data-test="settings-language"
                @change="handleLanguageChange"
              >
                <el-option
                  v-for="opt in LANGUAGE_OPTIONS"
                  :key="opt.value"
                  :value="opt.value"
                  :label="opt.label"
                />
              </el-select>
            </div>

            <div class="settings__row">
              <label class="settings__label">{{ t('ThemeColor') }}</label>
              <div class="settings__theme-row">
                <el-input
                  :model-value="ui.themeColor"
                  class="settings__theme-input"
                  data-test="settings-theme-input"
                  @change="handleThemeColorChange"
                />
                <el-color-picker
                  :model-value="ui.themeColor"
                  data-test="settings-theme-picker"
                  @change="handleThemeColorChange"
                />
              </div>
            </div>

            <div
              v-if="showLoginModePanel"
              class="settings__row"
              data-test="settings-login-mode-row"
            >
              <label class="settings__label">{{ t('LoginMode') }}</label>
              <el-select
                :model-value="ui.loginMethod"
                class="settings__select"
                data-test="settings-login-mode"
                @change="handleLoginMethodChange"
              >
                <el-option
                  v-for="opt in LOGIN_METHOD_OPTIONS"
                  :key="opt.value"
                  :value="opt.value"
                  :label="t(opt.labelKey)"
                />
              </el-select>
            </div>
          </div>

          <!-- Right column: 4 boolean checkboxes (D4) -->
          <div class="settings__col">
            <div class="settings__row settings__row--checkbox">
              <el-checkbox
                :model-value="ui.askUpdate"
                data-test="settings-ask-update"
                @change="(value) => ui.setAskUpdate(Boolean(value))"
              >
                {{ t('AutoCheckUpdate') }}
              </el-checkbox>
            </div>

            <div class="settings__row settings__row--checkbox">
              <el-checkbox
                :model-value="ui.autoStartGame"
                data-test="settings-auto-start-game"
                @change="(value) => ui.setAutoStartGame(Boolean(value))"
              >
                {{ t('RunAfterLogin') }}
              </el-checkbox>
            </div>

            <div class="settings__row settings__row--checkbox">
              <el-checkbox
                :model-value="ui.minimizeToTray"
                data-test="settings-minimize-to-tray"
                @change="(value) => ui.setMinimizeToTray(Boolean(value))"
              >
                {{ t('MinimizeToTaskbar') }}
              </el-checkbox>
            </div>

            <div class="settings__row settings__row--checkbox">
              <el-tooltip placement="right" :content="t('settings.disableHardwareAccelerationTip')">
                <el-checkbox
                  :model-value="ui.disableHwAccel"
                  data-test="settings-disable-hw-accel"
                  @change="(value) => handleDisableHwAccelChange(Boolean(value))"
                >
                  {{ t('DisableHardwareAcceleration') }}
                </el-checkbox>
              </el-tooltip>
            </div>
          </div>
        </div>
      </section>

      <!-- Game section (D5) — only when a game is selected (WPF parity: if no game, t_GamePath is empty + the section is uninteractive). -->
      <section
        v-if="game.selectedGame"
        class="settings__section bf-glass-panel"
        data-test="settings-game-section"
      >
        <header class="settings__section-header">
          <el-icon><Operation /></el-icon>
          <span>{{ t('Game') }}</span>
        </header>

        <div class="settings__grid">
          <div class="settings__row">
            <label class="settings__label">{{ t('GamePath') }}</label>
            <el-input
              :model-value="gamePath"
              readonly
              :placeholder="t('settings.gamePathPlaceholder')"
              class="settings__game-path-input"
              data-test="settings-game-path"
              @click="pickGamePath"
            >
              <template #suffix>
                <el-icon class="settings__game-path-icon"><FolderOpened /></el-icon>
              </template>
            </el-input>
          </div>

          <div class="settings__grid settings__grid--two-col">
            <div class="settings__col">
              <div class="settings__row settings__row--checkbox">
                <el-tooltip placement="right" :content="t('settings.tradLoginTip')">
                  <el-checkbox
                    :model-value="ui.tradLogin"
                    data-test="settings-trad-login"
                    @change="(value) => ui.setTradLogin(Boolean(value))"
                  >
                    {{ t('TraditionalLoginMode') }}
                  </el-checkbox>
                </el-tooltip>
              </div>

              <div class="settings__row settings__row--checkbox">
                <el-tooltip placement="right" :content="t('settings.killPatcherTip')">
                  <el-checkbox
                    :model-value="ui.autoKillPatcher"
                    data-test="settings-auto-kill-patcher"
                    @change="(value) => ui.setAutoKillPatcher(Boolean(value))"
                  >
                    {{ t('KillPatcher') }}
                  </el-checkbox>
                </el-tooltip>
              </div>
            </div>

            <div class="settings__col">
              <div class="settings__row settings__row--checkbox">
                <el-tooltip placement="right" :content="t('settings.skipPlayWindowTip')">
                  <el-checkbox
                    :model-value="ui.skipPlayWnd"
                    data-test="settings-skip-play-wnd"
                    @change="(value) => ui.setSkipPlayWnd(Boolean(value))"
                  >
                    {{ t('SkipPlayWindow') }}
                  </el-checkbox>
                </el-tooltip>
              </div>

              <div class="settings__row">
                <el-button
                  class="bf-btn-secondary settings__inline-btn"
                  data-test="settings-tools"
                  @click="handleTools"
                >
                  {{ t('Tools') }}
                </el-button>
              </div>
            </div>
          </div>
        </div>
      </section>

      <!-- Game section empty banner (no selected game) — informational, mirrors WPF's empty t_GamePath fallback semantically. -->
      <section
        v-else
        class="settings__section bf-glass-panel settings__section--empty"
        data-test="settings-game-section-empty"
      >
        <el-icon class="settings__empty-icon" :size="20"><InfoFilled /></el-icon>
        <p class="settings__empty-text">{{ t('settings.gameSectionEmpty') }}</p>
      </section>

      <!-- Footer: Back button -->
      <footer class="settings__footer">
        <el-button
          class="bf-btn-secondary settings__back-btn"
          data-test="settings-back"
          @click="handleBack"
        >
          <el-icon><ArrowLeft /></el-icon>
          <span>{{ t('Back') }}</span>
        </el-button>
      </footer>
    </div>
  </main>
</template>

<style scoped>
.settings {
  box-sizing: border-box;
  min-height: 100vh;
  padding: 2.5rem 1.5rem;
  display: flex;
  justify-content: center;
}

.settings__container {
  width: 100%;
  max-width: 880px;
  display: flex;
  flex-direction: column;
  gap: 1rem;
}

/* --------------- header --------------- */

.settings__header {
  display: flex;
  align-items: center;
  gap: 0.875rem;
  margin-bottom: 0.25rem;
}

.settings__header-icon {
  width: 44px;
  height: 44px;
  border-radius: var(--bf-radius-button);
  background: linear-gradient(
    135deg,
    color-mix(in srgb, var(--bf-primary-container) 30%, transparent),
    color-mix(in srgb, var(--bf-primary) 25%, transparent)
  );
  color: var(--bf-on-primary);
  display: grid;
  place-items: center;
  flex-shrink: 0;
  box-shadow: var(--bf-shadow-card);
}

.settings__header-text {
  min-width: 0;
}

.settings__title {
  margin: 0;
  font-size: 1.625rem;
  font-weight: 800;
  letter-spacing: -0.01em;
  line-height: 1.15;
}

.settings__subline {
  margin: 0.25rem 0 0;
  font-size: 0.8125rem;
  color: var(--bf-on-surface-variant);
}

/* --------------- section --------------- */

.settings__section {
  padding: 1rem 1.25rem 1.25rem;
  display: flex;
  flex-direction: column;
  gap: 0.75rem;
}

.settings__section-header {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  font-size: 0.875rem;
  font-weight: 700;
  color: var(--bf-on-surface-variant);
  padding-bottom: 0.5rem;
  border-bottom: 1px solid color-mix(in srgb, var(--bf-outline-variant) 25%, transparent);
}

.settings__section--empty {
  flex-direction: row;
  align-items: center;
  gap: 0.625rem;
  padding: 0.875rem 1rem;
}

.settings__empty-icon {
  flex-shrink: 0;
  color: var(--bf-on-surface-variant);
}

.settings__empty-text {
  margin: 0;
  font-size: 0.8125rem;
  color: var(--bf-on-surface-variant);
}

/* --------------- grid / row --------------- */

.settings__grid {
  display: flex;
  flex-direction: column;
  gap: 0.625rem;
}

.settings__grid--two-col {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 0.875rem;
}

@media (max-width: 600px) {
  .settings__grid--two-col {
    grid-template-columns: 1fr;
  }
}

.settings__col {
  display: flex;
  flex-direction: column;
  gap: 0.625rem;
  min-width: 0;
}

.settings__row {
  display: flex;
  flex-direction: column;
  gap: 0.25rem;
}

.settings__row--checkbox {
  flex-direction: row;
  align-items: center;
}

.settings__label {
  font-size: 0.8125rem;
  color: var(--bf-on-surface-variant);
}

.settings__select,
.settings__game-path-input,
.settings__theme-input {
  width: 100%;
}

.settings__theme-row {
  display: flex;
  align-items: center;
  gap: 0.5rem;
}

.settings__theme-input {
  flex: 1;
}

.settings__inline-btn {
  align-self: flex-start;
}

.settings__game-path-input :deep(.el-input__inner) {
  cursor: pointer;
}

.settings__game-path-icon {
  color: var(--bf-on-surface-variant);
}

/* --------------- footer --------------- */

.settings__footer {
  display: flex;
  justify-content: flex-end;
  margin-top: 0.5rem;
}

.settings__back-btn {
  display: inline-flex;
  align-items: center;
  gap: 0.375rem;
}
</style>
