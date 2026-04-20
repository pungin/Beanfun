import { describe, expect, it } from 'vitest'

import {
  type CalcInput,
  type ScrollCounts,
  SCROLLS,
  calcStat,
  effectiveAtk,
  effectiveStat,
  getStarForceStats,
} from '../../../src/services/equipCalculator'

/**
 * Convenience: a {@link ScrollCounts} populated with zeroes.
 *
 * Most calc tests only exercise one or two scroll fields at a
 * time; building a full record from scratch in every test would
 * bury the actual fixture under boilerplate. The factory mirrors
 * the WPF default state where every `t_*Num` `TextBox` starts
 * empty (parsed as 0) and the user fills in only the row(s)
 * they care about.
 */
const zeroCounts = (): ScrollCounts => ({
  destiny: 0,
  glory: 0,
  black: 0,
  v: 0,
  x: 0,
  red: 0,
  jd: 0,
  sm: 0,
  bm: 0,
  scrollStat: 0,
  scrollAtk: 0,
})

/**
 * Build a full {@link CalcInput} with sane WPF-default values
 * and the caller's overrides merged on top. The defaults
 * mirror the XAML's initial state: Weapon / Lv150 / not
 * superior / Average random-type for both Destiny and Glory /
 * every numeric field at 0 and starForce at 0.
 */
const buildInput = (overrides: Partial<CalcInput> = {}): CalcInput => ({
  eqpType: 'Weapon',
  reqLev: 150,
  superior: false,
  baseStat: 0,
  flameStat: 0,
  baseAtk: 0,
  flameAtk: 0,
  starForce: 0,
  destinyType: 1,
  gloryType: 1,
  counts: zeroCounts(),
  ...overrides,
})

/* ------------------------------------------------------------------ */
/* SCROLLS table — verbatim parity                                     */
/* ------------------------------------------------------------------ */

describe('SCROLLS', () => {
  it('matches WPF Destiny ranges (4-arg ScrollStat ctor)', () => {
    expect(SCROLLS.Destiny.weapon).toEqual({
      statMin: 14,
      statMax: 20,
      atkMin: 14,
      atkMax: 20,
    })
    expect(SCROLLS.Destiny.armor).toEqual({
      statMin: 0,
      statMax: 0,
      atkMin: 9,
      atkMax: 15,
    })
    expect(SCROLLS.Destiny.accessory).toEqual({
      statMin: 0,
      statMax: 0,
      atkMin: 9,
      atkMax: 15,
    })
  })

  it('matches WPF Glory ranges (4-arg ScrollStat ctor)', () => {
    expect(SCROLLS.Glory.weapon).toEqual({
      statMin: 10,
      statMax: 20,
      atkMin: 10,
      atkMax: 20,
    })
    expect(SCROLLS.Glory.armor).toEqual({
      statMin: 0,
      statMax: 0,
      atkMin: 5,
      atkMax: 15,
    })
  })

  it('preserves the WPF 2-arg ctor: SM.Armor stat=5, atk=1; BM.Armor stat=5, atk=0', () => {
    /*
     * These values look like a WPF data error (armor scrolls
     * giving both stat and atk?) but the source code declares
     * them this way (L117-123). Pinning them in a test guards
     * against well-meaning future cleanup that would silently
     * change a player's calculator output.
     */
    expect(SCROLLS.SM.armor).toEqual({
      statMin: 5,
      statMax: 5,
      atkMin: 1,
      atkMax: 1,
    })
    expect(SCROLLS.BM.armor).toEqual({
      statMin: 5,
      statMax: 5,
      atkMin: 0,
      atkMax: 0,
    })
  })

  it('matches WPF Black weapon ScrollStat(14, 14)', () => {
    expect(SCROLLS.Black.weapon).toEqual({
      statMin: 14,
      statMax: 14,
      atkMin: 14,
      atkMax: 14,
    })
  })
})

/* ------------------------------------------------------------------ */
/* effectiveStat / effectiveAtk                                        */
/* ------------------------------------------------------------------ */

describe('effectiveStat / effectiveAtk', () => {
  it('returns the min on randomType=0', () => {
    expect(effectiveStat(SCROLLS.Glory.weapon, 0)).toBe(10)
    expect(effectiveAtk(SCROLLS.Glory.weapon, 0)).toBe(10)
  })

  it('returns the truncated average on randomType=1', () => {
    /*
     * (14 + 20) / 2 = 17 (matches WPF's `(byte)((min+max)/2)`
     * for Destiny weapon). (5 + 15) / 2 = 10 for armor atk.
     */
    expect(effectiveStat(SCROLLS.Destiny.weapon, 1)).toBe(17)
    expect(effectiveAtk(SCROLLS.Destiny.weapon, 1)).toBe(17)
    expect(effectiveAtk(SCROLLS.Glory.armor, 1)).toBe(10)
  })

  it('returns the max on randomType=2', () => {
    expect(effectiveStat(SCROLLS.Glory.weapon, 2)).toBe(20)
    expect(effectiveAtk(SCROLLS.Glory.weapon, 2)).toBe(20)
  })

  it('returns the constant value when min === max regardless of randomType', () => {
    /*
     * SM.Weapon is `ScrollStat(5, 7)` (2-arg ctor → stat=5, atk=7
     * for both min and max), so all three random types yield
     * the same number. This documents that the SPA can pass
     * any randomType for the 7 fixed-yield scrolls without
     * affecting the result.
     */
    expect(effectiveStat(SCROLLS.SM.weapon, 0)).toBe(5)
    expect(effectiveStat(SCROLLS.SM.weapon, 1)).toBe(5)
    expect(effectiveStat(SCROLLS.SM.weapon, 2)).toBe(5)
    expect(effectiveAtk(SCROLLS.SM.weapon, 1)).toBe(7)
  })
})

/* ------------------------------------------------------------------ */
/* getStarForceStats — superior branch                                 */
/* ------------------------------------------------------------------ */

describe('getStarForceStats — superior', () => {
  it('returns static stat values for SF 0-4 (WPF L655-674)', () => {
    expect(getStarForceStats(true, 'Glove', 0, 0, 150)).toEqual({ stat: 19, atk: 0 })
    expect(getStarForceStats(true, 'Glove', 1, 0, 150)).toEqual({ stat: 20, atk: 0 })
    expect(getStarForceStats(true, 'Glove', 2, 0, 150)).toEqual({ stat: 22, atk: 0 })
    expect(getStarForceStats(true, 'Glove', 3, 0, 150)).toEqual({ stat: 25, atk: 0 })
    expect(getStarForceStats(true, 'Glove', 4, 0, 150)).toEqual({ stat: 29, atk: 0 })
  })

  it('returns SF + 4 atk for SF 5-9 (WPF L675-682)', () => {
    expect(getStarForceStats(true, 'Armor', 5, 0, 150).atk).toBe(9)
    expect(getStarForceStats(true, 'Armor', 7, 0, 150).atk).toBe(11)
    expect(getStarForceStats(true, 'Armor', 9, 0, 150).atk).toBe(13)
  })

  it('returns 15 + 2*(SF-10) atk for SF 10-14 (WPF L683-690)', () => {
    expect(getStarForceStats(true, 'Glove', 10, 0, 150).atk).toBe(15)
    expect(getStarForceStats(true, 'Glove', 12, 0, 150).atk).toBe(19)
    expect(getStarForceStats(true, 'Glove', 14, 0, 150).atk).toBe(23)
  })

  it('ignores eqpType / reqLev / atk under the superior branch', () => {
    /*
     * The superior branch returns early, so the eqpType /
     * reqLev / atk arguments are inert. This test pins that
     * invariant — useful when the SPA's `superior` watcher
     * forces other fields into "any value" territory mid-flight.
     */
    expect(getStarForceStats(true, 'Weapon', 0, 999, 200).stat).toBe(19)
    expect(getStarForceStats(true, 'Heart', 5, 999, 200).atk).toBe(9)
  })
})

/* ------------------------------------------------------------------ */
/* getStarForceStats — Weapon branch                                   */
/* ------------------------------------------------------------------ */

describe('getStarForceStats — Weapon', () => {
  it('SF<15 yields tiered all-stats and floor(atk/50)+1 atk delta', () => {
    /*
     * SF=0 with atk=0 → stat=2 (range 0-4), atk=floor(0/50)+1=1.
     * SF=14 with atk=100 → stat=3 (range 5-14), atk=floor(100/50)+1=3.
     */
    expect(getStarForceStats(false, 'Weapon', 0, 0, 150)).toEqual({ stat: 2, atk: 1 })
    expect(getStarForceStats(false, 'Weapon', 14, 100, 150)).toEqual({ stat: 3, atk: 3 })
  })

  it('SF=15 atk varies by reqLev (WPF L734-741)', () => {
    expect(getStarForceStats(false, 'Weapon', 15, 0, 150)).toEqual({ stat: 11, atk: 8 })
    expect(getStarForceStats(false, 'Weapon', 15, 0, 160)).toEqual({ stat: 13, atk: 9 })
    expect(getStarForceStats(false, 'Weapon', 15, 0, 200)).toEqual({ stat: 15, atk: 13 })
  })

  it('SF=22 atk jumps to 31/32/34 by reqLev (WPF L788-794) but stat=0 (allStats out of range)', () => {
    /*
     * `else if (starForce < 22)` excludes SF=22, so allStats
     * falls into the trailing `else { allStats = 0; }` arm
     * (WPF L716-719). The atk lookup still credits the SF=22
     * row though.
     */
    expect(getStarForceStats(false, 'Weapon', 22, 0, 150)).toEqual({ stat: 0, atk: 31 })
    expect(getStarForceStats(false, 'Weapon', 22, 0, 160)).toEqual({ stat: 0, atk: 32 })
    expect(getStarForceStats(false, 'Weapon', 22, 0, 200)).toEqual({ stat: 0, atk: 34 })
  })

  it('SF=23 only credits atk at reqLev>=200 (WPF L796-800 one-sided guard)', () => {
    expect(getStarForceStats(false, 'Weapon', 23, 0, 150).atk).toBe(0)
    expect(getStarForceStats(false, 'Weapon', 23, 0, 160).atk).toBe(0)
    expect(getStarForceStats(false, 'Weapon', 23, 0, 200).atk).toBe(35)
  })
})

/* ------------------------------------------------------------------ */
/* getStarForceStats — Other (Armor / Accessory / Heart / Glove)        */
/* ------------------------------------------------------------------ */

describe('getStarForceStats — Other', () => {
  it('Armor SF=15 atk by reqLev (WPF L838-844)', () => {
    expect(getStarForceStats(false, 'Armor', 15, 0, 150)).toEqual({ stat: 11, atk: 9 })
    expect(getStarForceStats(false, 'Armor', 15, 0, 160)).toEqual({ stat: 13, atk: 10 })
    expect(getStarForceStats(false, 'Armor', 15, 0, 200)).toEqual({ stat: 15, atk: 12 })
  })

  it('Armor SF=24 atk uses the case-24 row absent from the weapon table (WPF L910-917)', () => {
    expect(getStarForceStats(false, 'Armor', 24, 0, 150).atk).toBe(22)
    expect(getStarForceStats(false, 'Armor', 24, 0, 200).atk).toBe(25)
  })

  it('Armor SF<15 yields no atk (only Glove gets the SF<15 bonus)', () => {
    expect(getStarForceStats(false, 'Armor', 4, 0, 150).atk).toBe(0)
    expect(getStarForceStats(false, 'Armor', 14, 0, 150).atk).toBe(0)
    expect(getStarForceStats(false, 'Accessory', 12, 0, 150).atk).toBe(0)
  })

  it('Heart uses the Other branch (WPF L693 only matches eqpTyp==0 for Weapon)', () => {
    /*
     * Heart eqpType (eqpTyp=4 in WPF) shares the *scroll
     * lookup* with Weapon (`eqpTyp == 0 || eqpTyp == 4` →
     * Weapon slot, L497) but **not** the *star-force*
     * branching — `getStarForceStats` only checks
     * `eqpTyp == 0` for the weapon branch. A Heart with
     * SF=15 reqLev=200 therefore yields the Other row's
     * atk=12, not the Weapon row's atk=13.
     */
    expect(getStarForceStats(false, 'Heart', 15, 0, 200)).toEqual({ stat: 15, atk: 12 })
    expect(getStarForceStats(false, 'Heart', 14, 0, 200)).toEqual({ stat: 3, atk: 0 })
  })

  it('Glove SF<15 special atk credits (WPF L922-947)', () => {
    expect(getStarForceStats(false, 'Glove', 4, 0, 150).atk).toBe(1)
    expect(getStarForceStats(false, 'Glove', 6, 0, 150).atk).toBe(1)
    expect(getStarForceStats(false, 'Glove', 12, 0, 150).atk).toBe(1)
    /* Case 5 / 7 / 9 / 11 stay at 0 (no switch arm) */
    expect(getStarForceStats(false, 'Glove', 5, 0, 150).atk).toBe(0)
    expect(getStarForceStats(false, 'Glove', 11, 0, 150).atk).toBe(0)
  })

  it('Glove SF=13 atk only credited at reqLev>=200', () => {
    expect(getStarForceStats(false, 'Glove', 13, 0, 150).atk).toBe(0)
    expect(getStarForceStats(false, 'Glove', 13, 0, 160).atk).toBe(0)
    expect(getStarForceStats(false, 'Glove', 13, 0, 200).atk).toBe(1)
  })

  it('Glove SF=14 atk inverts at reqLev>=200 (downward tilt)', () => {
    /*
     * WPF L938-943: `value = reqLev >= 200 ? 1 : 2`. The
     * higher-tier glove gets *less* per SF=14 tick, which is
     * a deliberate balancing tilt (documented in the algorithm
     * docblock).
     */
    expect(getStarForceStats(false, 'Glove', 14, 0, 150).atk).toBe(2)
    expect(getStarForceStats(false, 'Glove', 14, 0, 200).atk).toBe(1)
  })
})

/* ------------------------------------------------------------------ */
/* calcStat — integration                                              */
/* ------------------------------------------------------------------ */

describe('calcStat — base / flame split', () => {
  it('returns all zeros for an empty input', () => {
    expect(calcStat(buildInput())).toEqual({
      totalStat: 0,
      addedStat: 0,
      totalAtk: 0,
      addedAtk: 0,
    })
  })

  it('flame stats only contribute to totalStat / totalAtk, never addedStat / addedAtk', () => {
    /*
     * WPF L633-636: addedStat = stat - baseStat (so flame
     * doesn't cancel into "added"). Pinning this prevents
     * regressions where a refactor "simplifies" by reusing
     * stat / atk for both readouts.
     */
    expect(calcStat(buildInput({ baseStat: 10, baseAtk: 20, flameStat: 3, flameAtk: 5 }))).toEqual({
      totalStat: 13,
      addedStat: 0,
      totalAtk: 25,
      addedAtk: 0,
    })
  })
})

describe('calcStat — scrolls', () => {
  it('Glory armor avg adds 10 atk per sheet, 0 stat', () => {
    /*
     * Glory.Armor.atk avg = (5 + 15) / 2 = 10. Stat is 0 since
     * Glory armor has min=max=0 stat.
     */
    const result = calcStat(
      buildInput({
        eqpType: 'Armor',
        counts: { ...zeroCounts(), glory: 1 },
        gloryType: 1,
      }),
    )
    expect(result.addedStat).toBe(0)
    expect(result.addedAtk).toBe(10)
  })

  it('Glory weapon min adds 10 stat and 10 atk per sheet (randomType=0 honoured)', () => {
    const result = calcStat(
      buildInput({
        eqpType: 'Weapon',
        counts: { ...zeroCounts(), glory: 1 },
        gloryType: 0,
      }),
    )
    expect(result.addedStat).toBe(10)
    expect(result.addedAtk).toBe(10)
  })

  it('Other-row scrollStat / scrollAtk are added once without sheet count', () => {
    /*
     * The "Other" scroll row has no count column (XAML L762-825);
     * its inputs are the per-application yield directly. Pinning
     * that the calc folds them in once (not zero, not multiplied)
     * guards against future "DRY-it-up" attempts that might add
     * a phantom sheet count.
     */
    const result = calcStat(
      buildInput({
        counts: { ...zeroCounts(), scrollStat: 7, scrollAtk: 11 },
      }),
    )
    expect(result.addedStat).toBe(7)
    expect(result.addedAtk).toBe(11)
  })

  it('Heart eqpType reuses the Weapon scroll slot (WPF L497 eqpTyp==0||4)', () => {
    /*
     * Destiny.Weapon avg = 17 stat / 17 atk. Destiny.Armor /
     * Destiny.Accessory have stat=0. If the .ts implementation
     * ever drifted to "Heart uses Armor slot", this test would
     * collapse stat to 0.
     */
    const result = calcStat(
      buildInput({
        eqpType: 'Heart',
        counts: { ...zeroCounts(), destiny: 1 },
        destinyType: 1,
      }),
    )
    expect(result.addedStat).toBe(17)
    expect(result.addedAtk).toBe(17)
  })
})

describe('calcStat — star-force loop', () => {
  it('Weapon SF=1 with atk=0 yields +2 stat / +1 atk (single tick)', () => {
    expect(calcStat(buildInput({ starForce: 1 }))).toEqual({
      totalStat: 2,
      addedStat: 2,
      totalAtk: 1,
      addedAtk: 1,
    })
  })

  it('Weapon SF=2 with baseAtk=100 threads running atk back into floor(atk/50)+1', () => {
    /*
     * Manual trace:
     *   start: atk=100
     *   i=0: SF=0 → stat+=2, atk+=floor(100/50)+1=3  (atk=103)
     *   i=1: SF=1 → stat+=2, atk+=floor(103/50)+1=3  (atk=106)
     * Final: stat=4, atk=106 → addedAtk=6.
     *
     * If the implementation forgets to thread `atk` and always
     * passes the initial 100 in, both ticks would still yield 3,
     * so this test wouldn't catch that bug — but a higher-atk
     * scenario where the floor crosses a 50-boundary would. We
     * keep this small case as a sanity check; the next test
     * exercises a boundary cross.
     */
    expect(calcStat(buildInput({ starForce: 2, baseAtk: 100 }))).toEqual({
      totalStat: 4,
      addedStat: 4,
      totalAtk: 106,
      addedAtk: 6,
    })
  })

  it('Weapon SF=2 with baseAtk=49 crosses the floor(atk/50) boundary on tick 1', () => {
    /*
     * start: atk=49
     *   i=0: SF=0 → stat+=2, atk+=floor(49/50)+1=1   (atk=50)
     *   i=1: SF=1 → stat+=2, atk+=floor(50/50)+1=2   (atk=52)
     * Final: addedAtk=3, addedStat=4.
     *
     * If the loop ever stopped threading `atk` and always
     * passed the initial 49, both ticks would yield 1 each
     * (addedAtk=2). This test pins the threading.
     */
    expect(calcStat(buildInput({ starForce: 2, baseAtk: 49 }))).toEqual({
      totalStat: 4,
      addedStat: 4,
      totalAtk: 52,
      addedAtk: 3,
    })
  })

  it('Armor SF=15 reqLev=200 sums tiered stat per range; atk only credited from SF>=15 ticks', () => {
    /*
     * Manual trace for armor SF=15 (loop runs i=0..14):
     *   i=0..4 (5 ticks): each SF in 0-4 → allStats=2, atk=0
     *     stat += 2 * 5 = 10
     *   i=5..14 (10 ticks): each SF in 5-14 → allStats=3, atk=0
     *     stat += 3 * 10 = 30
     * Final: stat=40, atk=0. (Note: the atk=12 SF=15 row is
     * for SF=15 ENTERING; here SF=15 is the *target*, so the
     * loop runs i=0..14 only — SF=15 itself is not entered.)
     */
    expect(calcStat(buildInput({ eqpType: 'Armor', starForce: 15, reqLev: 200 }))).toEqual({
      totalStat: 40,
      addedStat: 40,
      totalAtk: 0,
      addedAtk: 0,
    })
  })

  it('Superior glove SF=10 sums the documented stat / atk per-tick yields', () => {
    /*
     * Manual trace (loop i=0..9):
     *   i=0..4: stat += 19, 20, 22, 25, 29 → 115
     *   i=5..9: atk += 9, 10, 11, 12, 13   → 55
     */
    expect(
      calcStat(
        buildInput({
          eqpType: 'Glove',
          superior: true,
          reqLev: 150,
          starForce: 10,
        }),
      ),
    ).toEqual({
      totalStat: 115,
      addedStat: 115,
      totalAtk: 55,
      addedAtk: 55,
    })
  })

  it('Glove SF=14 reqLev=150 picks up case 4/6/8/10/12 atk=1 + case 14 atk=2', () => {
    /*
     * Loop i=0..13 (SF=14 target, so case 14 is NOT entered):
     *   stat: SF 0-4 → 2 each (5 ticks = 10)
     *         SF 5-13 → 3 each (9 ticks = 27)
     *         total = 37
     *   atk:  SF 4 → 1, SF 6 → 1, SF 8 → 1, SF 10 → 1, SF 12 → 1
     *         (case 13 skipped at reqLev=150)
     *         total = 5
     */
    expect(calcStat(buildInput({ eqpType: 'Glove', starForce: 14, reqLev: 150 }))).toEqual({
      totalStat: 37,
      addedStat: 37,
      totalAtk: 5,
      addedAtk: 5,
    })
  })
})
