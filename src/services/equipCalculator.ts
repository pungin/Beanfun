/**
 * Pure helpers for the MapleStory equip Star-Force calculator
 * (P12.5 D6).
 *
 * # WPF parity
 *
 * Direct port of `Beanfun/Windows/EquipCalculator.xaml.cs`:
 *
 * - `Scrolls` static class L69-125 → {@link SCROLLS}
 * - `ScrollStat.Stat` / `.Atk` getters L23-43 → {@link effectiveStat} /
 *   {@link effectiveAtk}
 * - `getStarForceStats` L639-950 → {@link getStarForceStats}
 * - `calcStat` L314-637 → {@link calcStat}
 *
 * Every numeric constant, inequality bound, and switch-case is
 * preserved literally; only the data shapes change (`Dictionary<int,int>`
 * → `StarForceDelta` object, C# `byte` / `int` → TS `number`,
 * `int.Parse(...)` try/catch → upstream `numOrZero` coercion).
 *
 * # Why this lives in `services/` instead of inside the .vue
 *
 * The algorithm is framework-agnostic — it consumes plain numbers
 * and an enum, and returns plain numbers. Pulling it out of the
 * Vue component lets us unit-test every star-force lookup row
 * without mounting the dialog (mirrors the
 * `services/coreCalculator.ts` ↔ `windows/CoreCalculator.vue`
 * split from D4). The reactive shell stays in
 * `windows/EquipCalculator.vue`; the math has no business
 * touching `ref()` / `useI18n()`.
 *
 * # Why we don't simplify the lookup tables
 *
 * The `getStarForceStats` switch table looks repetitive (8-9
 * `case N: value = reqLev >= 200 ? X : reqLev >= 160 ? Y : Z`
 * arms in a row) and is *almost* a closed-form polynomial, but
 * the WPF data has off-by-one quirks at SF 22 (weapon: 31/32/34
 * skips 33 entirely; armor: 18/19/21 skips 20) that no closed
 * form captures. Preserving the lookup table verbatim is the
 * only safe port; condensing it would silently change values a
 * player has memorised.
 *
 * # Why we don't expose individual scroll types as separate consts
 *
 * The `SCROLLS` const is a single keyed record so callers can
 * iterate it (e.g. for a "show all scroll yields" debug panel)
 * without listing each one by name. Per-scroll consts would be
 * tightly coupled to the calling code and DRY-violating.
 *
 * # Numeric semantics
 *
 * - WPF uses `byte` (0-255) for scroll counts / star-force /
 *   flame stats; `int` for base stats / atk / scrollStat /
 *   scrollTK. Empty inputs in WPF parse via `int.Parse` /
 *   `byte.Parse` with try/catch → `0` on fail. The SPA's
 *   `el-input-number :min="0"` clamps the byte fields to the
 *   non-negative range and the `numOrZero` helper in the .vue
 *   coerces `null` → `0` so the algorithm here always sees
 *   plain numbers (no `null` / `NaN` defence needed).
 * - The SPA UI clamps every numeric input to `:min="0"` while
 *   WPF would let `int.Parse` accept negatives for
 *   `baseStat` / `baseAtk` / `scrollStat` / `scrollAtk`. The
 *   WPF behaviour for negatives was undefined (the `floor(atk/50)`
 *   weapon formula yields perverse values for negative atk),
 *   so the SPA's clamp is a UX guard rather than a parity
 *   regression — the algorithm itself still accepts any
 *   `number` and propagates it deterministically.
 * - Integer division in C# truncates toward zero. We use
 *   `Math.trunc` for the `(min + max) / 2` average and
 *   `Math.floor` for `atk / 50` — both match WPF for the
 *   non-negative inputs the UI permits. (`Math.trunc` and
 *   `Math.floor` diverge for negatives; `floor` is what WPF's
 *   `int / int` does for negative dividend, but again the UI
 *   clamps so the divergence is unreachable in practice.)
 *
 * # Side effects
 *
 * Pure: no I/O, no closures over external state, no exceptions
 * thrown. Output is deterministic for any input.
 */

/**
 * Equipment type the user is enhancing. Maps 1:1 to WPF's
 * `byte eqpTyp` (L318-324):
 *
 *   0 = Weapon, 1 = Glove, 2 = Armor, 3 = Accessory, 4 = Heart
 *
 * String names are easier to read than the WPF byte codes; the
 * algorithm branches on the string identity instead of numeric
 * equality so the WPF intent ("Weapon-or-Heart" / "Glove" /
 * "Accessory" / "Armor") survives the port.
 */
export type EqpType = 'Weapon' | 'Glove' | 'Armor' | 'Accessory' | 'Heart'

/**
 * Required level radio bound — the three thresholds the
 * star-force lookup tables branch on (`reqLev >= 200` /
 * `>= 160` / else). Mirrors `rb_Lv150` / `rb_Lv160` / `rb_Lv200`
 * group (XAML L69-94).
 */
export type ReqLev = 150 | 160 | 200

/**
 * Per-scroll random-type radio bound. Maps to WPF's
 * `Scrolls.Destiny.Weapon.RandomType` byte (`0` / `1` / `2`):
 *
 *   0 = Min  → returns `*Min`
 *   1 = Avg  → returns `(min + max) / 2` (truncated)
 *   2 = Max  → returns `*Max`
 *
 * Only `Destiny` and `Glory` actually consult this in the SPA
 * UI; the other 7 scrolls all have `min === max` so the random
 * type doesn't affect their effective contribution. We still
 * pass `0` to those scrolls' lookups for code uniformity.
 */
export type RandomType = 0 | 1 | 2

/**
 * Per-equipment-slot stat range for one scroll. Mirrors WPF's
 * `ScrollStat` class (L19-66) with the two constructor variants
 * collapsed: when WPF used the 2-arg `ScrollStat(stat, atk)`
 * form, both `*Min` and `*Max` are set to the same value, so
 * the random-type getter returns the constant regardless. The
 * 4-arg `ScrollStat(statMin, statMax, atkMin, atkMax)` form is
 * only used for `Destiny` and `Glory` (the two scrolls with
 * actual ranges) and is preserved verbatim.
 */
export interface ScrollStat {
  readonly statMin: number
  readonly statMax: number
  readonly atkMin: number
  readonly atkMax: number
}

/**
 * One scroll's per-equipment-slot yields. Each property
 * corresponds to WPF's `Scroll.Weapon` / `.Armor` / `.Accessory`
 * (L13-17). Heart-type equipment uses the `weapon` slot at
 * lookup time (matches WPF L497 `eqpTyp == 0 || eqpTyp == 4`).
 */
export interface Scroll {
  readonly weapon: ScrollStat
  readonly armor: ScrollStat
  readonly accessory: ScrollStat
}

/**
 * The 9 named scroll IDs, plus 2 free-form `scrollStat` /
 * `scrollAtk` inputs in {@link ScrollCounts}, that the
 * calculator folds in. Order matches WPF's static class field
 * declaration (L71-79).
 */
export type NamedScrollId = 'Destiny' | 'Glory' | 'Black' | 'V' | 'X' | 'Red' | 'JD' | 'SM' | 'BM'

/**
 * Number of sheets the player has applied per scroll, plus the
 * "Other" row's free-form stat / atk pair (the last row of the
 * scroll table — XAML L762-825 — which has no count column;
 * the value is the per-scroll yield directly).
 *
 * Every property is required so the algorithm doesn't have to
 * guard against `undefined`; the .vue caller always builds a
 * complete object via {@link buildCounts}-style spread.
 */
export interface ScrollCounts {
  readonly destiny: number
  readonly glory: number
  readonly black: number
  readonly v: number
  readonly x: number
  readonly red: number
  readonly jd: number
  readonly sm: number
  readonly bm: number
  /** "Other" scroll: per-application stat. Mirrors WPF `t_ScrollStat` (XAML L763). */
  readonly scrollStat: number
  /** "Other" scroll: per-application atk. Mirrors WPF `t_ScrollATK` (XAML L795). */
  readonly scrollAtk: number
}

/**
 * Full input bundle for {@link calcStat}. Maps 1:1 to the
 * `calcStat()` local-variable block in WPF (L318-491).
 */
export interface CalcInput {
  readonly eqpType: EqpType
  readonly reqLev: ReqLev
  readonly superior: boolean
  readonly baseStat: number
  readonly flameStat: number
  readonly baseAtk: number
  readonly flameAtk: number
  readonly starForce: number
  readonly destinyType: RandomType
  readonly gloryType: RandomType
  readonly counts: ScrollCounts
}

/**
 * The four readout labels the WPF window paints into
 * `lbl_TotalStat` / `lbl_AddedStat` / `lbl_TotalATK` /
 * `lbl_AddedATK` (XAML L109-191 / L201-284).
 */
export interface CalcResult {
  readonly totalStat: number
  readonly addedStat: number
  readonly totalAtk: number
  readonly addedAtk: number
}

/**
 * Per-step contribution from a single star-force enhancement
 * tick. Mirrors WPF's `Dictionary<int, int>` return shape from
 * `getStarForceStats` (L647-649) — key `1` was the stat delta
 * and key `2` was the atk delta. We use a typed object instead
 * of a numeric-keyed map for safer access at the call sites.
 */
export interface StarForceDelta {
  readonly stat: number
  readonly atk: number
}

/* ------------------------------------------------------------------ */
/* Scroll constant table — verbatim port of WPF L83-123               */
/* ------------------------------------------------------------------ */

/**
 * The 9 named scrolls and their per-slot yields. Each entry
 * mirrors WPF's `Scrolls.<Name>.<Slot> = new ScrollStat(...)`
 * initialiser literally:
 *
 * - 4-arg `ScrollStat(statMin, statMax, atkMin, atkMax)` →
 *   the four range fields populated independently
 *   (only `Destiny` / `Glory`).
 * - 2-arg `ScrollStat(stat, atk)` → both `min` and `max` of
 *   each pair set to the same value (WPF L52-58 constructor).
 *
 * `Destiny` / `Glory` keep their default `RandomType = 1`
 * (Average) inside the `Scrolls` static-ctor block (L86-95),
 * but the SPA defers that bound to the .vue's `destinyType` /
 * `gloryType` refs and passes whichever the user picked into
 * {@link calcStat}. The other 7 scrolls have `min === max` so
 * the `randomType` argument doesn't change their yield.
 */
export const SCROLLS: Record<NamedScrollId, Scroll> = {
  Destiny: {
    weapon: { statMin: 14, statMax: 20, atkMin: 14, atkMax: 20 },
    armor: { statMin: 0, statMax: 0, atkMin: 9, atkMax: 15 },
    accessory: { statMin: 0, statMax: 0, atkMin: 9, atkMax: 15 },
  },
  Glory: {
    weapon: { statMin: 10, statMax: 20, atkMin: 10, atkMax: 20 },
    armor: { statMin: 0, statMax: 0, atkMin: 5, atkMax: 15 },
    accessory: { statMin: 0, statMax: 0, atkMin: 5, atkMax: 15 },
  },
  Black: {
    weapon: { statMin: 14, statMax: 14, atkMin: 14, atkMax: 14 },
    armor: { statMin: 2, statMax: 2, atkMin: 9, atkMax: 9 },
    accessory: { statMin: 0, statMax: 0, atkMin: 9, atkMax: 9 },
  },
  V: {
    weapon: { statMin: 11, statMax: 11, atkMin: 13, atkMax: 13 },
    armor: { statMin: 0, statMax: 0, atkMin: 8, atkMax: 8 },
    accessory: { statMin: 0, statMax: 0, atkMin: 8, atkMax: 8 },
  },
  X: {
    weapon: { statMin: 10, statMax: 10, atkMin: 12, atkMax: 12 },
    armor: { statMin: 0, statMax: 0, atkMin: 7, atkMax: 7 },
    accessory: { statMin: 0, statMax: 0, atkMin: 7, atkMax: 7 },
  },
  Red: {
    weapon: { statMin: 8, statMax: 8, atkMin: 10, atkMax: 10 },
    armor: { statMin: 0, statMax: 0, atkMin: 5, atkMax: 5 },
    accessory: { statMin: 0, statMax: 0, atkMin: 5, atkMax: 5 },
  },
  JD: {
    weapon: { statMin: 5, statMax: 5, atkMin: 9, atkMax: 9 },
    armor: { statMin: 0, statMax: 0, atkMin: 4, atkMax: 4 },
    accessory: { statMin: 0, statMax: 0, atkMin: 4, atkMax: 4 },
  },
  /*
   * SM and BM swap the conventional "stat scrolls give stat,
   * armor gets stat slot" model: WPF's 2-arg ctor for
   * `SM.Armor = ScrollStat(5, 1)` sets stat=5 AND atk=1 on
   * armor (L117-119), and `BM.Armor = ScrollStat(5, 0)` sets
   * stat=5 AND atk=0 on armor (L121-123). This looks like a
   * data error but is what the WPF tool ships, so we preserve
   * it byte-for-byte and let the test suite document the
   * unusual values.
   */
  SM: {
    weapon: { statMin: 5, statMax: 5, atkMin: 7, atkMax: 7 },
    armor: { statMin: 5, statMax: 5, atkMin: 1, atkMax: 1 },
    accessory: { statMin: 5, statMax: 5, atkMin: 1, atkMax: 1 },
  },
  BM: {
    weapon: { statMin: 4, statMax: 4, atkMin: 7, atkMax: 7 },
    armor: { statMin: 5, statMax: 5, atkMin: 0, atkMax: 0 },
    accessory: { statMin: 5, statMax: 5, atkMin: 0, atkMax: 0 },
  },
}

/* ------------------------------------------------------------------ */
/* Helpers                                                             */
/* ------------------------------------------------------------------ */

/**
 * Effective stat yield for a scroll given the user's
 * random-type selection. Mirrors WPF's `ScrollStat.Stat` getter
 * (L23-31).
 */
export function effectiveStat(stat: ScrollStat, randomType: RandomType): number {
  if (randomType === 0) return stat.statMin
  if (randomType === 1) return Math.trunc((stat.statMin + stat.statMax) / 2)
  return stat.statMax
}

/**
 * Effective atk yield for a scroll given the user's
 * random-type selection. Mirrors WPF's `ScrollStat.Atk` getter
 * (L35-43).
 */
export function effectiveAtk(stat: ScrollStat, randomType: RandomType): number {
  if (randomType === 0) return stat.atkMin
  if (randomType === 1) return Math.trunc((stat.atkMin + stat.atkMax) / 2)
  return stat.atkMax
}

/**
 * Pick the right per-equipment-slot {@link ScrollStat} for the
 * user's equipment-type selection. Mirrors WPF's tri-arity
 * pattern that appears 18 times in `calcStat` (L495-622, e.g.
 * `eqpTyp == 0 || eqpTyp == 4 ? Scrolls.X.Weapon : eqpTyp == 3
 * ? Scrolls.X.Accessory : Scrolls.X.Armor`).
 *
 * Heart sharing the Weapon slot is intentional WPF behaviour
 * (the heart-type equip applies weapon-class scrolls, per the
 * red `HeartNotice` label in XAML L62-67).
 */
function scrollForType(scroll: Scroll, eqpType: EqpType): ScrollStat {
  if (eqpType === 'Weapon' || eqpType === 'Heart') return scroll.weapon
  if (eqpType === 'Accessory') return scroll.accessory
  return scroll.armor
}

/* ------------------------------------------------------------------ */
/* Star-Force per-tick lookup                                          */
/* ------------------------------------------------------------------ */

/**
 * Compute the stat / atk delta from one star-force enhancement
 * tick. Direct port of `getStarForceStats` (L639-950).
 *
 * The function is *deltas only* — the caller is responsible for
 * threading the running atk back in for the weapon SF<15 branch
 * (the WPF `floor(atk/50)+1` formula uses the *current* running
 * atk total, which grows as previous ticks contribute).
 *
 * # Branch summary
 *
 * 1. **Superior** (heart / glove / armor with `cb_Superior`
 *    checked, only valid up to SF=14 — the SPA caps the input
 *    via `lbl_StarForceMax` in the .vue): SF 0-4 yield static
 *    stat values; SF 5-9 yield linear `SF + 4` atk; SF 10-14
 *    yield `15 + 2 * (SF - 10)` atk.
 * 2. **Weapon** (`eqpType === 'Weapon'`): tiered all-stats by
 *    SF range + reqLev; SF<15 atk uses `floor(atk/50) + 1`;
 *    SF 15-23 atk uses a per-row lookup table (with a SF=23
 *    quirk that only credits atk at reqLev>=200).
 * 3. **Other** (everything except Weapon, including Heart):
 *    same tiered all-stats; SF>=15 uses a different lookup
 *    table with one more row (case 24) and slightly different
 *    values; SF<15 only Glove gets a small atk credit on
 *    even-numbered SF + 13/14.
 */
export function getStarForceStats(
  superior: boolean,
  eqpType: EqpType,
  starForce: number,
  atk: number,
  reqLev: ReqLev,
): StarForceDelta {
  let stat = 0
  let atkDelta = 0

  if (superior) {
    /* 尊貴裝 — WPF L650-692 */
    switch (starForce) {
      case 0:
        stat = 19
        break
      case 1:
        stat = 20
        break
      case 2:
        stat = 22
        break
      case 3:
        stat = 25
        break
      case 4:
        stat = 29
        break
      case 5:
      case 6:
      case 7:
      case 8:
      case 9:
        atkDelta = starForce + 4
        break
      case 10:
      case 11:
      case 12:
      case 13:
      case 14:
        atkDelta = 15 + 2 * (starForce - 10)
        break
      /*
       * SF >= 15 unreachable in the SPA when superior is on
       * (the .vue `starForceMax` computed clamps to 15). WPF
       * also has no case arm here so the defaults stay 0/0.
       */
    }
    return { stat, atk: atkDelta }
  }

  if (eqpType === 'Weapon') {
    /* 武器 — WPF L693-803 */
    let allStats: number
    if (starForce >= 0 && starForce < 5) allStats = 2
    else if (starForce >= 5 && starForce < 15) allStats = 3
    else if (starForce < 22) {
      if (reqLev >= 200) allStats = 15
      else if (reqLev >= 160) allStats = 13
      else allStats = 11
    } else allStats = 0
    stat = allStats

    if (starForce < 15) {
      /*
       * Running-atk-dependent formula. WPF L727:
       *   stats[2] = (int)Math.Floor(atk / 50.0D) + 1
       * `Math.floor` matches C# `Math.Floor(double)` for the
       * non-negative atk values the SPA permits.
       */
      atkDelta = Math.floor(atk / 50) + 1
    } else {
      let value = 0
      switch (starForce) {
        case 15:
          value = reqLev >= 200 ? 13 : reqLev >= 160 ? 9 : 8
          break
        case 16:
          value = reqLev >= 200 ? 13 : 9
          break
        case 17:
          value = reqLev >= 200 ? 14 : reqLev >= 160 ? 10 : 9
          break
        case 18:
          value = reqLev >= 200 ? 14 : reqLev >= 160 ? 11 : 10
          break
        case 19:
          value = reqLev >= 200 ? 15 : reqLev >= 160 ? 12 : 11
          break
        case 20:
          value = reqLev >= 200 ? 16 : reqLev >= 160 ? 13 : 12
          break
        case 21:
          value = reqLev >= 200 ? 17 : reqLev >= 160 ? 14 : 13
          break
        case 22:
          value = reqLev >= 200 ? 34 : reqLev >= 160 ? 32 : 31
          break
        case 23:
          /*
           * WPF L796-800: only sets value at reqLev>=200, falls
           * through to the default `value = 0` otherwise.
           * Preserved literally; no other case mid-table has
           * the same one-sided guard.
           */
          if (reqLev >= 200) value = 35
          break
      }
      atkDelta = value
    }
    return { stat, atk: atkDelta }
  }

  /* 其他裝備 — WPF L805-947 */
  let allStats: number
  if (starForce >= 0 && starForce < 5) allStats = 2
  else if (starForce >= 5 && starForce < 15) allStats = 3
  else if (starForce < 22) {
    if (reqLev >= 200) allStats = 15
    else if (reqLev >= 160) allStats = 13
    else allStats = 11
  } else allStats = 0
  stat = allStats

  if (starForce >= 15) {
    let value = 0
    switch (starForce) {
      case 15:
        value = reqLev >= 200 ? 12 : reqLev >= 160 ? 10 : 9
        break
      case 16:
        value = reqLev >= 200 ? 13 : reqLev >= 160 ? 11 : 10
        break
      case 17:
        value = reqLev >= 200 ? 14 : reqLev >= 160 ? 12 : 11
        break
      case 18:
        value = reqLev >= 200 ? 15 : reqLev >= 160 ? 13 : 12
        break
      case 19:
        value = reqLev >= 200 ? 16 : reqLev >= 160 ? 14 : 13
        break
      case 20:
        value = reqLev >= 200 ? 17 : reqLev >= 160 ? 15 : 14
        break
      case 21:
        value = reqLev >= 200 ? 19 : reqLev >= 160 ? 17 : 16
        break
      case 22:
        value = reqLev >= 200 ? 21 : reqLev >= 160 ? 19 : 18
        break
      case 23:
        value = reqLev >= 200 ? 23 : reqLev >= 160 ? 21 : 20
        break
      case 24:
        value = reqLev >= 200 ? 25 : reqLev >= 160 ? 23 : 22
        break
    }
    atkDelta = value
  } else if (eqpType === 'Glove') {
    /*
     * Glove SF<15 special atk credits — WPF L922-947. Note
     * that the case-14 arm yields 2 *unless* reqLev>=200, in
     * which case it yields 1 (a deliberate downward tilt for
     * the highest-level gloves to discourage over-starring).
     * Heart and Accessory and Armor get atk=0 in this branch
     * because the WPF code only enters the inner switch when
     * `eqpTyp == 1` (glove).
     */
    let value = 0
    switch (starForce) {
      case 4:
      case 6:
      case 8:
      case 10:
      case 12:
        value = 1
        break
      case 13:
        if (reqLev >= 200) value = 1
        break
      case 14:
        value = reqLev >= 200 ? 1 : 2
        break
    }
    atkDelta = value
  }

  return { stat, atk: atkDelta }
}

/* ------------------------------------------------------------------ */
/* Top-level calc                                                      */
/* ------------------------------------------------------------------ */

/**
 * Run the full equip Star-Force calculation for one input
 * bundle. Direct port of `calcStat` (L314-637).
 *
 * # Algorithm flow
 *
 * 1. Collect per-scroll yields (one `effectiveStat` /
 *    `effectiveAtk` lookup per scroll, weighted by the user's
 *    sheet count).
 * 2. Add the "Other" row's free-form `scrollStat` / `scrollAtk`
 *    once (no sheet-count multiplier — those values are the
 *    per-application yield directly).
 * 3. Iterate `starForce` times, accumulating per-tick deltas
 *    from {@link getStarForceStats}. The loop **must** thread
 *    the growing `atk` total back into each call because the
 *    weapon SF<15 branch derives its atk delta from the current
 *    running atk (`floor(atk/50)+1`).
 * 4. Compose the four output labels:
 *    - `totalStat = stat + flameStat`
 *    - `addedStat = stat - baseStat`
 *    - `totalAtk  = atk + flameAtk`
 *    - `addedAtk  = atk - baseAtk`
 *
 * `flameStat` / `flameAtk` are added only to the *total*
 * readouts, not the *added* readouts — mirrors WPF L633-636
 * exactly. The visual intent is "added = what the player
 * gained from scrolls + star force"; flame additions sit
 * outside that gain.
 */
export function calcStat(input: CalcInput): CalcResult {
  const {
    eqpType,
    reqLev,
    superior,
    baseStat,
    flameStat,
    baseAtk,
    flameAtk,
    starForce,
    destinyType,
    gloryType,
    counts,
  } = input

  const destiny = scrollForType(SCROLLS.Destiny, eqpType)
  const glory = scrollForType(SCROLLS.Glory, eqpType)
  const black = scrollForType(SCROLLS.Black, eqpType)
  const v = scrollForType(SCROLLS.V, eqpType)
  const x = scrollForType(SCROLLS.X, eqpType)
  const red = scrollForType(SCROLLS.Red, eqpType)
  const jd = scrollForType(SCROLLS.JD, eqpType)
  const sm = scrollForType(SCROLLS.SM, eqpType)
  const bm = scrollForType(SCROLLS.BM, eqpType)

  /*
   * Atk-side accumulation — mirrors WPF L493-553 in the same
   * order, with `randomType` deferred to `effectiveAtk` for
   * Destiny / Glory and pinned to `0` for the rest (their
   * `min === max` so the parameter is irrelevant).
   */
  let atk =
    baseAtk +
    counts.destiny * effectiveAtk(destiny, destinyType) +
    counts.glory * effectiveAtk(glory, gloryType) +
    counts.black * effectiveAtk(black, 0) +
    counts.v * effectiveAtk(v, 0) +
    counts.x * effectiveAtk(x, 0) +
    counts.red * effectiveAtk(red, 0) +
    counts.jd * effectiveAtk(jd, 0) +
    counts.sm * effectiveAtk(sm, 0) +
    counts.bm * effectiveAtk(bm, 0) +
    counts.scrollAtk

  /*
   * Stat-side accumulation — mirrors WPF L555-623 in the same
   * order. Same `randomType` pinning applies.
   */
  let stat =
    baseStat +
    counts.destiny * effectiveStat(destiny, destinyType) +
    counts.glory * effectiveStat(glory, gloryType) +
    counts.black * effectiveStat(black, 0) +
    counts.v * effectiveStat(v, 0) +
    counts.x * effectiveStat(x, 0) +
    counts.red * effectiveStat(red, 0) +
    counts.jd * effectiveStat(jd, 0) +
    counts.sm * effectiveStat(sm, 0) +
    counts.bm * effectiveStat(bm, 0) +
    counts.scrollStat

  /*
   * Star-force loop. WPF L626-631 iterates `i` from 0 to
   * `starForce - 1`, threading the *current* atk back into
   * each call (so the weapon SF<15 `floor(atk/50)+1` branch
   * sees the up-to-date running total). Preserved literally;
   * any deviation here would silently change the per-tick
   * weapon-atk yields and break parity for high-SF runs.
   */
  for (let i = 0; i < starForce; i++) {
    const delta = getStarForceStats(superior, eqpType, i, atk, reqLev)
    stat += delta.stat
    atk += delta.atk
  }

  return {
    totalStat: stat + flameStat,
    addedStat: stat - baseStat,
    totalAtk: atk + flameAtk,
    addedAtk: atk - baseAtk,
  }
}
