<script setup lang="ts">
import { computed } from 'vue'
import { useRoute } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { getCurrentWindow } from '@tauri-apps/api/window'

const route = useRoute()
const { t } = useI18n()
const appWindow = getCurrentWindow()

const titleText = computed(() => {
  const key = route.meta.titleKey as string | undefined
  return key ? t(key) : t('AppName')
})

const titleIcon = computed(() => {
  return (route.meta.titleIcon as string | undefined) ?? 'coffee'
})

function handleDrag(e: MouseEvent): void {
  if (e.buttons === 1) {
    e.preventDefault()
    appWindow.startDragging()
  }
}

function handleMinimize(): void {
  appWindow.minimize()
}

function handleClose(): void {
  appWindow.close()
}
</script>

<template>
  <div class="bf-titlebar" @mousedown="handleDrag">
    <div class="bf-titlebar__left">
      <span class="material-symbols-outlined bf-titlebar__icon">{{ titleIcon }}</span>
      <span class="bf-titlebar__title">{{ titleText }}</span>
    </div>
    <div class="bf-titlebar__right">
      <div v-if="$slots.default" class="bf-titlebar__actions" @mousedown.stop>
        <slot />
      </div>
      <button
        type="button"
        class="bf-titlebar__btn"
        :title="t('titleBar.minimize')"
        @mousedown.stop
        @click="handleMinimize"
      >
        <span class="material-symbols-outlined">minimize</span>
      </button>
      <button
        type="button"
        class="bf-titlebar__btn bf-titlebar__btn--close"
        :title="t('titleBar.close')"
        @mousedown.stop
        @click="handleClose"
      >
        <span class="material-symbols-outlined">close</span>
      </button>
    </div>
  </div>
</template>

<style scoped>
.bf-titlebar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  height: 40px;
  padding: 0 0.375rem 0 1rem;
  border-bottom: 1px solid rgba(255, 255, 255, 0.4);
  user-select: none;
  cursor: default;
  flex-shrink: 0;
}
.bf-titlebar__left {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  pointer-events: none;
}
.bf-titlebar__icon {
  font-size: 20px;
  color: var(--bf-primary-container, #ff8201);
}
.bf-titlebar__title {
  font-size: 13px;
  font-weight: 600;
  color: var(--bf-on-surface, #221a11);
}
.bf-titlebar__right {
  display: flex;
  align-items: center;
  gap: 2px;
}
.bf-titlebar__actions {
  display: flex;
  align-items: center;
  gap: 2px;
  margin-right: 4px;
}
.bf-titlebar__btn {
  appearance: none;
  background: transparent;
  border: none;
  width: 32px;
  height: 32px;
  display: grid;
  place-items: center;
  border-radius: 6px;
  cursor: pointer;
  color: var(--bf-on-surface, #221a11);
  transition: background 150ms ease;
  font: inherit;
  padding: 0;
}
.bf-titlebar__btn .material-symbols-outlined {
  font-size: 18px;
}
.bf-titlebar__btn:hover {
  background: rgba(0, 0, 0, 0.06);
}
.bf-titlebar__btn--close:hover {
  background: rgba(220, 38, 38, 0.8);
  color: #fff;
}
</style>
