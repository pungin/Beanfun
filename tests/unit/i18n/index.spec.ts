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
  translateCommandError,
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
  // Generated dictionaries must stay complete in every supported locale.
  it('all three generated WPF dictionaries load non-empty', () => {
    expect(Object.keys(zhTwGenerated).length).toBeGreaterThan(0)
    expect(Object.keys(zhCnGenerated).length).toBeGreaterThan(0)
    expect(Object.keys(enUsGenerated).length).toBeGreaterThan(0)
  })

  it('zh-CN declares the same generated keys as zh-TW', () => {
    expect(Object.keys(zhCnGenerated).sort()).toEqual(Object.keys(zhTwGenerated).sort())
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

  it('keeps every message non-empty with matching interpolation parameters', () => {
    const flatten = (obj: object, prefix = ''): Record<string, string> =>
      Object.fromEntries(
        Object.entries(obj).flatMap(([key, value]) => {
          const path = prefix ? `${prefix}.${key}` : key
          return typeof value === 'object' ? Object.entries(flatten(value, path)) : [[path, value]]
        }),
      )
    const reference = flatten(i18nMessages['zh-TW'])
    const placeholders = (text: string) => [...text.matchAll(/\{(\w+)\}/g)].map((m) => m[1]).sort()
    for (const [locale, messages] of Object.entries(i18nMessages)) {
      const flat = flatten(messages)
      expect(Object.keys(flat).sort(), locale).toEqual(Object.keys(reference).sort())
      for (const [key, value] of Object.entries(flat)) {
        expect(value.trim(), `${locale}: ${key}`).not.toBe('')
        expect(placeholders(value), `${locale}: ${key}`).toEqual(placeholders(reference[key]))
      }
    }
  })

  it('merges WPF flat keys with frontend-only nested keys', () => {
    expect(i18nMessages['zh-TW']).toHaveProperty('AppName', '繽放')
    expect(i18nMessages['zh-TW'].titleBar).toHaveProperty('minimize')
    expect(i18nMessages['en-US']).toHaveProperty('AppName')
    expect(i18nMessages['en-US'].titleBar).toHaveProperty('minimize')
  })
})

describe('createAppI18n', () => {
  it('boots with zh-TW as the default locale and zh-TW messages applied', () => {
    const i18n = createAppI18n()
    expect(i18n.global.locale.value).toBe('zh-TW')
    expect(i18n.global.t('titleBar.minimize')).toBe(i18nMessages['zh-TW'].titleBar.minimize)
  })

  it('renders zh-CN messages when locale switches', () => {
    const i18n = createAppI18n()
    setLocale(i18n, 'zh-CN')
    expect(i18n.global.t('titleBar.minimize')).toBe(i18nMessages['zh-CN'].titleBar.minimize)
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
    expect(i18n.global.t('titleBar.minimize')).toBe(i18nMessages['zh-TW'].titleBar.minimize)
    setLocale(i18n, 'en-US')
    expect(i18n.global.t('titleBar.minimize')).toBe(i18nMessages['en-US'].titleBar.minimize)
  })

  it('keeps the zh-CN to zh-TW fallback order for a missing resource', () => {
    const i18n = createAppI18n()
    i18n.global.mergeLocaleMessage('zh-TW', { fallbackProbe: '繁體備援' })
    i18n.global.mergeLocaleMessage('en-US', { fallbackProbe: 'English fallback' })
    setLocale(i18n, 'zh-CN')
    expect(i18n.global.t('fallbackProbe')).toBe('繁體備援')
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

  it('translates newly covered errors in every locale without producing a toast', () => {
    const i18n = createAppI18n()
    wireI18n(i18n)
    for (const locale of SUPPORTED_LOCALES) {
      setLocale(i18n, locale)
      expect(
        translateCommandError({ code: 'config.io_failed', message: 'raw error', details: null }),
      ).toBe(i18nMessages[locale].errors.config.io_failed)
      expect(i18n.global.t('about.qqCopied', { number: '123456' })).toContain('123456')
    }
    expect(elMessageError).not.toHaveBeenCalled()
    expect(consoleErrorSpy).not.toHaveBeenCalled()
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
