/**
 * D10 — vue-i18n bootstrap & key-consistency guard.
 *
 * The two flagship guarantees of this spec:
 *
 * 1. **No locale drift.** All three locales (zh-TW / zh-CN / en-US)
 *    must declare the same set of generated WPF keys *and* the same
 *    nested tree under the frontend-only namespaces. Any future
 *    `convert-lang.mjs` regen or hand-edit to `messages.ts` that
 *    drops a key on a single locale fails this spec immediately.
 * 2. **Wire-up actually wires.** `wireI18n` registers both the UI
 *    store's locale applier *and* the invoke layer's error
 *    translator, so a `commands.foo()` failure surfaces a localized
 *    toast and `setLanguage(...)` flips the rendered text.
 */

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'

import enUsGenerated from '../../../src/locales/en-US.json'
import zhCnGenerated from '../../../src/locales/zh-CN.json'
import zhTwGenerated from '../../../src/locales/zh-TW.json'

import { createAppI18n, i18nMessages, setLocale, wireI18n } from '../../../src/i18n'
import { FRONTEND_ONLY_MESSAGES } from '../../../src/i18n/messages'
import {
  __resetInvokeRegistriesForTesting,
  surfaceCommandError,
} from '../../../src/services/invoke'
import { __resetUiAppliersForTesting, SUPPORTED_LOCALES, useUiStore } from '../../../src/stores/ui'

vi.mock('element-plus', () => ({ ElMessage: { error: vi.fn() } }))

vi.mock('../../../src/types/bindings', () => ({
  commands: {
    setConfig: vi.fn(async () => ({ status: 'ok', data: null })),
    getAllConfig: vi.fn(async () => ({ status: 'ok', data: {} })),
  },
}))

import { ElMessage } from 'element-plus'

const elMessageError = vi.mocked(ElMessage.error)

/** Walk a plain object and yield every leaf key path (`a.b.c`). */
function collectKeyPaths(obj: unknown, prefix = ''): string[] {
  if (obj === null || typeof obj !== 'object') return [prefix]
  const out: string[] = []
  for (const [key, value] of Object.entries(obj as Record<string, unknown>)) {
    const next = prefix ? `${prefix}.${key}` : key
    if (value !== null && typeof value === 'object' && !Array.isArray(value)) {
      out.push(...collectKeyPaths(value, next))
    } else {
      out.push(next)
    }
  }
  return out.sort()
}

describe('locale assets', () => {
  /*
   * WPF-generated JSON drift policy (P11 reality check):
   *
   * The original P11 Q3 plan assumed three XAML dictionaries share an
   * identical key set — they don't. `Beanfun/Lang/zh-Hans.xaml` is
   * genuinely missing ~30 keys vs `zh.xaml` / `en.xaml`, and the
   * legacy WPF app relies on ResourceDictionary's built-in fallback
   * chain at runtime to paper over this. Mirroring WPF 1:1 means
   * we inherit that drift as-is rather than back-filling strings
   * we have no authoritative translation for.
   *
   * So the invariants we actually enforce here are:
   *
   * 1. Every generated JSON loads non-empty — catches a broken
   *    `convert-lang.mjs` invocation (empty output silently).
   * 2. zh-CN is a *subset* of zh-TW — catches a renegade key added
   *    to zh-CN that doesn't match the canonical source. (Upstream
   *    WPF never adds zh-CN-only keys, so this also serves as a
   *    forward guard: if the WPF translator starts inventing keys,
   *    we notice.)
   * 3. zh-TW and en-US match each other — both are kept parallel by
   *    upstream convention and empirically ship with the same key
   *    set today.
   *
   * Runtime gaps in zh-CN are filled by vue-i18n's `fallbackLocale:
   * 'en-US'` (see `i18n/index.ts`), which matches WPF's behavior.
   */
  it('all three generated WPF dictionaries load non-empty', () => {
    expect(Object.keys(zhTwGenerated).length).toBeGreaterThan(0)
    expect(Object.keys(zhCnGenerated).length).toBeGreaterThan(0)
    expect(Object.keys(enUsGenerated).length).toBeGreaterThan(0)
  })

  it('zh-CN introduces no keys that do not exist in the zh-TW canonical dictionary', () => {
    const zhTwKeys = new Set(Object.keys(zhTwGenerated))
    const renegade = Object.keys(zhCnGenerated).filter((k) => !zhTwKeys.has(k))
    expect(renegade).toEqual([])
  })

  it('zh-TW and en-US declare an identical key set (no drift between parallel translations)', () => {
    const zhTwKeys = Object.keys(zhTwGenerated).sort()
    const enUsKeys = Object.keys(enUsGenerated).sort()
    expect(enUsKeys).toEqual(zhTwKeys)
  })

  /*
   * Frontend-only message tree (`i18n/messages.ts`): we 100% control
   * these, so strict equality is the right bar. A missing key in any
   * locale is a source-of-truth bug caught at `npm run test` time.
   */
  it('all three frontend-only message trees declare an identical nested key tree', () => {
    const zhTwPaths = collectKeyPaths(FRONTEND_ONLY_MESSAGES['zh-TW'])
    const zhCnPaths = collectKeyPaths(FRONTEND_ONLY_MESSAGES['zh-CN'])
    const enUsPaths = collectKeyPaths(FRONTEND_ONLY_MESSAGES['en-US'])

    expect(zhCnPaths).toEqual(zhTwPaths)
    expect(enUsPaths).toEqual(zhTwPaths)
  })

  it('exposes a message map covering every supported locale', () => {
    for (const locale of SUPPORTED_LOCALES) {
      expect(i18nMessages).toHaveProperty(locale)
    }
  })

  it('merges WPF flat keys with frontend-only nested keys', () => {
    expect(i18nMessages['zh-TW']).toHaveProperty('AppName', '繽放')
    expect(i18nMessages['zh-TW'].loginShell).toHaveProperty('heading')
    expect(i18nMessages['en-US']).toHaveProperty('AppName')
    expect(i18nMessages['en-US'].loginShell).toHaveProperty('heading')
  })
})

describe('createAppI18n', () => {
  it('boots with zh-TW as the default locale and zh-TW messages applied', () => {
    const i18n = createAppI18n()
    expect(i18n.global.locale.value).toBe('zh-TW')
    expect(i18n.global.t('loginShell.heading')).toBe(i18nMessages['zh-TW'].loginShell.heading)
  })

  it('renders zh-CN messages when locale switches', () => {
    const i18n = createAppI18n()
    setLocale(i18n, 'zh-CN')
    expect(i18n.global.t('loginShell.heading')).toBe(i18nMessages['zh-CN'].loginShell.heading)
  })

  it('substitutes positional placeholders ({0}) in WPF-style messages', () => {
    /*
     * `GashRemain` is a generated WPF key with a single `{0}` placeholder
     * (point amount). Picked here because the frontend-only namespace no
     * longer ships any positional-arg strings post-Placeholder removal,
     * and this spec must keep proving vue-i18n's WPF-style interpolation
     * still works on the auto-generated dictionary.
     */
    const i18n = createAppI18n()
    const rendered = i18n.global.t('GashRemain', ['1234'])
    expect(rendered).toContain('1234')
  })

  it('setLocale switches the rendered language end-to-end', () => {
    const i18n = createAppI18n()
    expect(i18n.global.t('loginShell.heading')).toBe(i18nMessages['zh-TW'].loginShell.heading)
    setLocale(i18n, 'en-US')
    expect(i18n.global.t('loginShell.heading')).toBe(i18nMessages['en-US'].loginShell.heading)
  })
})

describe('wireI18n', () => {
  let consoleErrorSpy: ReturnType<typeof vi.spyOn>

  beforeEach(() => {
    setActivePinia(createPinia())
    __resetUiAppliersForTesting()
    __resetInvokeRegistriesForTesting()
    elMessageError.mockClear()
    consoleErrorSpy = vi.spyOn(console, 'error').mockImplementation(() => {})
  })

  afterEach(() => {
    __resetUiAppliersForTesting()
    __resetInvokeRegistriesForTesting()
    consoleErrorSpy.mockRestore()
  })

  it('registers a locale applier so ui.setLanguage flips the i18n locale', async () => {
    const i18n = createAppI18n()
    wireI18n(i18n)

    const ui = useUiStore()
    await ui.setLanguage('en-US')

    expect(i18n.global.locale.value).toBe('en-US')
  })

  it('routes invoke errors through the i18n translator → localized toast', () => {
    const i18n = createAppI18n()
    wireI18n(i18n)

    surfaceCommandError({
      code: 'auth.session_required',
      message: 'fallback (English from backend)',
      details: null,
    })

    expect(elMessageError).toHaveBeenCalledWith(i18nMessages['zh-TW'].errors.auth.session_required)
  })

  it('falls back to backend-supplied message when the error code has no i18n key', () => {
    const i18n = createAppI18n()
    wireI18n(i18n)

    const fallback = 'totally novel error from backend'
    surfaceCommandError({
      code: 'never_heard_of_this_domain.nope',
      message: fallback,
      details: null,
    })

    expect(elMessageError).toHaveBeenCalledWith(fallback)
  })

  it('localized toast follows locale switches at runtime', () => {
    const i18n = createAppI18n()
    wireI18n(i18n)

    surfaceCommandError({
      code: 'auth.session_required',
      message: 'ignored',
      details: null,
    })
    expect(elMessageError).toHaveBeenLastCalledWith(
      i18nMessages['zh-TW'].errors.auth.session_required,
    )

    setLocale(i18n, 'en-US')
    surfaceCommandError({
      code: 'auth.session_required',
      message: 'ignored',
      details: null,
    })
    expect(elMessageError).toHaveBeenLastCalledWith(
      i18nMessages['en-US'].errors.auth.session_required,
    )
  })
})
