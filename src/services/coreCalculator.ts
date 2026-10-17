/**
 * Pure helpers for the MapleStory perfect-core calculator
 * (P12.5 D4).
 *
 * Preserves the combinations, ordering and distinct-main-skill rule of
 * `Beanfun/Windows/CoreCalculator.xaml.cs`. Prefix coverage bounds avoid
 * enumerating branches that cannot produce a valid combination.
 *
 * # Why this lives in `services/` instead of `composables/`
 *
 * The helpers are framework-agnostic — they consume plain
 * arrays + strings and return plain arrays + strings, with no
 * Vue reactivity / i18n binding inside. Putting them under
 * `services/` (alongside `invoke.ts`) keeps them unit-testable
 * without instantiating a Vue component or stubbing
 * `useI18n`. The `windows/CoreCalculator.vue` shell wires the
 * reactive refs + i18n labels around these helpers in the
 * usual Vue way.
 *
 * # Why we re-implement (not import a generic combinations crate)
 *
 * Main-skill uniqueness is checked during traversal, before descending
 * into a branch. A generic combinations iterator would perform this
 * check only after generating combinations that are already invalid.
 */

/**
 * A single Maple core: one main skill (`skill1`) plus two
 * secondary skills (`skill2`, `skill3`).
 *
 * `skill2` / `skill3` are interchangeable by design (a core's
 * two secondary slots have no ordering inside Maple). The
 * {@link coreItemEquals} helper enforces that semantic so
 * `(Wing / Bomb / Star)` and `(Wing / Star / Bomb)` are
 * treated as the same core in dedup checks (mirrors
 * `CoreItem.Equals` L294-301 of the WPF source).
 */
export interface CoreItem {
  readonly skill1: string
  readonly skill2: string
  readonly skill3: string
}

/**
 * Number of cores required to form a "perfect" combination
 * for the given count of must-have skills.
 *
 * Mirrors `mustCoreCount` (L227-233) verbatim:
 *
 *     ceil(skillCount * 2 / 3), minimum 2
 *
 * The minimum-2 floor reflects that even with one or two
 * must-skills the player still needs at least two cores to
 * cover each one twice (the "perfect" criterion below).
 */
export function mustCoreCount(skillCount: number): number {
  const raw = Math.ceil((skillCount * 2) / 3)
  return raw < 2 ? 2 : raw
}

/**
 * Order-insensitive equality on the secondary skill pair —
 * mirrors `CoreItem.Equals` exactly.
 *
 * Two cores are equal iff:
 *
 *     a.skill1 === b.skill1 AND
 *     ({ a.skill2, a.skill3 } as multiset === { b.skill2, b.skill3 })
 *
 * Used by the AddCore dedup check so the same core can't be
 * entered twice via swapped secondary slots.
 */
export function coreItemEquals(a: CoreItem, b: CoreItem): boolean {
  if (a.skill1 !== b.skill1) return false
  return (
    (a.skill2 === b.skill2 && a.skill3 === b.skill3) ||
    (a.skill2 === b.skill3 && a.skill3 === b.skill2)
  )
}

/**
 * Render a single core for the result panel. Mirrors
 * `CoreItem.ToString()` (L283-292):
 *
 *     "{skill1}({Main})/{skill2}/{skill3}"
 *
 * `mainLabel` is supplied by the caller because the WPF source
 * resolves it via `Application.Current.TryFindResource("Main")`
 * — the SPA passes `t('Main')` from the component so the helper
 * stays i18n-agnostic.
 */
export function coreItemToString(item: CoreItem, mainLabel: string): string {
  return `${item.skill1}(${mainLabel})/${item.skill2}/${item.skill3}`
}

/**
 * Enumerate valid groups in the original mask order (largest index first).
 * Prefix coverage counts prune branches that cannot cover a required skill;
 * duplicate main skills are rejected before descending into a branch.
 */
export function findPerfectCores(
  coreItems: readonly CoreItem[],
  mustSkills: readonly string[],
): CoreItem[][] {
  const result: CoreItem[][] = []
  const mustCount = mustCoreCount(mustSkills.length)
  if (coreItems.length < mustCount) return result

  const multiplicities = new Map<string, number>()
  for (const skill of mustSkills) {
    multiplicities.set(skill, (multiplicities.get(skill) ?? 0) + 1)
  }
  const skills = [...multiplicities.keys()]
  const thresholds = skills.map((skill) => Math.ceil(2 / multiplicities.get(skill)!))
  // Snapshot skill data before traversing reactive input.
  const inputs = coreItems.map((item) => ({
    main: item.skill1,
    skills: new Set([item.skill1, item.skill2, item.skill3]),
  }))
  if (new Set(inputs.map((item) => item.main)).size < mustCount) return result
  const coverage = inputs.map((item) =>
    skills.flatMap((skill, index) => (item.skills.has(skill) ? [index] : [])),
  )
  const prefixes = [new Uint32Array(skills.length)]
  for (const covered of coverage) {
    const next = prefixes[prefixes.length - 1].slice()
    for (const skill of covered) next[skill]++
    prefixes.push(next)
  }

  const counts = new Uint32Array(skills.length)
  const mains = new Set<string>()
  const chosen: number[] = []
  function visit(limit: number, remaining: number): void {
    if (limit < remaining) return
    for (let skill = 0; skill < skills.length; skill++) {
      if (counts[skill] + Math.min(remaining, prefixes[limit][skill]) < thresholds[skill]) {
        return
      }
    }
    if (remaining === 0) {
      result.push([...chosen].reverse().map((index) => coreItems[index]))
      return
    }
    for (let index = remaining - 1; index < limit; index++) {
      const main = inputs[index].main
      if (mains.has(main)) continue
      mains.add(main)
      chosen.push(index)
      for (const skill of coverage[index]) counts[skill]++
      visit(index, remaining - 1)
      for (const skill of coverage[index]) counts[skill]--
      chosen.pop()
      mains.delete(main)
    }
  }
  visit(coreItems.length, mustCount)
  return result
}

/**
 * Labels the {@link formatPerfectCoreResult} call needs so it
 * can stay i18n-agnostic. Caller passes `t(...)` lookups + a
 * `formatGroup` callback that injects the 1-based group index
 * into the `CoreGroup` template (`第{0}組組合：`).
 */
export interface FormatLabels {
  /** `t('Main')` — the localized "Main" tag inside `CoreItem.ToString`. */
  readonly mainLabel: string
  /** `t('NotFindPerfectCore')` — body when `result` is empty. */
  readonly notFound: string
  /**
   * `(n) => t('CoreGroup', [n])` — header for each group of
   * cores. Caller owns the i18n list-mode interpolation so
   * this helper has no dependency on vue-i18n.
   */
  readonly formatGroup: (groupIndex1Based: number) => string
}

/**
 * Render the Calculate button result panel. Mirrors
 * `btn_Calculator_Click` L204-223 verbatim, including the
 * `By:LinTx` byline (the original author's signature in the
 * WPF source — preserved as a credit line, *not* localised).
 *
 * `\n` is used in place of WPF's `\r\n` because Vue's
 * `<el-input type="textarea">` and JS clipboard APIs both
 * normalise to `\n`; WPF used `\r\n` only because that's the
 * Windows convention for `string.Format` line endings. The
 * user-visible text is identical — paste into Notepad would
 * collapse one to the other on most Windows tooling anyway.
 */
export function formatPerfectCoreResult(
  result: readonly CoreItem[][],
  labels: FormatLabels,
): string {
  if (result.length === 0) {
    return `${labels.notFound}\nBy:LinTx`
  }
  const lines: string[] = []
  for (let i = 0; i < result.length; i++) {
    lines.push(labels.formatGroup(i + 1))
    for (const item of result[i]) {
      lines.push(coreItemToString(item, labels.mainLabel))
    }
    lines.push('')
  }
  lines.push('By:LinTx')
  return lines.join('\n')
}
