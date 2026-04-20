<script setup lang="ts">
/**
 * Perfect-Core calculator dialog (P12.5 D4).
 *
 * # WPF parity
 *
 * Mirrors `Beanfun/Windows/CoreCalculator.xaml(.cs)`:
 *
 * - Title `{DynamicResource PerfectCoreCaculator}` (same key
 *   even though the WPF source mis-spells "Calculator" as
 *   "Caculator" — every Maple-tools key reuses the typo so we
 *   don't drift the locale tree).
 * - Two `GroupBox`es on the left:
 *     1. `PerfectCoreNeedSkills` — input + Add/Delete + list
 *        of must-have skills.
 *     2. `PerfectCoreMyCores` — three `ComboBox`es (Main,
 *        Secondary, Secondary) + Add/Delete + list of cores.
 * - One `GroupBox` on the right (`PerfectCoreResult`) hosting
 *   a read-only `TextBox` whose initial content is `By:LinTx`
 *   (the original WPF author's signature line — kept literal,
 *   not localised).
 * - Top-right Calculator button reads `Caculator` initially
 *   and switches to `CaculatorCore({mustCount}, {skills})`
 *   after the user adds or removes a must-skill (mirroring
 *   `btn_AddMustSkill_Click` / `btn_DeleteMustSkill_Click`
 *   which only update the label as a side effect of the
 *   first user action).
 *
 * # Pure algorithm lives next door
 *
 * `services/coreCalculator.ts` owns the WPF-equivalent
 * `mustCoreCount` / `coreItemEquals` / `coreItemToString` /
 * `findPerfectCores` / `formatPerfectCoreResult` helpers so
 * the combinatorial core (no pun intended) is unit-testable
 * without mounting the dialog. This component supplies the
 * reactive UI shell + the i18n bindings; every numeric /
 * string-format decision routes through the helpers.
 *
 * # Validation: ElMessage.error vs WPF MessageBox.Show
 *
 * WPF's `errorMessage(...)` (L235-243) renders a blocking
 * `MessageBox.Show(message, "SystemInfo", OK, Error)`. The SPA
 * lifts that to `ElMessage.error(message)` non-blocking
 * toasts, matching the rest of `windows/*.vue` (e.g.
 * `AddServiceAccount.vue`, `UnconnectedGame_AddAccount.vue`).
 * The user gets immediate feedback without a second click —
 * functional parity is preserved (the dialog does not close
 * either way; the user re-enters and retries).
 *
 * # Add-Core validation: WPF C# operator precedence
 *
 * The WPF source (L88-93) writes the dedup check as:
 *
 *     s1==s2 || s1==s3 || s2==s3 && s2!="Others"
 *
 * In C# (and TS) `&&` binds tighter than `||`, so the
 * expression is `s1==s2 || s1==s3 || ((s2==s3) && (s2!="Others"))`
 * — meaning two secondary slots can repeat **only** when
 * both are the localized "Others" wildcard. This matters
 * because a player legitimately may have a core with two
 * "Others" secondaries (filler skills they don't care to
 * track). Preserved literally; the docblock here is the only
 * place that surfaces the precedence assumption so a future
 * refactor doesn't accidentally flip a paren.
 *
 * # "Others" numbering
 *
 * The Main combo lists `Others1 / Others2 / …` up to
 * `useOtherSkillCount + 1`. Whenever the user adds a core
 * whose `skill1` is the highest-numbered "Others" currently
 * available, `useOtherSkillCount` bumps so the next-N option
 * appears in the dropdown for the next core. Mirrors WPF L105-112
 * verbatim — without this counter the user would be capped at
 * one "Others" main slot per session even though the player
 * may legitimately have multiple cores with different
 * unrelated main skills they don't want to enumerate by name.
 *
 * # Mockup conflict resolution (P12.5 plan, user-approved)
 *
 * `mockups/CoreCalculator.html` reimagines the dialog as a
 * V-Matrix grid with level meters / import / share — none of
 * which exists in WPF or in any backend payload. Per the
 * P12.5 stance ("WPF 沒有的拒"), the mockup chrome is dropped
 * and the dialog renders the same three-pane WPF layout.
 *
 * # Caller wiring
 *
 * ```vue
 * <CoreCalculator v-model:visible="coreCalcOpen" />
 * ```
 *
 * Stateless from the parent's perspective — the dialog owns
 * its must-skills / cores / result entirely, so closing and
 * re-opening preserves the user's last session within the
 * dialog instance (mirrors WPF where the `Window` survives as
 * long as the user keeps it open).
 */

import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { ElButton, ElDialog, ElIcon, ElInput, ElMessage, ElOption, ElSelect } from 'element-plus'
import { CircleClose, Cpu } from '@element-plus/icons-vue'

import {
  type CoreItem,
  coreItemEquals,
  coreItemToString,
  findPerfectCores,
  formatPerfectCoreResult,
  mustCoreCount as computeMustCoreCount,
} from '../services/coreCalculator'

defineOptions({ name: 'CoreCalculatorDialog' })

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

/*
 * # State
 *
 * Each ref maps 1:1 to a WPF field on the `CoreCalculator`
 * partial class:
 *
 * - `mustSkills` ↔ `MustSkills` (ObservableCollection<string>)
 * - `useOtherSkillCount` ↔ `_useOtherSkillCount` (int)
 * - `coreItems` ↔ `CoreItems` (ObservableCollection<CoreItem>)
 * - `skillNameInput` ↔ `t_SkillName.Text`
 * - `selectedMustSkillIndex` ↔ `l_MustSkills.SelectedItem`
 *   (we store an index instead of the value because two
 *   identical-string entries can't exist — the AddMustSkill
 *   dedup ensures uniqueness — so index is just as
 *   stable and slightly cheaper to compare)
 * - `skill1` / `skill2` / `skill3` ↔ `c_Skill{1..3}.Text`
 * - `selectedCoreIndex` ↔ `l_Cores.SelectedItem`
 *   (index over reference because Vue refs would otherwise
 *   require deep-equality lookups; the helper handles
 *   structural equality where it matters)
 * - `resultText` ↔ `t_Result.Text`
 * - `calculating` ↔ `btn_Calculator.IsEnabled` (inverted)
 * - `hasInteracted` ↔ implicit WPF behaviour where
 *   `btn_Calculator.Content` only changes once the user
 *   touches MustSkills (initial label stays as `Caculator`
 *   from the XAML resource until the first add/delete fires
 *   the side effect)
 */
const mustSkills = ref<string[]>([])
const useOtherSkillCount = ref(0)
const coreItems = ref<CoreItem[]>([])

const skillNameInput = ref('')
const selectedMustSkillIndex = ref<number | null>(null)

const skill1 = ref('')
const skill2 = ref('')
const skill3 = ref('')
const selectedCoreIndex = ref<number | null>(null)

const resultText = ref('By:LinTx')
const calculating = ref(false)
const hasInteracted = ref(false)

/*
 * # Derived sources
 *
 * `mainSkillSource` = `[...mustSkills, Others1, Others2, …,
 *  Others<N+1>]` where `N` = `useOtherSkillCount`. Mirrors
 * the WPF property getter (L17-33) which rebuilt the
 * collection whenever its size drifted from
 * `MustSkills.Count + _useOtherSkillCount + 1`.
 *
 * `secondarySkillSource` = `[...mustSkills, Others]` — the
 * Secondary combos always share a single localized "Others"
 * wildcard slot (mirrors L36-47).
 *
 * Computeds re-derive on every change instead of trying to
 * match WPF's "rebuild only when the size mismatches" cache —
 * Vue's reactivity already memoises on dependency change, and
 * the full rebuild is O(N + M) for the array sizes a real
 * player ever has (typically ≤ 30 must-skills total).
 */
const othersLabel = computed((): string => t('Others'))

const mainSkillSource = computed((): string[] => [
  ...mustSkills.value,
  ...Array.from({ length: useOtherSkillCount.value + 1 }, (_, i) => `${othersLabel.value}${i + 1}`),
])

const secondarySkillSource = computed((): string[] => [...mustSkills.value, othersLabel.value])

const mustCoreCountValue = computed((): number => computeMustCoreCount(mustSkills.value.length))

/*
 * `Caculator` is the static XAML label; once the user adds
 * or removes a must-skill, WPF rewrites it to
 * `CaculatorCore({needed}, {totalSkills})`. The
 * `hasInteracted` flag captures that "first mutation"
 * boundary so the initial render matches WPF byte-for-byte.
 */
const calculatorLabel = computed((): string => {
  if (!hasInteracted.value) return t('Caculator')
  return t('CaculatorCore', [mustCoreCountValue.value, mustSkills.value.length])
})

/**
 * Reset combo selections after a successful Add Core. WPF
 * doesn't explicitly clear them (the `ComboBox` retains its
 * last value), but a literal port would leave the secondary
 * combos still showing the just-added core's secondaries —
 * which feels broken when the user adds 5+ cores in a row.
 * This is an SPA-side ergonomic improvement; functional
 * parity for the validation chain is preserved.
 */
function resetCoreInputs(): void {
  skill1.value = ''
  skill2.value = ''
  skill3.value = ''
}

function handleAddMustSkill(): void {
  const name = skillNameInput.value.trim()
  if (name === '') {
    ElMessage.error(t('SkillNameIsEmpty'))
    return
  }
  if (mustSkills.value.includes(name)) {
    ElMessage.error(t('SkillNameIsRepeat'))
    return
  }
  mustSkills.value = [...mustSkills.value, name]
  skillNameInput.value = ''
  hasInteracted.value = true
  resetCoreInputs()
}

function handleDeleteMustSkill(): void {
  const idx = selectedMustSkillIndex.value
  if (idx === null || idx < 0 || idx >= mustSkills.value.length) return

  const removed = mustSkills.value[idx]
  mustSkills.value = mustSkills.value.filter((_, i) => i !== idx)
  selectedMustSkillIndex.value = null
  hasInteracted.value = true

  /*
   * Clear any combo selection that referenced the deleted
   * skill so the dropdowns don't display a stale label.
   * WPF didn't bother — the ComboBox.Text would silently
   * mismatch its ItemsSource until the user re-clicked.
   */
  if (skill1.value === removed) skill1.value = ''
  if (skill2.value === removed) skill2.value = ''
  if (skill3.value === removed) skill3.value = ''
}

function handleAddCore(): void {
  const s1 = skill1.value
  const s2 = skill2.value
  const s3 = skill3.value
  if (s1 === '' || s2 === '' || s3 === '') {
    ElMessage.error(t('CoreSkillNameIsEmpty'))
    return
  }
  /*
   * WPF C# precedence: `s1==s2 || s1==s3 || ((s2==s3) && (s2!="Others"))`.
   * See module docblock for why the wildcard exception
   * exists. We also have to compare against the *current*
   * localized "Others" string (not the i18n key) because
   * the combos store the displayed label, just like WPF.
   */
  if (s1 === s2 || s1 === s3 || (s2 === s3 && s2 !== othersLabel.value)) {
    ElMessage.error(t('CoreSkillNameIsRepeat'))
    return
  }
  const item: CoreItem = { skill1: s1, skill2: s2, skill3: s3 }
  if (coreItems.value.some((existing) => coreItemEquals(existing, item))) {
    ElMessage.error(t('CoreIsRepeat'))
    return
  }
  coreItems.value = [...coreItems.value, item]

  /*
   * If the user just consumed the highest-available "OthersN"
   * slot in the Main combo, bump the counter so the next
   * core's Main combo offers `Others{N+1}`. Mirrors WPF
   * L105-112 verbatim.
   */
  const nextOthers = `${othersLabel.value}${useOtherSkillCount.value + 1}`
  if (item.skill1 === nextOthers) {
    useOtherSkillCount.value++
  }

  resetCoreInputs()
}

function handleDeleteCore(): void {
  const idx = selectedCoreIndex.value
  if (idx === null || idx < 0 || idx >= coreItems.value.length) return
  coreItems.value = coreItems.value.filter((_, i) => i !== idx)
  selectedCoreIndex.value = null
}

async function handleCalculate(): Promise<void> {
  /*
   * The pure helper is synchronous, but we still flip the
   * `calculating` flag (and yield to the event loop with
   * `await Promise.resolve()`) so the disabled state has a
   * chance to render before a heavy enumeration starts. WPF
   * relied on the UI thread blocking long enough for the
   * `IsEnabled = false` to paint; SPA needs the explicit
   * micro-task break for the same effect.
   */
  calculating.value = true
  await Promise.resolve()
  try {
    const groups = findPerfectCores(coreItems.value, mustSkills.value)
    resultText.value = formatPerfectCoreResult(groups, {
      mainLabel: t('Main'),
      notFound: t('NotFindPerfectCore'),
      formatGroup: (n) => t('CoreGroup', [n]),
    })
  } finally {
    calculating.value = false
  }
}

function handleClose(): void {
  emit('update:visible', false)
}

function handleVisibleChange(value: boolean): void {
  emit('update:visible', value)
}

function selectMustSkill(index: number): void {
  selectedMustSkillIndex.value = selectedMustSkillIndex.value === index ? null : index
}

function selectCore(index: number): void {
  selectedCoreIndex.value = selectedCoreIndex.value === index ? null : index
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
    class="core-calc-dialog"
    data-test="core-calculator-dialog"
    @update:model-value="handleVisibleChange"
  >
    <template #header>
      <div class="core-calc__header">
        <div class="core-calc__header-meta">
          <el-icon class="core-calc__header-icon" :size="20">
            <Cpu />
          </el-icon>
          <span class="core-calc__header-title" data-test="core-calculator-title">
            {{ t('PerfectCoreCaculator') }}
          </span>
        </div>
        <button
          type="button"
          class="core-calc__header-close"
          :title="t('Cancel')"
          data-test="core-calculator-close"
          @click="handleClose"
        >
          <el-icon><CircleClose /></el-icon>
        </button>
      </div>
    </template>

    <div class="core-calc__body">
      <!-- Left column: must-skills + cores -->
      <div class="core-calc__left">
        <section class="core-calc__panel" data-test="core-calculator-must-skills">
          <header class="core-calc__panel-header">{{ t('PerfectCoreNeedSkills') }}</header>
          <div class="core-calc__row">
            <el-input
              v-model="skillNameInput"
              size="small"
              clearable
              data-test="core-calculator-skill-name-input"
              @keyup.enter="handleAddMustSkill"
            />
            <el-button
              size="small"
              class="bf-btn-secondary"
              data-test="core-calculator-add-must-skill"
              @click="handleAddMustSkill"
            >
              {{ t('Add') }}
            </el-button>
            <el-button
              size="small"
              class="bf-btn-secondary"
              :disabled="selectedMustSkillIndex === null"
              data-test="core-calculator-delete-must-skill"
              @click="handleDeleteMustSkill"
            >
              {{ t('Delete') }}
            </el-button>
          </div>
          <ul
            class="core-calc__list bf-custom-scrollbar"
            data-test="core-calculator-must-skills-list"
          >
            <li
              v-for="(skill, idx) in mustSkills"
              :key="skill"
              class="core-calc__list-item"
              :class="{ 'core-calc__list-item--selected': selectedMustSkillIndex === idx }"
              :data-test="`core-calculator-must-skill-item-${idx}`"
              :aria-pressed="selectedMustSkillIndex === idx ? 'true' : 'false'"
              tabindex="0"
              role="button"
              @click="selectMustSkill(idx)"
              @keyup.enter="selectMustSkill(idx)"
              @keyup.space.prevent="selectMustSkill(idx)"
            >
              {{ skill }}
            </li>
          </ul>
        </section>

        <section class="core-calc__panel" data-test="core-calculator-cores">
          <header class="core-calc__panel-header">{{ t('PerfectCoreMyCores') }}</header>
          <div class="core-calc__field">
            <label class="core-calc__field-label">{{ t('Main') }}</label>
            <el-select
              v-model="skill1"
              size="small"
              :placeholder="t('Main')"
              data-test="core-calculator-skill1"
              class="core-calc__field-control"
            >
              <el-option v-for="opt in mainSkillSource" :key="opt" :label="opt" :value="opt" />
            </el-select>
          </div>
          <div class="core-calc__field">
            <label class="core-calc__field-label">{{ t('Secondary') }}</label>
            <el-select
              v-model="skill2"
              size="small"
              :placeholder="t('Secondary')"
              data-test="core-calculator-skill2"
              class="core-calc__field-control"
            >
              <el-option v-for="opt in secondarySkillSource" :key="opt" :label="opt" :value="opt" />
            </el-select>
          </div>
          <div class="core-calc__field">
            <label class="core-calc__field-label">{{ t('Secondary') }}</label>
            <el-select
              v-model="skill3"
              size="small"
              :placeholder="t('Secondary')"
              data-test="core-calculator-skill3"
              class="core-calc__field-control"
            >
              <el-option v-for="opt in secondarySkillSource" :key="opt" :label="opt" :value="opt" />
            </el-select>
          </div>
          <div class="core-calc__row core-calc__row--actions">
            <el-button
              size="small"
              class="bf-btn-secondary"
              data-test="core-calculator-add-core"
              @click="handleAddCore"
            >
              {{ t('Add') }}
            </el-button>
            <el-button
              size="small"
              class="bf-btn-secondary"
              :disabled="selectedCoreIndex === null"
              data-test="core-calculator-delete-core"
              @click="handleDeleteCore"
            >
              {{ t('Delete') }}
            </el-button>
          </div>
          <ul class="core-calc__list bf-custom-scrollbar" data-test="core-calculator-cores-list">
            <li
              v-for="(item, idx) in coreItems"
              :key="`${item.skill1}|${item.skill2}|${item.skill3}|${idx}`"
              class="core-calc__list-item"
              :class="{ 'core-calc__list-item--selected': selectedCoreIndex === idx }"
              :data-test="`core-calculator-core-item-${idx}`"
              :aria-pressed="selectedCoreIndex === idx ? 'true' : 'false'"
              tabindex="0"
              role="button"
              @click="selectCore(idx)"
              @keyup.enter="selectCore(idx)"
              @keyup.space.prevent="selectCore(idx)"
            >
              {{ coreItemToString(item, t('Main')) }}
            </li>
          </ul>
        </section>
      </div>

      <!-- Right column: calculator + result -->
      <div class="core-calc__right">
        <el-button
          type="primary"
          :loading="calculating"
          data-test="core-calculator-run"
          class="core-calc__run"
          @click="handleCalculate"
        >
          {{ calculatorLabel }}
        </el-button>
        <section class="core-calc__panel core-calc__panel--result">
          <header class="core-calc__panel-header">{{ t('PerfectCoreResult') }}</header>
          <el-input
            v-model="resultText"
            type="textarea"
            readonly
            resize="none"
            :rows="16"
            class="core-calc__result"
            data-test="core-calculator-result"
          />
        </section>
      </div>
    </div>
  </el-dialog>
</template>

<style scoped>
.core-calc__header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.75rem;
}

.core-calc__header-meta {
  display: inline-flex;
  align-items: center;
  gap: 0.625rem;
  min-width: 0;
}

.core-calc__header-icon {
  color: var(--bf-primary);
  flex-shrink: 0;
}

.core-calc__header-title {
  font-size: 0.9375rem;
  font-weight: 700;
  color: var(--bf-on-surface);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.core-calc__header-close {
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

.core-calc__header-close:hover {
  background: color-mix(in srgb, var(--bf-danger) 80%, transparent);
  color: var(--bf-on-danger);
}

.core-calc__body {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 0.75rem;
  padding: 0.25rem;
}

.core-calc__left,
.core-calc__right {
  display: flex;
  flex-direction: column;
  gap: 0.625rem;
  min-width: 0;
}

.core-calc__panel {
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
  padding: 0.625rem 0.75rem;
  background: color-mix(in srgb, var(--bf-surface) 60%, transparent);
  border: 1px solid var(--bf-outline-variant);
  border-radius: var(--bf-radius-input);
}

.core-calc__panel-header {
  font-size: 0.8125rem;
  font-weight: 600;
  color: var(--bf-on-surface-variant);
}

.core-calc__row {
  display: flex;
  gap: 0.375rem;
  align-items: center;
}

.core-calc__row--actions {
  justify-content: flex-end;
}

.core-calc__field {
  display: grid;
  grid-template-columns: 48px 1fr;
  align-items: center;
  gap: 0.5rem;
}

.core-calc__field-label {
  font-size: 0.8125rem;
  color: var(--bf-on-surface-variant);
}

.core-calc__field-control {
  width: 100%;
}

.core-calc__list {
  list-style: none;
  margin: 0;
  padding: 0.25rem;
  background: var(--bf-surface);
  border: 1px solid var(--bf-outline-variant);
  border-radius: var(--bf-radius-input);
  min-height: 96px;
  max-height: 160px;
  overflow-y: auto;
  font-size: 0.8125rem;
  color: var(--bf-on-surface);
}

.core-calc__list-item {
  padding: 0.25rem 0.5rem;
  border-radius: calc(var(--bf-radius-input) - 4px);
  cursor: pointer;
  transition: background var(--bf-motion-fast);
  outline: none;
}

.core-calc__list-item:hover,
.core-calc__list-item:focus-visible {
  background: color-mix(in srgb, var(--bf-primary-container) 25%, transparent);
}

.core-calc__list-item--selected {
  background: color-mix(in srgb, var(--bf-primary-container) 45%, transparent);
  font-weight: 600;
}

.core-calc__panel--result {
  flex: 1;
  min-height: 0;
}

.core-calc__run {
  align-self: stretch;
}

.core-calc__result :deep(.el-textarea__inner) {
  font-family: var(--bf-font-mono, ui-monospace, SFMono-Regular, monospace);
  font-size: 0.8125rem;
  line-height: 1.5;
  white-space: pre;
}
</style>
