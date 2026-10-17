<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import UpdateReleaseLink from './UpdateReleaseLink.vue'

defineProps<{ reason?: string | null; releaseTag?: string | null }>()
const { t } = useI18n()
</script>

<template>
  <div class="update-fallback">
    <p v-if="reason" data-test="update-failure-reason">{{ reason }}</p>
    <p>{{ t('appUpdate.manualDownloadHint') }}</p>
    <UpdateReleaseLink :release-tag="releaseTag" />
  </div>
</template>

<style scoped>
.update-fallback {
  display: flex;
  flex-direction: column;
  gap: 0.75rem;
  overflow-wrap: anywhere;
}

.update-fallback p {
  margin: 0;
}
</style>
