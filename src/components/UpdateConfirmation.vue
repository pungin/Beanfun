<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import UpdateReleaseLink from './UpdateReleaseLink.vue'

defineProps<{
  currentVersion: string
  newVersion: string
  releaseTag: string
}>()

const { t } = useI18n()
</script>

<template>
  <div class="update-confirmation">
    <p>
      {{ t('appUpdate.confirmVersions', { current: currentVersion, version: newVersion }) }}
    </p>
    <p>{{ t('appUpdate.confirmBody') }}</p>
    <p>{{ t('appUpdate.cancelForManualDownload') }}</p>
    <UpdateReleaseLink :release-tag="releaseTag" />
  </div>
</template>

<style scoped>
.update-confirmation {
  display: flex;
  flex-direction: column;
  gap: 0.75rem;
  overflow-wrap: anywhere;
}
</style>
