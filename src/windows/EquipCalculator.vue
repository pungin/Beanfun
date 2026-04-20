<script setup lang="ts">
/**
 * Equipment Star-Force calculator dialog (P12.5 D5 + D6).
 *
 * # Scope of D5 vs D6
 *
 * - **D5**: dialog shell, every form input the WPF window
 *   exposes, the visibility / cross-coupling side effects between
 *   `EquipType` ↔ `Superior` ↔ `ReqLev`, and the 9 scroll PNG
 *   thumbnails copied verbatim from `Beanfun/Resources/`.
 * - **D6**: the pure scroll-stat / star-force math pulled out of
 *   `Beanfun/Windows/EquipCalculator.xaml.cs::calcStat` (L443-637)
 *   and `getStarForceStats` (L639-950) into a sibling
 *   {@link ../services/equipCalculator.ts} module so the
 *   algorithm can be unit-tested without mounting this dialog.
 *   The four readout labels (`lbl_TotalStat` / `lbl_AddedStat` /
 *   `lbl_TotalATK` / `lbl_AddedATK`) read off a single
 *   {@link calcResult} computed that folds every input on this
 *   file through that pure algorithm.
 *
 * Splitting the work this way kept each commit reviewable: the
 * D5 diff was "9 PNG assets + 380 LoC of layout / state with no
 * runtime math worth reviewing"; the D6 diff is "wire the
 * algorithm into the existing refs" with the heavy lifting
 * already covered by `services/equipCalculator.spec.ts`.
 *
 * # WPF parity (`Beanfun/Windows/EquipCalculator.xaml(.cs)`)
 *
 * Mirrors the legacy window's three logical regions:
 *
 * 1. **Equipment-type / level / superior selectors** (XAML L18-104,
 *    code-behind L143-224):
 *    - 5 `RadioButton`s for equipment type
 *      (`Weapon` / `Glove` / `Armor` / `Accessory` / `Heart`).
 *    - Red `Heart` notice that's only visible when `Heart` is
 *      selected (XAML L62-67 sets `Visibility="Collapsed"` and the
 *      `rb_EqpTyp_IsCheckedChanged` handler L152-154 toggles it).
 *    - 3 `RadioButton`s for required level (150 / 160 / 200).
 *    - `Superior` checkbox whose visibility tracks
 *      `Lv150 && (Glove || Armor)` (handler L148-151) and whose
 *      `Checked` event further constrains the level radios:
 *      enabling Superior hides Lv160/Lv200 and forces Lv150
 *      (handler L208-217). Disabling Superior re-shows Lv160/Lv200
 *      (L219-222).
 * 2. **Stat / ATK / StarForce input row** (XAML L105-342):
 *    - Six numeric `TextBox`es plus four readonly result labels
 *      (`lbl_TotalStat` / `lbl_AddedStat` / `lbl_TotalATK` /
 *      `lbl_AddedATK`).
 *    - StarForce text box + a `lbl_StarForceMax` label that
 *      reads "25" by default and "15" while Superior is on
 *      (handler L207).
 * 3. **Scroll table** (XAML L345-826):
 *    - 10 rows: 9 named scrolls (`Destiny`, `Glory`, `Black`,
 *      `V`, `X`, `Red`, `JD`, `SM`, `BM`) plus one "Others"
 *      catch-all that takes both a `Stat` and an `Atk` value.
 *    - Per-row layout: image (32 px) + label + count input
 *      (+ for Destiny / Glory: a `Min / Average / Max`
 *      `RandomType` toggle).
 *    - The `Destiny` row in WPF reuses `Scroll_Glory.png`
 *      (XAML L347-352); not a typo — the legacy author never
 *      shipped a separate Destiny icon. We keep the same reuse
 *      so visual parity survives the port.
 *
 * # Mockup conflict resolution (P12.5 plan, user-approved)
 *
 * `mockups/EquipCalculator.html` proposes a brand-new card-based
 * UI with quick-fill presets, scroll cost projection, and a
 * weighted "expected stat per cost" optimizer. None of that
 * exists in WPF or in any backend payload, and the user rule for
 * P12.5 is "WPF 沒有的拒絕". The mockup chrome is dropped; this
 * file renders the same three-region WPF layout (selectors panel
 * left, scroll table right) with the SPA's design-token glass
 * styling so it visually matches `MapleTools` / `CoreCalculator`.
 *
 * # WPF `GotFocus` clear-on-focus → SPA `placeholder` (D5 plan q5)
 *
 * Each WPF `TextBox` has a `*_GotFocus` handler that wipes the
 * text on first focus, paired with a `VisualBrush` style trigger
 * that paints "0" while the box is blurred and empty. That whole
 * dance simulates an HTML-style `placeholder`. We swap the dance
 * for an actual `placeholder="0"` on every `el-input-number` —
 * functional parity preserved, 14 dead `*_GotFocus` handlers
 * deleted (SRP/DRY win, not a behaviour change). This also means
 * "0" the user typed is preserved across blur/focus, where WPF
 * would silently wipe it on next focus — the WPF behaviour was a
 * UX bug nobody filed because the value didn't matter (a 0 stat
 * input contributes 0 to the result either way), so removing the
 * wipe doesn't change the calculator's output.
 *
 * # Why `el-input-number :controls="false"` instead of `el-input`
 *
 * The WPF widgets are bare `TextBox`es with no spinners; the SPA
 * needs strict `number | null` binding so the D6 algorithm can
 * fold the inputs without re-parsing. `el-input` with
 * `type="number"` would force string ↔ number juggling at every
 * read site; `el-input-number :controls="false"` keeps the value
 * model strict while hiding the increment/decrement chrome WPF
 * never had.
 *
 * # Caller wiring
 *
 * ```vue
 * <EquipCalculator v-model:visible="equipCalcOpen" />
 * ```
 *
 * Stateless from the parent's perspective — every selection /
 * input lives in the dialog's own refs, mirroring WPF where the
 * `Window` instance owns its UI state for the lifetime of the
 * window. Closing and re-opening preserves nothing across mounts
 * (the dialog is `destroy-on-close`, same as `CoreCalculator`).
 */

import { computed, reactive, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { ElCheckbox, ElDialog, ElIcon, ElInputNumber, ElRadio, ElRadioGroup } from 'element-plus'
import { CircleClose, MagicStick } from '@element-plus/icons-vue'

import { type CalcResult, calcStat } from '../services/equipCalculator'
import scrollBlackPng from '../assets/scrolls/Scroll_Black.png'
import scrollBmPng from '../assets/scrolls/Scroll_BM.png'
import scrollGloryPng from '../assets/scrolls/Scroll_Glory.png'
import scrollJdPng from '../assets/scrolls/Scroll_JD.png'
import scrollOtherPng from '../assets/scrolls/Scroll_Other.png'
import scrollRedPng from '../assets/scrolls/Scroll_Red.png'
import scrollSmPng from '../assets/scrolls/Scroll_SM.png'
import scrollVPng from '../assets/scrolls/Scroll_V.png'
import scrollXPng from '../assets/scrolls/Scroll_X.png'

defineOptions({ name: 'EquipCalculatorDialog' })

defineProps<{
  /**
   * Two-way visibility binding (`v-model:visible`). Same
   * convention as every other `windows/*.vue`.
   */
  visible: boolean
}>()

const emit = defineEmits<{
  (event: 'update:visible', next: boolean): void
}>()

const { t } = useI18n()

/* ------------------------------------------------------------------ */
/* Equipment / level / superior selectors                              */
/* ------------------------------------------------------------------ */

/**
 * Equipment type radio bound. Defaults to `Weapon` to mirror
 * `rb_Weapon.IsChecked="True"` (XAML L21).
 */
type EqpType = 'Weapon' | 'Glove' | 'Armor' | 'Accessory' | 'Heart'
const eqpType = ref<EqpType>('Weapon')

/**
 * Required level radio bound. Defaults to 150 to mirror
 * `rb_Lv150.IsChecked="True"` (XAML L71).
 */
type ReqLev = 150 | 160 | 200
const reqLev = ref<ReqLev>(150)

/**
 * Superior checkbox bound. Defaults to false (`cb_Superior` has
 * no `IsChecked` attribute in XAML L96-104, so the default is
 * unchecked).
 */
const superior = ref(false)

/**
 * Visibility of the Superior checkbox. Mirrors `rb_EqpTyp_IsCheckedChanged`
 * L148-151 + `rb_ReqLev_IsCheckedChanged` L162-171:
 *
 *     showSuperior = rb_Lv150.IsChecked
 *                 && (rb_Glove.IsChecked || rb_Armor.IsChecked)
 */
const showSuperior = computed(
  (): boolean => reqLev.value === 150 && (eqpType.value === 'Glove' || eqpType.value === 'Armor'),
)

/**
 * Visibility of the Heart-only red notice. Mirrors
 * `rb_EqpTyp_IsCheckedChanged` L152-154.
 */
const showHeartNotice = computed((): boolean => eqpType.value === 'Heart')

/**
 * Visibility of the Lv160 / Lv200 radios. Mirrors
 * `cb_Superior_IsCheckedChanged` L210-211 / L220-221: enabling
 * Superior hides them; disabling re-shows them. The radio bound
 * value is independently constrained by the `watch(superior)`
 * below (forces back to 150) so even if a programmatic write
 * tried to slip 160/200 in while Superior is on, the watcher
 * would correct it.
 */
const showHigherLevels = computed((): boolean => !superior.value)

/**
 * StarForce-max label text. Mirrors `cb_Superior_IsCheckedChanged`
 * L207: `superior ? "15" : "25"`.
 */
const starForceMax = computed((): number => (superior.value ? 15 : 25))

/*
 * Side-effect watchers — these mirror the three WPF
 * `*_IsCheckedChanged` handlers (L143-224) that synchronise the
 * three coupled flags (eqpType ↔ reqLev ↔ superior).
 *
 * Vue's reactivity makes the watchers cheaper than WPF's manual
 * routed events: every change triggers exactly one watcher pass
 * regardless of how many selectors moved, where WPF's handler
 * ran a full re-evaluation per radio click. Functional parity
 * preserved.
 */

/**
 * Mirror `rb_EqpTyp_IsCheckedChanged` L147 (`cb_Superior.IsChecked = false`).
 *
 * WPF resets unconditionally when *any* equipment-type radio
 * changes. The next-line visibility recompute (L148-151) is
 * already covered by the `showSuperior` computed; we only need
 * to reset the bound here. Resetting whether or not Superior
 * was previously visible matches the WPF behaviour exactly:
 * even if the new type still shows the checkbox, the previous
 * Superior selection is wiped (e.g. Glove → Armor still
 * un-checks Superior).
 */
watch(eqpType, () => {
  superior.value = false
})

/**
 * Mirror `rb_ReqLev_IsCheckedChanged` L162-169: when the new
 * reqLev makes Superior invisible, also reset its bound. Same
 * defensive intent as WPF — keep the bound state in sync with
 * the visible state so the D6 algorithm doesn't see a "Superior
 * = true" combined with a Lv200 input that the UI never let the
 * user produce together.
 */
watch(reqLev, () => {
  if (!showSuperior.value) {
    superior.value = false
  }
})

/**
 * Mirror `cb_Superior_IsCheckedChanged` L208-217: enabling
 * Superior forces `rb_Lv150.IsChecked = true`. In WPF this
 * mattered because the user could enable Superior while Lv150
 * happened to be off (the checkbox was visible because the
 * predicate `Lv150 && (Glove || Armor)` had been satisfied at
 * mount time and never re-evaluated until the next radio
 * change). Our `showSuperior` computed already only renders the
 * checkbox when reqLev === 150, so this branch is technically
 * unreachable from the UI — but we keep it for parity in case
 * a future store-driven write programmatically flips the bound.
 */
watch(superior, (next) => {
  if (next && reqLev.value !== 150) {
    reqLev.value = 150
  }
})

/* ------------------------------------------------------------------ */
/* Stat / ATK / StarForce inputs                                       */
/* ------------------------------------------------------------------ */

/*
 * `null` is the empty / placeholder state — `el-input-number`
 * renders the `placeholder="0"` glyph when the bound is null,
 * matching the WPF `VisualBrush` painted "0" when the textbox
 * was blurred and empty (see module docblock for details).
 *
 * The {@link calcResult} computed below folds every value
 * through a `numOrZero` helper so the algorithm sees `0` for
 * empty inputs, mirroring WPF's `int.TryParse(text, out int n)`
 * returning `0` on parse fail.
 */
const baseStat = ref<number | null>(null)
const flameStat = ref<number | null>(null)
const baseAtk = ref<number | null>(null)
const flameAtk = ref<number | null>(null)
const starForce = ref<number | null>(null)

/* ------------------------------------------------------------------ */
/* Scroll table                                                        */
/* ------------------------------------------------------------------ */

/**
 * Per-row metadata for the 10-row scroll table. The `id` is the
 * shared key into `scrollNums` (for the 9 named scrolls) and the
 * `image` is the asset import — bundled by Vite so the Tauri
 * webview can resolve them without round-tripping `commands.openUrl`.
 *
 * Every WPF entry from XAML L347-419 is preserved verbatim in
 * order; the `Destiny` row deliberately reuses `scrollGloryPng`
 * because the WPF `Image Source` did the same (see module docblock
 * → "Mockup conflict resolution" / "WPF parity" sections).
 */
type NamedScrollId = 'Destiny' | 'Glory' | 'Black' | 'V' | 'X' | 'Red' | 'JD' | 'SM' | 'BM'

interface ScrollRow {
  readonly id: NamedScrollId | 'Other'
  readonly labelKey: string
  readonly image: string
  /**
   * Whether the row exposes the `Min / Average / Max` random-type
   * radios. Only `Destiny` and `Glory` do (XAML L453-474 /
   * L508-529); `cb_Superior_IsCheckedChanged` and friends never
   * touch them.
   */
  readonly hasRandomType: boolean
}

const SCROLL_ROWS: readonly ScrollRow[] = [
  { id: 'Destiny', labelKey: 'ScrollDestiny', image: scrollGloryPng, hasRandomType: true },
  { id: 'Glory', labelKey: 'ScrollGlory', image: scrollGloryPng, hasRandomType: true },
  { id: 'Black', labelKey: 'ScrollBlack', image: scrollBlackPng, hasRandomType: false },
  { id: 'V', labelKey: 'ScrollV', image: scrollVPng, hasRandomType: false },
  { id: 'X', labelKey: 'ScrollX', image: scrollXPng, hasRandomType: false },
  { id: 'Red', labelKey: 'ScrollRed', image: scrollRedPng, hasRandomType: false },
  { id: 'JD', labelKey: 'ScrollJiDian', image: scrollJdPng, hasRandomType: false },
  { id: 'SM', labelKey: 'ScrollSuMin', image: scrollSmPng, hasRandomType: false },
  { id: 'BM', labelKey: 'ScrollChuanShuo', image: scrollBmPng, hasRandomType: false },
  { id: 'Other', labelKey: 'ScrollOthers', image: scrollOtherPng, hasRandomType: false },
] as const

/**
 * Per-named-scroll count input bound. `Other` is excluded
 * because that row carries two values (`scrollStat` + `scrollAtk`)
 * and is handled separately in the template.
 */
const scrollNums = reactive<Record<NamedScrollId, number | null>>({
  Destiny: null,
  Glory: null,
  Black: null,
  V: null,
  X: null,
  Red: null,
  JD: null,
  SM: null,
  BM: null,
})

/**
 * Per-scroll random-type radios for `Destiny` and `Glory`. The
 * value is `0` (Min) | `1` (Average) | `2` (Max), matching the
 * WPF `byte` cast in `rb_DestinyType_IsCheckedChanged` L179-185 /
 * `rb_GloryType_IsCheckedChanged` L193-199.
 *
 * Defaults to `1` (Average) to mirror `rb_DestinyAverage.IsChecked`
 * / `rb_GloryAverage.IsChecked` (XAML L466 / L521).
 */
type RandomType = 0 | 1 | 2
const destinyType = ref<RandomType>(1)
const gloryType = ref<RandomType>(1)

/**
 * "Others" scroll bound (the last row of the table). WPF binds
 * `t_ScrollStat` (XAML L763) + `t_ScrollATK` (L795) — two
 * independent textboxes; the row has no count column because the
 * stat / atk values *are* what gets applied per scroll. The
 * algorithm reads these as a per-scroll value added once, with
 * no sheet-count multiplier (mirroring WPF L555-557 / L483-491
 * in `calcStat`).
 */
const scrollStat = ref<number | null>(null)
const scrollAtk = ref<number | null>(null)

/* ------------------------------------------------------------------ */
/* Result labels                                                       */
/* ------------------------------------------------------------------ */

/**
 * Coerce a possibly-empty `el-input-number` value to the `0` the
 * algorithm expects. Mirrors WPF's `int.TryParse(text, out int n)`
 * pattern in `calcStat` (L334-491) where every textbox returns
 * `0` on parse fail (empty, non-numeric, or whitespace).
 *
 * Kept inline in this file — moving it to a shared util would be
 * premature DRY (it's a 1-line helper used in exactly one place
 * and the SPA's only `null`-permitting numeric inputs live here).
 */
const numOrZero = (value: number | null): number => value ?? 0

/**
 * Single source of truth for the four readout labels. Folding
 * every input ref through one `calcStat` call (instead of four
 * separate computeds that each rebuild the input bundle) means
 * the algorithm runs once per dependency change — Vue's
 * reactivity caches the `CalcResult` and the four label
 * computeds below are O(1) destructures.
 *
 * Mirrors WPF's `calcStat` flow (L314-637): every input read
 * synchronously, fold them through the pure `calcStat`, then
 * paint the four `Content` properties on the result labels. WPF
 * recomputes on every `_TextChanged` / `_IsCheckedChanged` event
 * (L143-313); Vue's reactivity does the same on ref change with
 * no manual subscription wiring.
 */
const calcResult = computed(
  (): CalcResult =>
    calcStat({
      eqpType: eqpType.value,
      reqLev: reqLev.value,
      /*
       * Mirrors WPF L330-331: `cb_Superior.IsChecked && cb_Superior.Visibility == Visible`.
       * Our `superior` ref is already kept in sync with visibility by
       * the `watch(eqpType)` / `watch(reqLev)` resets above, so a
       * second guard here would be redundant — but we keep the AND
       * with `showSuperior` defensively so any future watcher gap
       * can't leak a `superior=true` past an invisible checkbox.
       */
      superior: superior.value && showSuperior.value,
      baseStat: numOrZero(baseStat.value),
      flameStat: numOrZero(flameStat.value),
      baseAtk: numOrZero(baseAtk.value),
      flameAtk: numOrZero(flameAtk.value),
      starForce: numOrZero(starForce.value),
      destinyType: destinyType.value,
      gloryType: gloryType.value,
      counts: {
        destiny: numOrZero(scrollNums.Destiny),
        glory: numOrZero(scrollNums.Glory),
        black: numOrZero(scrollNums.Black),
        v: numOrZero(scrollNums.V),
        x: numOrZero(scrollNums.X),
        red: numOrZero(scrollNums.Red),
        jd: numOrZero(scrollNums.JD),
        sm: numOrZero(scrollNums.SM),
        bm: numOrZero(scrollNums.BM),
        scrollStat: numOrZero(scrollStat.value),
        scrollAtk: numOrZero(scrollAtk.value),
      },
    }),
)

const totalStat = computed((): number => calcResult.value.totalStat)
const addedStat = computed((): number => calcResult.value.addedStat)
const totalAtk = computed((): number => calcResult.value.totalAtk)
const addedAtk = computed((): number => calcResult.value.addedAtk)

/* ------------------------------------------------------------------ */
/* Dialog visibility plumbing                                          */
/* ------------------------------------------------------------------ */

function handleClose(): void {
  emit('update:visible', false)
}

function handleVisibleChange(value: boolean): void {
  emit('update:visible', value)
}
</script>

<template>
  <el-dialog
    :model-value="visible"
    :width="720"
    :close-on-click-modal="false"
    :close-on-press-escape="true"
    :show-close="false"
    align-center
    append-to-body
    destroy-on-close
    class="equip-calc-dialog"
    data-test="equip-calculator-dialog"
    @update:model-value="handleVisibleChange"
  >
    <template #header>
      <div class="equip-calc__header">
        <div class="equip-calc__header-meta">
          <el-icon class="equip-calc__header-icon" :size="20">
            <MagicStick />
          </el-icon>
          <span class="equip-calc__header-title" data-test="equip-calculator-title">
            {{ t('EquipStarForceCaculator') }}
          </span>
        </div>
        <button
          type="button"
          class="equip-calc__header-close"
          :title="t('Cancel')"
          data-test="equip-calculator-close"
          @click="handleClose"
        >
          <el-icon><CircleClose /></el-icon>
        </button>
      </div>
    </template>

    <div class="equip-calc__body">
      <!-- Left panel: type / level / superior / stat-atk-starforce inputs -->
      <section
        class="equip-calc__panel equip-calc__selectors"
        data-test="equip-calculator-selectors"
      >
        <div class="equip-calc__row">
          <span class="equip-calc__field-label">{{ t('EquipType') }}</span>
          <el-radio-group v-model="eqpType" data-test="equip-calculator-eqp-type">
            <el-radio value="Weapon" data-test="equip-calculator-eqp-weapon">{{
              t('Weapon')
            }}</el-radio>
            <el-radio value="Glove" data-test="equip-calculator-eqp-glove">{{
              t('Glove')
            }}</el-radio>
            <el-radio value="Armor" data-test="equip-calculator-eqp-armor">{{
              t('Armor')
            }}</el-radio>
            <el-radio value="Accessory" data-test="equip-calculator-eqp-accessory">{{
              t('Accessory')
            }}</el-radio>
            <el-radio value="Heart" data-test="equip-calculator-eqp-heart">{{
              t('Heart')
            }}</el-radio>
          </el-radio-group>
        </div>

        <p
          v-if="showHeartNotice"
          class="equip-calc__heart-notice"
          data-test="equip-calculator-heart-notice"
        >
          {{ t('HeartNotice') }}
        </p>

        <div class="equip-calc__row">
          <span class="equip-calc__field-label equip-calc__field-label--accent">REQ LEV:</span>
          <el-radio-group v-model="reqLev" data-test="equip-calculator-req-lev">
            <el-radio :value="150" data-test="equip-calculator-req-lev-150">150</el-radio>
            <el-radio v-if="showHigherLevels" :value="160" data-test="equip-calculator-req-lev-160"
              >160</el-radio
            >
            <el-radio v-if="showHigherLevels" :value="200" data-test="equip-calculator-req-lev-200"
              >200</el-radio
            >
          </el-radio-group>
        </div>

        <el-checkbox
          v-if="showSuperior"
          v-model="superior"
          class="equip-calc__superior"
          data-test="equip-calculator-superior"
        >
          {{ t('Superior') }}
        </el-checkbox>

        <!-- Stat row: base + flame -> total / added readout -->
        <div class="equip-calc__readout-row" data-test="equip-calculator-stat-row">
          <span class="equip-calc__field-label">{{ t('Stat') }}</span>
          <span class="equip-calc__readout-symbol equip-calc__readout-symbol--accent">+</span>
          <span class="equip-calc__readout-value" data-test="equip-calculator-total-stat">
            {{ totalStat }}
          </span>
          <span class="equip-calc__readout-paren">(</span>
          <el-input-number
            v-model="baseStat"
            :controls="false"
            :min="0"
            :step="1"
            placeholder="0"
            class="equip-calc__readout-input"
            data-test="equip-calculator-base-stat"
          />
          <span class="equip-calc__readout-symbol equip-calc__readout-symbol--flame">+</span>
          <el-input-number
            v-model="flameStat"
            :controls="false"
            :min="0"
            :step="1"
            placeholder="0"
            class="equip-calc__readout-input equip-calc__readout-input--flame"
            data-test="equip-calculator-flame-stat"
          />
          <span class="equip-calc__readout-symbol equip-calc__readout-symbol--accent">+</span>
          <span class="equip-calc__readout-value" data-test="equip-calculator-added-stat">
            {{ addedStat }}
          </span>
          <span class="equip-calc__readout-paren">)</span>
        </div>

        <!-- ATK row: base + flame -> total / added readout -->
        <div class="equip-calc__readout-row" data-test="equip-calculator-atk-row">
          <span class="equip-calc__field-label">{{ t('Atk_Matk_') }}</span>
          <span class="equip-calc__readout-symbol equip-calc__readout-symbol--accent">+</span>
          <span class="equip-calc__readout-value" data-test="equip-calculator-total-atk">
            {{ totalAtk }}
          </span>
          <span class="equip-calc__readout-paren">(</span>
          <el-input-number
            v-model="baseAtk"
            :controls="false"
            :min="0"
            :step="1"
            placeholder="0"
            class="equip-calc__readout-input"
            data-test="equip-calculator-base-atk"
          />
          <span class="equip-calc__readout-symbol equip-calc__readout-symbol--flame">+</span>
          <el-input-number
            v-model="flameAtk"
            :controls="false"
            :min="0"
            :step="1"
            placeholder="0"
            class="equip-calc__readout-input equip-calc__readout-input--flame"
            data-test="equip-calculator-flame-atk"
          />
          <span class="equip-calc__readout-symbol equip-calc__readout-symbol--accent">+</span>
          <span class="equip-calc__readout-value" data-test="equip-calculator-added-atk">
            {{ addedAtk }}
          </span>
          <span class="equip-calc__readout-paren">)</span>
        </div>

        <!-- StarForce row: input + max-cap label sandwich -->
        <div class="equip-calc__readout-row" data-test="equip-calculator-star-force-row">
          <span class="equip-calc__field-label">{{ t('StarForce') }}</span>
          <el-input-number
            v-model="starForce"
            :controls="false"
            :min="0"
            :max="starForceMax"
            :step="1"
            placeholder="0"
            class="equip-calc__readout-input"
            data-test="equip-calculator-star-force"
          />
          <span class="equip-calc__field-label">{{ t('StarForceSplit') }}</span>
          <span class="equip-calc__readout-value" data-test="equip-calculator-star-force-max">
            {{ starForceMax }}
          </span>
          <span class="equip-calc__field-label">{{ t('StarsInFused') }}</span>
        </div>
      </section>

      <!-- Right panel: scroll table (image + label + count input + opt random radios) -->
      <section class="equip-calc__panel equip-calc__scrolls" data-test="equip-calculator-scrolls">
        <div
          v-for="row in SCROLL_ROWS"
          :key="row.id"
          class="equip-calc__scroll-row"
          :data-test="`equip-calculator-scroll-row-${row.id}`"
        >
          <img :src="row.image" :alt="t(row.labelKey)" class="equip-calc__scroll-image" />
          <span class="equip-calc__scroll-label">{{ t(row.labelKey) }}</span>

          <!-- Named scrolls: single count input -->
          <template v-if="row.id !== 'Other'">
            <el-input-number
              v-model="scrollNums[row.id]"
              :controls="false"
              :min="0"
              :step="1"
              placeholder="0"
              class="equip-calc__scroll-input"
              :data-test="`equip-calculator-scroll-num-${row.id}`"
            />
            <span class="equip-calc__scroll-suffix">{{ t('Sheet') }}</span>

            <!-- Destiny / Glory only: Min / Average / Max selector -->
            <el-radio-group
              v-if="row.hasRandomType && row.id === 'Destiny'"
              v-model="destinyType"
              class="equip-calc__scroll-random"
              :data-test="`equip-calculator-scroll-random-${row.id}`"
            >
              <el-radio :value="0" data-test="equip-calculator-destiny-min">{{
                t('GloryMin')
              }}</el-radio>
              <el-radio :value="1" data-test="equip-calculator-destiny-average">{{
                t('GloryAverage')
              }}</el-radio>
              <el-radio :value="2" data-test="equip-calculator-destiny-max">{{
                t('GloryMax')
              }}</el-radio>
            </el-radio-group>
            <el-radio-group
              v-else-if="row.hasRandomType && row.id === 'Glory'"
              v-model="gloryType"
              class="equip-calc__scroll-random"
              :data-test="`equip-calculator-scroll-random-${row.id}`"
            >
              <el-radio :value="0" data-test="equip-calculator-glory-min">{{
                t('GloryMin')
              }}</el-radio>
              <el-radio :value="1" data-test="equip-calculator-glory-average">{{
                t('GloryAverage')
              }}</el-radio>
              <el-radio :value="2" data-test="equip-calculator-glory-max">{{
                t('GloryMax')
              }}</el-radio>
            </el-radio-group>
          </template>

          <!-- Others: stat + atk pair (no count column, mirrors WPF L762-825) -->
          <template v-else>
            <el-input-number
              v-model="scrollStat"
              :controls="false"
              :min="0"
              :step="1"
              placeholder="0"
              class="equip-calc__scroll-input"
              data-test="equip-calculator-scroll-stat"
            />
            <span class="equip-calc__scroll-suffix">{{ t('Stat') }}</span>
            <el-input-number
              v-model="scrollAtk"
              :controls="false"
              :min="0"
              :step="1"
              placeholder="0"
              class="equip-calc__scroll-input"
              data-test="equip-calculator-scroll-atk"
            />
            <span class="equip-calc__scroll-suffix">{{ t('Atk_Matk') }}</span>
          </template>
        </div>
      </section>
    </div>
  </el-dialog>
</template>

<style scoped>
.equip-calc__header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.75rem;
}

.equip-calc__header-meta {
  display: inline-flex;
  align-items: center;
  gap: 0.625rem;
  min-width: 0;
}

.equip-calc__header-icon {
  color: var(--bf-primary);
  flex-shrink: 0;
}

.equip-calc__header-title {
  font-size: 0.9375rem;
  font-weight: 700;
  color: var(--bf-on-surface);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.equip-calc__header-close {
  appearance: none;
  border: 0;
  background: transparent;
  width: 28px;
  height: 28px;
  display: grid;
  place-items: center;
  border-radius: var(--bf-radius-input);
  color: var(--bf-on-surface-variant);
  cursor: pointer;
  transition:
    background var(--bf-motion-fast),
    color var(--bf-motion-fast);
}

.equip-calc__header-close:hover {
  background: color-mix(in srgb, var(--bf-danger) 80%, transparent);
  color: var(--bf-on-danger);
}

/*
 * Two-column layout mirroring WPF's `<DockPanel><Border/>(left)
 * <DockPanel/>(right)</DockPanel>` outer structure. Min-width 0
 * on each cell prevents the scroll table's wide rows from
 * pushing the selectors panel beyond the dialog width.
 */
.equip-calc__body {
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(0, 1.2fr);
  gap: 0.75rem;
  padding: 0.25rem;
}

.equip-calc__panel {
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
  padding: 0.625rem 0.75rem;
  background: color-mix(in srgb, var(--bf-surface) 60%, transparent);
  border: 1px solid var(--bf-outline-variant);
  border-radius: var(--bf-radius-input);
  min-width: 0;
}

.equip-calc__selectors {
  gap: 0.625rem;
}

.equip-calc__row {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 0.5rem;
}

.equip-calc__field-label {
  font-size: 0.8125rem;
  color: var(--bf-on-surface-variant);
  white-space: nowrap;
}

/*
 * `--accent` mirrors WPF's `Foreground="Orange"` for the REQ LEV
 * marker (XAML L69) — the only label in the panel that the
 * legacy UI deliberately tints to flag a coupled-state knob.
 */
.equip-calc__field-label--accent {
  color: var(--bf-warning, #ff9800);
  font-weight: 600;
}

/*
 * Heart notice: `Foreground="Red"` in WPF (XAML L65). The SPA
 * uses the design-token `--bf-danger` for theme-respecting
 * contrast; the warning meaning is preserved.
 */
.equip-calc__heart-notice {
  margin: 0;
  font-size: 0.8125rem;
  color: var(--bf-danger);
}

.equip-calc__superior {
  margin: 0;
}

/*
 * The Stat / ATK / StarForce rows are visually a single line of
 * mixed text + inputs (mirrors WPF's `<StackPanel Orientation="Horizontal">`
 * with literal "+" / "(" / ")" `Label`s between bound widgets).
 * `flex-wrap: wrap` lets the row break gracefully if the dialog
 * width drops; WPF's `SizeToContent` would just expand instead.
 */
.equip-calc__readout-row {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 0.25rem;
}

.equip-calc__readout-symbol {
  font-size: 0.875rem;
  font-weight: 600;
  color: var(--bf-on-surface-variant);
}

.equip-calc__readout-symbol--accent {
  color: var(--bf-primary);
}

/*
 * Flame stat / atk values are tinted `LightGreen` in WPF (XAML
 * L150-184 / L243-277). We mirror the semantic with the
 * design-token success colour so the per-row contrast survives
 * theme switching.
 */
.equip-calc__readout-symbol--flame {
  color: var(--bf-success, #4caf50);
}

.equip-calc__readout-value {
  font-size: 0.875rem;
  font-weight: 600;
  color: var(--bf-primary);
  min-width: 1.5rem;
  text-align: center;
}

.equip-calc__readout-paren {
  font-size: 0.875rem;
  color: var(--bf-on-surface-variant);
}

.equip-calc__readout-input {
  width: 64px;
}

.equip-calc__readout-input--flame :deep(.el-input__inner) {
  color: var(--bf-success, #4caf50);
}

/*
 * Scroll table: a flat vertical stack of rows. WPF used three
 * parallel `StackPanel`s (image column, label column, input
 * column) with `Height="32"` per child — visually equivalent
 * to a flex row per scroll, which we adopt here for a tighter
 * DOM tree and easier responsive wrapping.
 */
.equip-calc__scrolls {
  gap: 0.375rem;
}

.equip-calc__scroll-row {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  flex-wrap: wrap;
}

.equip-calc__scroll-image {
  width: 32px;
  height: 32px;
  flex-shrink: 0;
  /*
   * `RenderOptions.BitmapScalingMode="NearestNeighbor"` (XAML
   * L350 et al.) — preserves the pixel-art aesthetic of the
   * original 32 × 32 PNGs at any zoom level. The CSS equivalent
   * is `image-rendering: pixelated`.
   */
  image-rendering: pixelated;
  object-fit: contain;
}

.equip-calc__scroll-label {
  font-size: 0.8125rem;
  color: var(--bf-on-surface);
  min-width: 6rem;
}

.equip-calc__scroll-input {
  width: 56px;
}

.equip-calc__scroll-suffix {
  font-size: 0.8125rem;
  color: var(--bf-on-surface-variant);
}

/*
 * The Min / Average / Max selector for Destiny / Glory rows
 * lives inline at the row's tail; tighten the inter-radio gap
 * so the row doesn't blow past the panel's right edge on narrow
 * viewports.
 */
.equip-calc__scroll-random :deep(.el-radio) {
  margin-right: 0.25rem;
}

/*
 * el-input-number with controls="false" still applies inner
 * padding meant for the spinner buttons. Tighten it so the
 * compact 56-64px slots breathe correctly inside the row layout.
 */
.equip-calc__readout-input :deep(.el-input__inner),
.equip-calc__scroll-input :deep(.el-input__inner) {
  padding-left: 0.375rem;
  padding-right: 0.375rem;
  text-align: center;
}
</style>
