#!/usr/bin/env node
// @ts-check

/**
 * Convert legacy WPF `Beanfun/Lang/*.xaml` resource dictionaries
 * into vue-i18n flat KV JSON files in `src/locales/`.
 *
 * Run from the `beanfun-next/` directory:
 *
 *     node scripts/convert-lang.mjs
 *
 * # Mapping (P11 Q3 = A: WPF key 1:1)
 *
 *     Beanfun/Lang/zh.xaml      → src/locales/zh-TW.json
 *     Beanfun/Lang/zh-Hans.xaml → src/locales/zh-CN.json
 *     Beanfun/Lang/en.xaml      → src/locales/en-US.json
 *
 * # What is extracted
 *
 * Only `<system:String x:Key="K">V</system:String>` entries become
 * keys in the output JSON. The XAML files also contain non-string
 * resources (`<Geometry>` for the SVG-style logo path,
 * `<TextBlock>` for embedded mini-rich-text views) that are not
 * translatable strings and would not survive a JSON round-trip; the
 * Vue port re-implements those visual resources directly in the
 * page templates instead. See P12 page rebuild for the migration
 * plan.
 *
 * # Placeholder & escape conventions (preserved verbatim)
 *
 * - `{0}`, `{1}`, … — kept as-is. vue-i18n's list-mode interpolation
 *   accepts them via `t(key, [arg0, arg1])`, matching WPF's
 *   `string.Format` semantics 1:1.
 * - `%0d` — the WPF source uses URI-style escapes for newlines in a
 *   handful of strings (e.g. `FeedbackText`). Kept as raw text; the
 *   consuming page is responsible for the same `Uri.UnescapeDataString`
 *   step the WPF code does.
 * - `&lt;R&gt;`, `&lt;B&gt;`, etc. — XML entity escapes for
 *   nested mini-markup (`<R Foreground="Red">…</R>`) used by WPF's
 *   `RichTextBlock`. fast-xml-parser auto-decodes these so the
 *   JSON value contains the unescaped `<R>` / `<B>` form. The Vue
 *   port renders these strings via `<RichText>` (P12) and parses
 *   the same syntax.
 *
 * # Why a separate parser export
 *
 * `parseXamlStrings` is exported so the vitest spec can drive it
 * with inline fixtures without touching the real WPF files. The
 * `main` entry point only runs when this module is executed
 * directly (CLI use); importing it in tests does not write
 * anything to disk.
 */

import { XMLParser } from 'fast-xml-parser'
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const __filename = fileURLToPath(import.meta.url)
const __dirname = dirname(__filename)

/** Frontend root (`beanfun-next/`). */
const FRONTEND_ROOT = resolve(__dirname, '..')
/** Repo root (one level above `beanfun-next/`). */
const REPO_ROOT = resolve(FRONTEND_ROOT, '..')

const WPF_LANG_DIR = resolve(REPO_ROOT, 'Beanfun', 'Lang')
const FRONTEND_LOCALES_DIR = resolve(FRONTEND_ROOT, 'src', 'locales')

/**
 * @typedef {{ source: string; target: string }} LocaleMapEntry
 */

/** @type {LocaleMapEntry[]} */
export const LOCALE_FILE_MAP = [
  { source: 'zh.xaml', target: 'zh-TW.json' },
  { source: 'zh-Hans.xaml', target: 'zh-CN.json' },
  { source: 'en.xaml', target: 'en-US.json' },
]

/**
 * Parse a XAML resource-dictionary string and return only the
 * `<system:String x:Key="K">V</system:String>` entries as a flat
 * object.
 *
 * Order of keys follows the order they appear in the source XAML so
 * that diffs remain reviewable when WPF strings are re-translated
 * upstream (insertion order is preserved by `Object.keys` in
 * modern engines).
 *
 * @param {string} xaml — UTF-8 XAML source.
 * @returns {Record<string, string>}
 */
export function parseXamlStrings(xaml) {
  const parser = new XMLParser({
    ignoreAttributes: false,
    attributeNamePrefix: '@_',
    textNodeName: '#text',
    parseAttributeValue: false,
    parseTagValue: false,
    trimValues: false,
    // Ensure repeated `<system:String>` siblings always become an
    // array even when there's only one — simpler downstream branching.
    isArray: (tagName) => tagName === 'system:String',
  })

  const tree = parser.parse(xaml)
  const root = tree.ResourceDictionary
  if (!root) {
    throw new Error('XAML root element <ResourceDictionary> not found')
  }

  /** @type {unknown} */
  const stringNodes = root['system:String']
  if (!stringNodes) return {}
  if (!Array.isArray(stringNodes)) {
    throw new Error('expected system:String to be an array (isArray hint failed)')
  }

  /** @type {Record<string, string>} */
  const out = {}
  for (const node of stringNodes) {
    if (typeof node !== 'object' || node === null) continue
    const obj = /** @type {Record<string, unknown>} */ (node)
    const key = obj['@_x:Key']
    if (typeof key !== 'string' || key.length === 0) continue
    const text = obj['#text']
    out[key] = typeof text === 'string' ? text : ''
  }
  return out
}

/**
 * CLI entry point. Reads each XAML file in {@link LOCALE_FILE_MAP},
 * converts it via {@link parseXamlStrings}, and writes the JSON
 * artefact to `src/locales/`. Logs a one-line summary per file to
 * stdout; throws (non-zero exit via the caller) on any I/O or parse
 * error so CI pipelines can detect drift.
 */
export function convertAllLocales() {
  mkdirSync(FRONTEND_LOCALES_DIR, { recursive: true })
  for (const { source, target } of LOCALE_FILE_MAP) {
    const inPath = resolve(WPF_LANG_DIR, source)
    const outPath = resolve(FRONTEND_LOCALES_DIR, target)
    const xaml = readFileSync(inPath, 'utf8')
    const obj = parseXamlStrings(xaml)
    writeFileSync(outPath, JSON.stringify(obj, null, 2) + '\n', 'utf8')
    console.log(`convert-lang: ${source} → ${target} (${Object.keys(obj).length} keys)`)
  }
}

const isMain = process.argv[1] && resolve(process.argv[1]) === resolve(__filename)
if (isMain) {
  try {
    convertAllLocales()
  } catch (err) {
    console.error('convert-lang: failed', err)
    process.exit(1)
  }
}
