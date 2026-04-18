/**
 * Runtime Element Plus primary-color switcher.
 *
 * ELP exposes the primary color as a CSS custom property
 * (`--el-color-primary`) plus a fixed set of derived shades
 * (`--el-color-primary-light-{3,5,7,9}` and
 * `--el-color-primary-dark-2`). Components don't read the raw
 * primary directly — they reference the shade variables in their
 * stylesheets. To live-swap the theme color we therefore have to
 * (a) set the primary, and (b) recompute every shade by linear-mixing
 * the primary toward white (lighter shades) or black (darker shade).
 *
 * The mixing formula matches ELP's own SCSS implementation 1:1:
 *
 *     light-N = mix(primary, white, weight = N * 10%)
 *     dark-N  = mix(primary, black, weight = N * 10%)
 *
 * where `weight` is the *amount of the secondary color* (white/black).
 * This keeps the visual identity of the official ELP themes —
 * components designed for ELP just work without per-component overrides.
 *
 * # Preset colors (P11 mockup `_design-system.html`)
 *
 * The 8 presets mirror the WPF Settings page color picker exactly so
 * users migrating from the legacy client see familiar swatches. The
 * raw `primary` hex is the same value the mockup's
 * `[data-theme="…"] { --primary: … }` rules use.
 *
 * # Why a composable + framework-agnostic helper
 *
 * `setPrimaryColor` is exported as a plain function (no Vue
 * reactivity required) so the boot sequence in `App.vue` can call it
 * before Pinia / Vue Router are mounted. `useThemeColor()` wraps the
 * helper for components that want a typed handle to the preset list
 * + setter without re-importing constants.
 */

/**
 * Named preset color, shown as a swatch in the Settings page.
 *
 * `name` is the i18n key suffix (`themePreset.{name}`); `primary`
 * is the hex color the WPF mockup uses. Stored as a plain const
 * tuple-of-objects rather than a `Record` so the swatch ordering in
 * the UI is stable and explicit.
 */
export interface ThemePreset {
  readonly name: string
  readonly primary: string
}

export const THEME_PRESETS: readonly ThemePreset[] = [
  { name: 'orange', primary: '#FF8201' },
  { name: 'green', primary: '#5C8430' },
  { name: 'lightblue', primary: '#0B6E99' },
  { name: 'pink', primary: '#D85A88' },
  { name: 'gold', primary: '#C9A227' },
  { name: 'silver', primary: '#7A7A7A' },
  { name: 'black', primary: '#1A1A1A' },
  { name: 'white', primary: '#555555' },
] as const

/** Default primary color when no user preference exists. */
export const DEFAULT_PRIMARY_COLOR = THEME_PRESETS[0].primary

/**
 * Case-insensitive alias table from legacy WPF Settings-page named
 * colors (see `Beanfun/Pages/Settings.xaml` L90-97) to the matching
 * P11 preset hex above.
 *
 * # Why this exists
 *
 * The old WPF client's ThemeColor ComboBox offered 8 entries — 2
 * hex literals (`#FF8201`, `#B6DE8E`) and 6 WPF named colors
 * (`White` / `Black` / `LightBlue` / `Pink` / `Gold` / `Silver`).
 * WPF parsed both shapes through
 * `ColorConverter.ConvertFromString(sColor)`
 * (`MainWindow.xaml.cs::changeThemeColor` L249). Users who picked
 * a named color have `<ThemeColor>LightBlue</ThemeColor>` written
 * verbatim into their `Config.xml` — which on next boot hits our
 * stricter `parseHexColor` and throws `RangeError`.
 *
 * # Why alias instead of migrate
 *
 * We normalise the string in-memory rather than rewriting
 * `Config.xml` so:
 *
 * - Users running both clients against a shared config don't see
 *   one silently overwrite the other's theme value.
 * - The composable stays a pure function (no `config.set` side
 *   effect, no async boundary to propagate through `setPrimaryColor`
 *   callers) — SRP wins.
 *
 * # Why the hex target is the P11 preset (not the WPF named hex)
 *
 * The P11 design system intentionally re-tuned every named color
 * for contrast / saturation against the new white-cards UI
 * (`useThemeColor` module docstring L22-28). A user who selected
 * `LightBlue` in WPF expected the swatch they clicked — the P11
 * swatch labelled "lightblue" is the direct redesign descendant of
 * that option, so mapping to `#0B6E99` preserves semantic intent
 * even though the pixel is different.
 *
 * # Why the 2 hex ComboBox entries aren't listed
 *
 * `#FF8201` / `#B6DE8E` round-trip through `parseHexColor`
 * untouched — no alias needed. Keeping them out of this table
 * also means a user who hand-wrote `#B6DE8E` in Config.xml sees
 * that exact color, not the P11 `#5C8430` green preset (respect
 * user's literal input).
 */
const WPF_NAMED_COLOR_ALIASES: Record<string, string> = {
  white: '#555555',
  black: '#1A1A1A',
  lightblue: '#0B6E99',
  pink: '#D85A88',
  gold: '#C9A227',
  silver: '#7A7A7A',
}

/**
 * Translate a stored ThemeColor string into a form
 * [`parseHexColor`] can consume.
 *
 * Handles:
 *
 * - hex strings (with or without `#`, 3 or 6 digits) — returned as
 *   is; downstream [`parseHexColor`] does the actual validation.
 * - WPF legacy named colors from
 *   [`WPF_NAMED_COLOR_ALIASES`] — returned as the P11 preset hex.
 *   Case-insensitive, leading / trailing whitespace tolerated
 *   (mirrors WPF's `ColorConverter` which trims).
 *
 * Unknown strings fall through unchanged so
 * [`parseHexColor`]'s `RangeError` surfaces the bad value to
 * callers (which already wrap it in a try/catch + log +
 * default-fallback, see `stores/ui.ts::applyAll`).
 */
export function resolvePrimaryColor(stored: string): string {
  const key = stored.trim().toLowerCase()
  return WPF_NAMED_COLOR_ALIASES[key] ?? stored
}

/**
 * ELP shade definitions: each tuple is
 * `[cssVarSuffix, mixWeight, mixTarget]` where `mixWeight` is the
 * weight of `mixTarget` (i.e. how much white or black to add).
 *
 * The list intentionally mirrors ELP's `dark-2`, `light-3`, …,
 * `light-9` naming so a future ELP minor version that introduces
 * additional shades only requires adding entries here.
 */
const ELP_SHADES = [
  ['dark-2', 0.2, '#000000'],
  ['light-3', 0.3, '#ffffff'],
  ['light-5', 0.5, '#ffffff'],
  ['light-7', 0.7, '#ffffff'],
  ['light-8', 0.8, '#ffffff'],
  ['light-9', 0.9, '#ffffff'],
] as const

/**
 * Parse a 3- or 6-digit hex color into an `[r, g, b]` triple.
 *
 * @throws {RangeError} when `hex` is not a valid 3/6-digit hex string.
 */
export function parseHexColor(hex: string): [number, number, number] {
  const normalized = hex.trim().replace(/^#/, '').toLowerCase()
  const expanded =
    normalized.length === 3
      ? normalized
          .split('')
          .map((c) => c + c)
          .join('')
      : normalized
  if (!/^[0-9a-f]{6}$/.test(expanded)) {
    throw new RangeError(`invalid hex color: ${hex}`)
  }
  const num = parseInt(expanded, 16)
  return [(num >> 16) & 0xff, (num >> 8) & 0xff, num & 0xff]
}

const toHexComponent = (n: number): string =>
  Math.round(Math.max(0, Math.min(255, n)))
    .toString(16)
    .padStart(2, '0')

/**
 * Linear-mix two hex colors and return the result as a 6-digit
 * hex string with a leading `#`. `weight` is the proportion of
 * `mixWith` (0 = pure base, 1 = pure mixWith). Out-of-range
 * weights are clamped to `[0, 1]`.
 *
 * Matches ELP's `mix($base, $mixWith, $weight)` SCSS function
 * verbatim (linear RGB; no gamma correction — accepted simplification
 * because the WPF original also doesn't gamma-correct).
 */
export function mixHexColor(base: string, mixWith: string, weight: number): string {
  const w = Math.max(0, Math.min(1, weight))
  const [br, bg, bb] = parseHexColor(base)
  const [mr, mg, mb] = parseHexColor(mixWith)
  const r = br * (1 - w) + mr * w
  const g = bg * (1 - w) + mg * w
  const b = bb * (1 - w) + mb * w
  return `#${toHexComponent(r)}${toHexComponent(g)}${toHexComponent(b)}`
}

/**
 * Apply `primaryHex` as the document's primary color, recomputing
 * every ELP shade.
 *
 * @param primaryHex — 3- or 6-digit hex (with or without `#`), OR
 *   a legacy WPF Settings named color (see
 *   [`WPF_NAMED_COLOR_ALIASES`]). The legacy strings route through
 *   [`resolvePrimaryColor`] before hex parsing so a `Config.xml`
 *   written by the old WPF client (e.g. `<ThemeColor>LightBlue`)
 *   boots cleanly into the matching P11 preset instead of crashing
 *   on the first frame.
 * @param target — optional override for the element receiving the
 *   custom properties; defaults to `document.documentElement`. The
 *   override exists so vitest can pass a fresh `HTMLElement` per
 *   test (jsdom shares `document.documentElement` across all `it`
 *   blocks in the same file).
 *
 * @throws {RangeError} when `primaryHex` is neither a valid
 *   3/6-digit hex nor a known WPF named-color alias.
 */
export function setPrimaryColor(primaryHex: string, target?: HTMLElement): void {
  const root = target ?? document.documentElement
  const resolved = resolvePrimaryColor(primaryHex)
  const normalized = `#${parseHexColor(resolved).map(toHexComponent).join('')}`

  root.style.setProperty('--el-color-primary', normalized)
  for (const [suffix, weight, mixWith] of ELP_SHADES) {
    root.style.setProperty(`--el-color-primary-${suffix}`, mixHexColor(normalized, mixWith, weight))
  }
}

/**
 * Composable handle for components that want both the preset list
 * and the setter without re-importing the module-level helpers.
 *
 * No reactivity is created — the current color is read from the
 * computed CSS custom property whenever `getCurrentPrimary` is
 * called, which keeps this composable safe to call outside a
 * component lifecycle (e.g. during boot before `setup()` runs).
 */
export function useThemeColor() {
  return {
    presets: THEME_PRESETS,
    defaultPrimary: DEFAULT_PRIMARY_COLOR,
    setPrimaryColor,
    /**
     * Returns the currently-applied primary as the document sees it,
     * trimmed of any whitespace. Useful for Settings-page swatches
     * to highlight the active preset.
     */
    getCurrentPrimary(target?: HTMLElement): string {
      const root = target ?? document.documentElement
      return getComputedStyle(root).getPropertyValue('--el-color-primary').trim()
    },
  }
}
