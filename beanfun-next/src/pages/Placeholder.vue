<script setup lang="ts">
/**
 * P11 infrastructure smoke page. Proves the whole frontend boot
 * pipeline is wired end-to-end by rendering:
 *
 * - A localized heading + subline (vue-i18n is live).
 * - The app + Tauri runtime versions via the `version` IPC command
 *   (tauri-specta bindings + invoke round-trip work).
 *
 * Will be replaced in P12 by the real login page; the file sticks
 * around only until `pages/LoginPage.vue` exists.
 */

import { computed, onMounted, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { commands, type VersionInfo } from '../types/bindings'

defineOptions({ name: 'PlaceholderPage' })

const { t } = useI18n()

const version = ref<VersionInfo | null>(null)
const versionError = ref<string | null>(null)

onMounted(async () => {
  try {
    version.value = await commands.version()
  } catch (err) {
    versionError.value = err instanceof Error ? err.message : String(err)
  }
})

const heading = computed(() => t('placeholder.heading'))
const subline = computed(() => t('placeholder.subline'))
</script>

<template>
  <div class="placeholder">
    <h1>{{ heading }}</h1>
    <p>{{ subline }}</p>
    <section v-if="version" class="placeholder__version">
      <strong>{{ t('placeholder.versionLabel') }}</strong>
      <code>{{ t('placeholder.appVersion') }}: {{ version.app }}</code>
      <code>{{ t('placeholder.tauriVersion') }}: {{ version.tauri }}</code>
    </section>
    <section v-else-if="versionError" class="placeholder__error">
      {{ t('placeholder.versionError', [versionError]) }}
    </section>
    <section v-else class="placeholder__loading">
      {{ t('placeholder.versionLoading') }}
    </section>
  </div>
</template>

<style scoped>
.placeholder {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 1rem;
  padding: 2rem;
  font-family:
    'Plus Jakarta Sans',
    'Inter',
    -apple-system,
    sans-serif;
  color: var(--el-color-primary, #333);
}

.placeholder h1 {
  font-size: 2rem;
  font-weight: 700;
  margin: 0;
}

.placeholder p {
  margin: 0;
  color: #555;
}

.placeholder__version,
.placeholder__error,
.placeholder__loading {
  display: flex;
  flex-direction: column;
  gap: 0.25rem;
  padding: 1rem 1.25rem;
  border: 1px solid var(--el-color-primary-light-7, #ddd);
  border-radius: 8px;
  background: rgba(255, 255, 255, 0.6);
  font-size: 0.875rem;
}

.placeholder__version code {
  font-family: 'Cascadia Code', 'Consolas', 'Menlo', monospace;
  color: #333;
}
</style>
