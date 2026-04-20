import { describe, expect, it } from 'vitest'

import {
  type CoreItem,
  coreItemEquals,
  coreItemToString,
  findPerfectCores,
  formatPerfectCoreResult,
  mustCoreCount,
} from '../../../src/services/coreCalculator'

/**
 * Convenience: build a `CoreItem` literal in tests.
 *
 * Keeping the test factory tiny (instead of `{ skill1, skill2,
 * skill3 }` everywhere) makes the multi-core fixtures below
 * legible at a glance — matching the WPF original where each
 * `new CoreItem("A", "B", "C")` reads as a 3-tuple.
 */
const core = (skill1: string, skill2: string, skill3: string): CoreItem => ({
  skill1,
  skill2,
  skill3,
})

describe('mustCoreCount', () => {
  it('returns the WPF minimum of 2 for empty / tiny skill lists', () => {
    expect(mustCoreCount(0)).toBe(2)
    expect(mustCoreCount(1)).toBe(2)
    expect(mustCoreCount(2)).toBe(2)
  })

  it('rounds ceil(skillCount * 2 / 3) above the floor', () => {
    expect(mustCoreCount(3)).toBe(2)
    expect(mustCoreCount(4)).toBe(3)
    expect(mustCoreCount(5)).toBe(4)
    expect(mustCoreCount(6)).toBe(4)
    expect(mustCoreCount(9)).toBe(6)
  })
})

describe('coreItemEquals', () => {
  it('treats swapped secondary slots as equal (mirrors WPF Equals)', () => {
    expect(coreItemEquals(core('A', 'B', 'C'), core('A', 'C', 'B'))).toBe(true)
    expect(coreItemEquals(core('A', 'B', 'C'), core('A', 'B', 'C'))).toBe(true)
  })

  it('rejects when skill1 differs even if secondaries match', () => {
    expect(coreItemEquals(core('A', 'B', 'C'), core('Z', 'B', 'C'))).toBe(false)
  })

  it('rejects when one secondary differs', () => {
    expect(coreItemEquals(core('A', 'B', 'C'), core('A', 'B', 'D'))).toBe(false)
  })
})

describe('coreItemToString', () => {
  it('renders WPF format `skill1(main)/skill2/skill3`', () => {
    expect(coreItemToString(core('Wing', 'Bomb', 'Star'), 'Main')).toBe('Wing(Main)/Bomb/Star')
  })

  it('uses the caller-supplied main label so locales swap cleanly', () => {
    expect(coreItemToString(core('Wing', 'Bomb', 'Star'), '主')).toBe('Wing(主)/Bomb/Star')
  })
})

describe('findPerfectCores', () => {
  it('treats every distinct-skill1 subset of size 2 as perfect when must-skills is empty', () => {
    /*
     * WPF parity: with `MustSkills` empty, the perfect check
     * is a vacuous `foreach` (no skills to verify), so every
     * size-`mustCount` subset that survives the
     * dedup-by-skill1 gate is reported. `mustCount` floors
     * at 2 (see `mustCoreCount`), so two cores with distinct
     * `skill1` produce one group. Documented here so a
     * future "is this a bug?" check confirms it's intended
     * WPF behaviour, not an oversight in the port.
     */
    const items = [core('A', 'B', 'C'), core('B', 'A', 'D')]
    const result = findPerfectCores(items, [])
    expect(result).toHaveLength(1)
    expect(result[0]).toEqual(items)
  })

  it('finds the perfect combination covering each must-skill at least twice', () => {
    /*
     * 3 must-skills A/B/C → mustCount = 2 (ceil(3*2/3) = 2).
     * Two cores below contain each of A/B/C at least twice
     * across their three slots:
     *   - A(...)/B/C   covers A, B, C (all once)
     *   - B(...)/A/C   covers A (1), B (1+1=2 with first), C (1+1=2)
     * Sum: A=2, B=2, C=2 → perfect.
     */
    const items = [core('A', 'B', 'C'), core('B', 'A', 'C')]
    const result = findPerfectCores(items, ['A', 'B', 'C'])
    expect(result).toHaveLength(1)
    expect(result[0]).toEqual([core('A', 'B', 'C'), core('B', 'A', 'C')])
  })

  it('reports zero groups when no subset of size mustCount covers every must-skill twice', () => {
    /*
     * Same must-skills, but the second core only covers A
     * once and never touches B/C → A=1, B=1, C=1, all below
     * the ≥2 threshold.
     */
    const items = [core('A', 'B', 'C'), core('A', 'D', 'E')]
    const result = findPerfectCores(items, ['A', 'B', 'C'])
    expect(result).toEqual([])
  })

  it('skips candidate subsets where two picked cores share the same skill1', () => {
    /*
     * mustCount = 2. Picking cores [0] and [1] both have
     * skill1=A → dedup-by-skill1 collapses sub.length to 1,
     * which fails the `sub.length === mustCount` gate. The
     * algorithm should still find the valid (A,*) + (B,*)
     * combination via the [0]+[2] subset.
     */
    const items = [core('A', 'B', 'C'), core('A', 'B', 'C'), core('B', 'A', 'C')]
    const result = findPerfectCores(items, ['A', 'B', 'C'])
    /*
     * Expected: only one perfect group, using cores [0] + [2]
     * (the [1] + [2] combo would yield the same shape but
     * `findPerfectCores` walks subsets lexicographically so
     * the first hit wins; the [0] + [2] subset is reached
     * first). The [1] + [2] subset is also a valid perfect
     * combination on its own, so `result.length === 2` is
     * the expected count.
     */
    expect(result).toHaveLength(2)
    expect(result[0]).toEqual([core('A', 'B', 'C'), core('B', 'A', 'C')])
    expect(result[1]).toEqual([core('A', 'B', 'C'), core('B', 'A', 'C')])
  })
})

describe('formatPerfectCoreResult', () => {
  const labels = {
    mainLabel: 'Main',
    notFound: 'No combo found',
    formatGroup: (n: number): string => `Group ${n}:`,
  }

  it('returns "<notFound>\\nBy:LinTx" on empty result (mirrors WPF L206)', () => {
    expect(formatPerfectCoreResult([], labels)).toBe('No combo found\nBy:LinTx')
  })

  it('lists every group with header + blank-line separator + LinTx footer', () => {
    const groups = [
      [core('A', 'B', 'C'), core('B', 'A', 'C')],
      [core('A', 'D', 'E'), core('D', 'A', 'E')],
    ]
    expect(formatPerfectCoreResult(groups, labels)).toBe(
      [
        'Group 1:',
        'A(Main)/B/C',
        'B(Main)/A/C',
        '',
        'Group 2:',
        'A(Main)/D/E',
        'D(Main)/A/E',
        '',
        'By:LinTx',
      ].join('\n'),
    )
  })
})
