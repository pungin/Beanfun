/**
 * D9 — Static i18n key-usage audit.
 *
 * Two invariants this spec mechanically enforces, both as `expect(...).toEqual([])`
 * fail-loud assertions (no warning-only mode — see "design rationale" below):
 *
 * 1. **Missing-key guard**: every literal `t('some.key')` call site
 *    in the application source resolves to a key declared in the
 *    canonical zh-TW message tree (frontend-only ∪ WPF generated).
 *    Catches typos like `t('loginVerfy.title')` at `npm run test`
 *    time instead of at the live boot when the user sees a fallback
 *    string.
 *
 * 2. **Dead frontend-only key guard**: every leaf key declared
 *    under `FRONTEND_ONLY_MESSAGES['zh-TW']` is consumed somewhere
 *    — either via a literal `t('...')` call **or** via an
 *    explicitly-registered dynamic-key consumer (see
 *    {@link DYNAMIC_KEY_CONSUMERS}). Catches dead keys left behind
 *    by D-step refactors (e.g. removing a banner without removing
 *    its translation) so the message tree doesn't bit-rot.
 *
 * # Design rationale
 *
 * - **Why fail rather than warn**: the user-rule is "don't take
 *   shortcuts; finish what's started" — a warning-only spec is
 *   noise nobody checks. Hard-fail forces the next D-step author
 *   to either (a) actually use the new key, (b) remove it, or
 *   (c) extend `DYNAMIC_KEY_CONSUMERS` with the dynamic-consumer
 *   declaration + a `reason` comment. All three outcomes are
 *   strictly better than silent dead keys.
 *
 * - **Why limit dead-key check to frontend-only keys**: the WPF
 *   generated locale JSON ships hundreds of keys for pages we
 *   haven't ported yet (`AccountList`, `Settings`, `EditAccount`,
 *   etc.); marking those "dead" would produce noise on every
 *   D-step and bury the signal we actually want. Once a WPF page
 *   is ported, its keys naturally stop being "dead" because the
 *   ported view consumes them.
 *
 * - **Why scan only `pages/composables/components/stores`**: those
 *   are the only directories where `useI18n().t(...)` is legitimately
 *   called. Excluding `services/` skips `invoke.ts` whose docblock
 *   text would create false positives; excluding `i18n/messages.ts`
 *   skips its own declarations. `types/bindings.ts` is auto-generated
 *   and lives outside the scan scope by directory, not allowlist.
 *
 * - **Why a separate spec from `index.spec.ts`**: `index.spec.ts`
 *   is concerned with the runtime i18n bootstrap (message loading,
 *   locale switching, translator wiring); this spec is a
 *   compile-time-style static analysis over source files. They
 *   share `collectKeyPaths` conceptually but live as siblings to
 *   keep each file focused on a single responsibility.
 *
 * # Maintenance
 *
 * When you add a new dynamic-key consumer (e.g. a future
 * `t(\`section.\${id}.title\`)` pattern), update
 * {@link DYNAMIC_KEY_CONSUMERS} with either a `prefix` (matches
 * any path starting with `prefix`) or a `literal` (matches exact
 * paths) entry. The `reason` and `usedBy` fields document *why*
 * the keys are not statically traceable so future maintainers
 * don't delete the entry by mistake.
 */

import { describe, expect, it } from 'vitest'

import { i18nMessages } from '../../../src/i18n'
import { FRONTEND_ONLY_MESSAGES } from '../../../src/i18n/messages'

/* ------------------------------------------------------------------ */
/* File loader                                                         */
/* ------------------------------------------------------------------ */

/**
 * Load every application source file likely to contain `t(...)`
 * calls. `import.meta.glob('?raw', { eager: true })` returns
 * `Record<absolutePath, fileContents>` — vite's preferred way of
 * doing build-time file enumeration without a Node `fs` round-trip.
 *
 * The pattern intentionally stays narrow (only the four directories
 * where Vue components and Pinia stores live); see the module
 * docblock "Why scan only…" section for the rationale.
 */
const APP_SOURCES = import.meta.glob('/src/{pages,composables,components,stores}/**/*.{vue,ts}', {
  query: '?raw',
  import: 'default',
  eager: true,
}) as Record<string, string>

/* ------------------------------------------------------------------ */
/* Comment stripping                                                   */
/* ------------------------------------------------------------------ */

/**
 * Strip JS / TS / Vue HTML comments from source so docblocks that
 * happen to mention `t('foo')` don't show up as fake call sites.
 *
 * The implementation is intentionally simple-minded: it does not
 * understand template-literal nesting or string-inside-comment
 * edge cases. Empirically every `t(...)` reference in our codebase
 * lives either in real code or in a `/* ... *\/`-style block
 * comment, both of which this handles.
 */
function stripComments(src: string): string {
  return (
    src
      // Block comments: /* ... */
      .replace(/\/\*[\s\S]*?\*\//g, '')
      // Line comments: // ... (whole-line only — avoids URL `://`)
      .replace(/^\s*\/\/[^\n]*$/gm, '')
      // Vue / HTML comments: <!-- ... -->
      .replace(/<!--[\s\S]*?-->/g, '')
  )
}

/* ------------------------------------------------------------------ */
/* Literal `t('...')` extractor                                        */
/* ------------------------------------------------------------------ */

/**
 * Match a bare `t('literal_key')` call. The lookbehind-style
 * leading character class rejects `obj.t(...)`, `let.t(...)` etc.
 * so we only catch the `useI18n().t` injection.
 *
 * Quote characters: only `'` and `"` — template literals are
 * intentionally rejected because they always denote a dynamic key
 * (`t(\`errors.\${code}\`)`) which belongs in
 * {@link DYNAMIC_KEY_CONSUMERS}, not the static call-site list.
 *
 * Key character class `[\w.]+`: alphanumerics, underscore, and
 * dot. Matches both flat WPF keys (`Login`) and nested frontend
 * keys (`loginShell.heading`).
 */
const LITERAL_T_RE = /(?:[^a-zA-Z_$.]|^)t\(\s*(['"])([\w.]+)\1\s*[,)]/g

interface TCallSite {
  readonly file: string
  readonly key: string
}

function extractLiteralTCalls(file: string, src: string): TCallSite[] {
  const stripped = stripComments(src)
  const out: TCallSite[] = []
  let m: RegExpExecArray | null
  while ((m = LITERAL_T_RE.exec(stripped)) !== null) {
    out.push({ file, key: m[2] })
  }
  return out
}

const ALL_LITERAL_CALL_SITES: readonly TCallSite[] = Object.entries(APP_SOURCES).flatMap(
  ([file, src]) => extractLiteralTCalls(file, src),
)

const ALL_LITERAL_KEYS: ReadonlySet<string> = new Set(ALL_LITERAL_CALL_SITES.map((c) => c.key))

/* ------------------------------------------------------------------ */
/* Dynamic-key consumer registry                                       */
/* ------------------------------------------------------------------ */

/**
 * Declarations of code paths where i18n keys are looked up
 * dynamically (not via a literal `t('...')` call). Every entry
 * here is implicitly a "this key family is in use, don't flag
 * its leaves as dead" allowance.
 *
 * Add a new entry whenever you introduce a dynamic-key call site
 * that the static analyzer cannot otherwise see, **and** include
 * the `reason` + `usedBy` so a future maintainer reading this
 * file understands the contract before removing the entry.
 *
 * - `prefix` entries match any path starting with the prefix
 *   (used for runtime-resolved key families like `errors.{code}`).
 * - `literal` entries match exact paths (used for short
 *   enumerated lists like `Taiwan` / `HongKong`).
 */
type DynamicConsumer =
  | {
      readonly kind: 'prefix'
      readonly prefix: string
      readonly reason: string
      readonly usedBy: string
    }
  | {
      readonly kind: 'literal'
      readonly keys: readonly string[]
      readonly reason: string
      readonly usedBy: string
    }

const DYNAMIC_KEY_CONSUMERS: readonly DynamicConsumer[] = [
  {
    kind: 'prefix',
    prefix: 'errors.',
    reason:
      'Resolved at runtime from CommandError.code via the registered translator (`translator(`errors.${code}`, fallback)`).',
    usedBy: 'src/services/invoke.ts::surfaceCommandError',
  },
  {
    kind: 'prefix',
    prefix: 'themePreset.',
    reason:
      'Each preset name is appended by the future Settings page swatch list (THEME_PRESETS[i].name → t(`themePreset.${name}`)).',
    usedBy: 'src/composables/useThemeColor.ts::THEME_PRESETS',
  },
  {
    kind: 'literal',
    keys: ['loginRegion.defaultBadge', 'loginRegion.totpHint'],
    reason: 'Region tile hint key is read dynamically from TILES[i].hintKey.',
    usedBy: 'src/pages/LoginRegionSelection.vue::TILES',
  },
]

function isCoveredByDynamicConsumer(path: string): boolean {
  for (const consumer of DYNAMIC_KEY_CONSUMERS) {
    if (consumer.kind === 'prefix' && path.startsWith(consumer.prefix)) return true
    if (consumer.kind === 'literal' && consumer.keys.includes(path)) return true
  }
  return false
}

/* ------------------------------------------------------------------ */
/* Key-path collector                                                  */
/* ------------------------------------------------------------------ */

/**
 * Walk a plain object and yield every leaf key path (`a.b.c`).
 * Re-implemented here (instead of importing from `index.spec.ts`)
 * to keep each spec file self-contained — sharing helpers across
 * specs creates a coupling that bites when one spec is run in
 * isolation via `vitest path/to/single.spec.ts`.
 */
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

const ZH_TW_DECLARED_KEYS: ReadonlySet<string> = new Set(collectKeyPaths(i18nMessages['zh-TW']))

const FRONTEND_ONLY_LEAF_PATHS: readonly string[] = collectKeyPaths(FRONTEND_ONLY_MESSAGES['zh-TW'])

/* ------------------------------------------------------------------ */
/* The actual specs                                                    */
/* ------------------------------------------------------------------ */

describe('static t() call-site scanner', () => {
  it('actually finds source files to scan (sanity guard against a broken glob)', () => {
    /*
     * If `import.meta.glob` ever silently returns an empty record
     * (e.g. someone moves the spec file or renames `src/`), the
     * literal extractor produces zero matches and both invariants
     * vacuously pass — silently disarming the entire audit. This
     * sanity assertion makes that failure mode impossible.
     */
    expect(Object.keys(APP_SOURCES).length).toBeGreaterThan(5)
    expect(ALL_LITERAL_CALL_SITES.length).toBeGreaterThan(20)
  })
})

describe('missing i18n key guard', () => {
  it("every t('literal') call site refers to a key declared in i18nMessages['zh-TW']", () => {
    const missing = ALL_LITERAL_CALL_SITES.filter(({ key }) => !ZH_TW_DECLARED_KEYS.has(key)).map(
      ({ file, key }) => `${file}: t('${key}')`,
    )

    expect(missing).toEqual([])
  })
})

describe('dead frontend-only key guard', () => {
  it('every leaf key under FRONTEND_ONLY_MESSAGES.zh-TW is consumed somewhere', () => {
    const dead = FRONTEND_ONLY_LEAF_PATHS.filter((path) => {
      if (ALL_LITERAL_KEYS.has(path)) return false
      if (isCoveredByDynamicConsumer(path)) return false
      return true
    })

    expect(dead).toEqual([])
  })

  it('every dynamic-consumer literal key actually exists in zh-TW (typo guard)', () => {
    /*
     * A literal-style consumer entry whose `keys` list contains a
     * typo would silently keep flagging the *real* key as dead.
     * Pinning the literal keys to the declared tree catches that
     * inversion at test time.
     */
    const literalConsumerKeys = DYNAMIC_KEY_CONSUMERS.flatMap((c) =>
      c.kind === 'literal' ? c.keys : [],
    )
    const typos = literalConsumerKeys.filter((key) => !ZH_TW_DECLARED_KEYS.has(key))
    expect(typos).toEqual([])
  })
})
