<script setup lang="ts">
/**
 * MapleStory Classic (懷舊服) game-account picker.
 *
 * The TW GamaPass flow ends with a `SelectGameAccount` step. When the
 * portal offers more than one account the backend does **not** make the
 * user hunt for the radio inside the web page — it posts the list over
 * `classic-account-choice` and this dialog asks natively, then answers
 * with `classic_select_account`, which selects the account and submits
 * the step inside the portal window.
 *
 * A single account never reaches here: the portal script selects and
 * submits it directly.
 *
 * Mounted once at the app root ({@link App.vue}) so it works no matter
 * which page started the launch (login form for TW, account list for
 * HK).
 */

import { onBeforeUnmount, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { ElButton, ElDialog, ElRadio, ElRadioGroup } from 'element-plus'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'

import { commands, type ClassicAccount } from '../types/bindings'
import { safeInvoke } from '../services/invoke'
import { CLASSIC_ACCOUNT_CHOICE_EVENT } from '../constants/classic'

defineOptions({ name: 'ClassicAccountPicker' })

const { t } = useI18n()

const visible = ref(false)
const accounts = ref<ClassicAccount[]>([])
const selected = ref('')
const submitting = ref(false)

const unlistenFns: UnlistenFn[] = []

async function registerListener(): Promise<void> {
  // `listen` needs the Tauri IPC bridge; in jsdom specs that don't stub
  // `@tauri-apps/api/event` it rejects, and the app must still mount.
  try {
    unlistenFns.push(
      await listen<{ accounts: ClassicAccount[] }>(CLASSIC_ACCOUNT_CHOICE_EVENT, (event) => {
        accounts.value = event.payload.accounts ?? []
        selected.value = accounts.value[0]?.value ?? ''
        visible.value = accounts.value.length > 0
      }),
    )
  } catch (e) {
    console.warn('[classic-picker] account-choice listener unavailable', e)
  }
}
void registerListener()

onBeforeUnmount(() => {
  for (const unlisten of unlistenFns) {
    try {
      unlisten()
    } catch (e) {
      console.error('[classic-picker] unlisten threw', e)
    }
  }
  unlistenFns.length = 0
})

async function confirm(): Promise<void> {
  if (!selected.value || submitting.value) return
  submitting.value = true
  const result = await safeInvoke(commands.classicSelectAccount(selected.value))
  submitting.value = false
  // Keep the dialog open on failure so the user can retry (the portal
  // window may have been closed, which the error names).
  if (result.ok) visible.value = false
}
</script>

<template>
  <el-dialog
    v-model="visible"
    :title="t('classic.accountPickerTitle')"
    width="min(420px, 90vw)"
    align-center
    data-test="classic-account-picker"
  >
    <p class="classic-picker__hint">{{ t('classic.accountPickerHint') }}</p>
    <el-radio-group v-model="selected" class="classic-picker__list">
      <el-radio
        v-for="account in accounts"
        :key="account.value"
        :value="account.value"
        class="classic-picker__item"
        :data-test="`classic-account-${account.value}`"
      >
        <span class="classic-picker__name">{{ account.name || account.value }}</span>
      </el-radio>
    </el-radio-group>
    <template #footer>
      <el-button
        type="primary"
        :loading="submitting"
        :disabled="selected === ''"
        data-test="classic-account-confirm"
        @click="confirm"
      >
        {{ t('classic.accountPickerConfirm') }}
      </el-button>
    </template>
  </el-dialog>
</template>

<style scoped>
.classic-picker__hint {
  margin: 0 0 0.75rem;
  font-size: 0.82rem;
  line-height: 1.6;
  color: var(--bf-on-surface-variant, #54443a);
}

.classic-picker__list {
  display: flex;
  flex-direction: column;
  align-items: stretch;
  gap: 0.25rem;
  width: 100%;
}

.classic-picker__item {
  height: auto;
  padding: 0.5rem 0.25rem;
  margin-right: 0;
}

.classic-picker__name {
  font-size: 0.9rem;
  font-weight: 600;
}
</style>
