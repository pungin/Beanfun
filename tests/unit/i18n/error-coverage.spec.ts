import { describe, expect, it } from 'vitest'
import { i18nMessages } from '../../../src/i18n'

const sources = import.meta.glob('/src-tauri/src/**/*.rs', {
  query: '?raw',
  import: 'default',
  eager: true,
}) as Record<string, string>

// Production error constructors precede the trailing Rust test modules.
// Remove comments so documentation examples do not become emitted errors.
const production = Object.fromEntries(
  Object.entries(sources).map(([file, source]) => [
    file,
    source
      .split('#[cfg(test)]')[0]
      .replace(/\/\*[\s\S]*?\*\//g, '')
      .replace(/^\s*\/\/[^\n]*$/gm, ''),
  ]),
)

const constants = Object.fromEntries(
  Object.entries(production).map(([file, source]) => [
    file,
    Object.fromEntries(
      [...source.matchAll(/const\s+(\w+)\s*:\s*&str\s*=\s*"([\w.]+)"/g)].map((match) => [
        match[1],
        match[2],
      ]),
    ),
  ]),
)

const codes = new Set<string>()
const unresolved: string[] = []
for (const [file, source] of Object.entries(production)) {
  for (const match of source.matchAll(/CommandError::new\(\s*([^,]+),/g)) {
    const argument = match[1].trim()
    const literal = /^"([\w.]+)"$/.exec(argument)?.[1]
    const name = argument.split('::').at(-1)!
    const candidates = new Set(
      Object.values(constants)
        .map((map) => map[name])
        .filter(Boolean),
    )
    // Resolve per-file constants first: several modules use PLATFORM_UNSUPPORTED_CODE.
    const code =
      literal ?? constants[file][name] ?? (candidates.size === 1 ? [...candidates][0] : null)
    if (code) codes.add(code)
    else unresolved.push(`${file}: ${argument}`)
  }
}

describe('backend error translations', () => {
  it('resolves every production CommandError constructor, including named constants', () => {
    expect(Object.keys(sources).length).toBeGreaterThan(10)
    expect(codes.size).toBeGreaterThan(100)
    expect(unresolved).toEqual([])
  })

  it.each(Object.entries(i18nMessages))(
    '%s translates every emitted backend error code',
    (_, messages) => {
      const errors = messages.errors as Record<string, Record<string, string>>
      const missing = [...codes].filter((code) => {
        const [domain, key] = code.split('.')
        return !errors[domain]?.[key]?.trim()
      })
      expect(missing).toEqual([])
    },
  )
})
