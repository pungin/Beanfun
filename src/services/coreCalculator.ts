/**
 * Pure helpers for the MapleStory perfect-core calculator
 * (P12.5 D4).
 *
 * Mirrors the logic of `Beanfun/Windows/CoreCalculator.xaml.cs`
 * verbatim so a player who memorised the WPF tool's behaviour
 * (which combinations it lists, in what order, with what
 * dedup rule) sees the same output here. Every numeric formula,
 * inequality bound, and traversal order is preserved literally;
 * only the data structures change shape (C# `bool[]` →
 * `Uint8Array`, `Dictionary<string,bool>` → `Set<string>`,
 * `ObservableCollection<T>` → readonly TS arrays).
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
 * The WPF algorithm is more than "pick K out of N"; it
 * **deduplicates by `skill1`** while iterating, so a candidate
 * subset that happens to contain two cores with the same main
 * skill is implicitly truncated and rejected. A generic
 * combinations iterator would have to be wrapped in a custom
 * filter that re-implements that dedup — at which point we
 * have the same code with an extra layer of indirection. The
 * literal port keeps the behaviour identical to WPF and lets
 * the docblock spell out the dedup rule once.
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
 * Enumerate every "perfect core" subset of `coreItems` that
 * covers each must-skill at least twice while using exactly
 * {@link mustCoreCount}`(mustSkills.length)` cores with
 * pair-wise distinct main skills.
 *
 * Direct port of `btn_Calculator_Click` L123-203. The
 * iteration uses the same `bool[] zero` mask scheme WPF does,
 * which is essentially Knuth's "advance the rightmost
 * shiftable 1" combination iterator — preserved literally so
 * the result order matches WPF rev-by-rev.
 *
 * # Algorithm sketch
 *
 * 1. Initialise `zero` to `[true × mustCount, false × rest]`.
 * 2. Each iteration:
 *    a. Walk `zero` left→right; collect a deduplicated `sub`
 *       of the picked cores keyed by `skill1`. If two
 *       picked cores share `skill1`, only the first is
 *       added — `sub.length < mustCount` then trips the
 *       perfect-check off and the candidate is silently
 *       rejected.
 *    b. During the same walk, find the leftmost
 *       `(true → false)` boundary (`per1 && !zero[i]`),
 *       flip `zero[i]` true, and call that index `index`.
 *       Track `leftCount` = the number of `true`s seen
 *       before that boundary.
 *    c. Reset `zero[0..index)` to `[true × leftCount,
 *       false × rest]`. This is the "advance" step that
 *       walks lexicographically through subsets.
 *    d. If `sub.length === mustCount`, count must-skill
 *       occurrences across the three slots of all cores in
 *       `sub`. Push to `result` iff every must-skill
 *       appears ≥ 2 times.
 *    e. If `index === -1` after the walk (no further
 *       advance possible), break.
 *
 * # Complexity
 *
 * O(C(N, K) × K × M) where N = cores, K = mustCount, M =
 * mustSkills.length. The WPF algo is the same and has been
 * shipped against player-realistic N (≤ ~30) for years; no
 * pruning is needed.
 */
export function findPerfectCores(
  coreItems: readonly CoreItem[],
  mustSkills: readonly string[],
): CoreItem[][] {
  const result: CoreItem[][] = []
  const mustCount = mustCoreCount(mustSkills.length)
  const size = coreItems.length

  /*
   * Initial mask: first `mustCount` indices selected. Using
   * `Uint8Array` instead of `boolean[]` saves one allocation
   * per iteration; WPF used `bool[]` for the same purpose.
   * Treat 0 as false / 1 as true throughout.
   */
  const zero = new Uint8Array(size)
  for (let i = 0; i < size; i++) {
    zero[i] = i < mustCount ? 1 : 0
  }

  for (;;) {
    const sub: CoreItem[] = []
    const keys = new Set<string>()
    let index = -1
    let per1 = false
    let leftCount = -1

    for (let i = 0; i < size; i++) {
      if (zero[i] === 1) {
        const item = coreItems[i]
        if (!keys.has(item.skill1)) {
          keys.add(item.skill1)
          sub.push(item)
        }
      }
      if (index === -1) {
        if (per1 && zero[i] === 0) {
          zero[i] = 1
          index = i
        } else {
          per1 = zero[i] === 1
          if (per1) leftCount++
        }
      }
    }

    /*
     * Reset the prefix `[0..index)` to `[true × leftCount,
     * false × rest]` so the next iteration picks the next
     * lexicographic subset. When `index === -1` (loop body
     * found no `1 → 0` boundary), this loop is a no-op and
     * the outer loop exits below.
     */
    for (let i = 0; i < index; i++) {
      zero[i] = i < leftCount ? 1 : 0
    }

    if (sub.length === mustCount) {
      const occurrences = new Map<string, number>()
      for (const skill of mustSkills) {
        for (const item of sub) {
          if (skill === item.skill1 || skill === item.skill2 || skill === item.skill3) {
            occurrences.set(skill, (occurrences.get(skill) ?? 0) + 1)
          }
        }
      }
      let isPerfect = true
      for (const skill of mustSkills) {
        if ((occurrences.get(skill) ?? 0) < 2) {
          isPerfect = false
          break
        }
      }
      if (isPerfect) result.push(sub)
    }

    if (index === -1) break
  }

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
