<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { ElMessage } from 'element-plus'
import { commands } from '../types/bindings'
import { safeInvoke } from '../services/invoke'

const props = defineProps<{ releaseTag?: string | null }>()
const { t } = useI18n()
const releaseUrl = computed(() => {
  const releases = 'https://github.com/pungin/Beanfun/releases'
  return props.releaseTag ? `${releases}/tag/${encodeURIComponent(props.releaseTag)}` : releases
})

async function openRelease(): Promise<void> {
  try {
    const result = await safeInvoke(commands.openUrl(releaseUrl.value))
    if (!result.ok) ElMessage.warning(t('appUpdate.releasePageFailed'))
  } catch (error) {
    console.warn('[AppUpdate] failed to open the release page', error)
    ElMessage.warning(t('appUpdate.releasePageFailed'))
  }
}
</script>

<template>
  <a
    :href="releaseUrl"
    class="update-release-link"
    target="_blank"
    rel="noopener noreferrer"
    @click.prevent="openRelease"
  >
    {{ t('appUpdate.releasePage') }}
  </a>
</template>

<style scoped>
.update-release-link {
  color: var(--el-color-primary);
  text-decoration: underline;
  overflow-wrap: anywhere;
}
</style>
