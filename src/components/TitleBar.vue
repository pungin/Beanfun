<script setup lang="ts">
import { computed, type Component } from 'vue'
import { useRoute } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { ElIcon } from 'element-plus'
import {
  Close,
  Grid,
  InfoFilled,
  Key,
  Lock,
  Minus,
  Monitor,
  Promotion,
  Setting,
  SwitchButton,
  VideoPlay,
} from '@element-plus/icons-vue'
import { commands } from '../types/bindings'

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

const TITLE_ICON_MAP: Record<string, Component> = {
  coffee: Monitor,
  encrypted: Lock,
  info: InfoFilled,
  login: SwitchButton,
  manage_accounts: Key,
  public: Promotion,
  qr_code_2: Grid,
  settings: Setting,
  shield_lock: Lock,
  sports_esports: VideoPlay,
  verified_user: Lock,
}

const titleIconComponent = computed(() => {
  return TITLE_ICON_MAP[titleIcon.value] ?? Monitor
})

function handleDrag(e: MouseEvent): void {
  if (e.buttons === 1) {
    e.preventDefault()
    appWindow.startDragging()
  }
}

async function handleMinimize(): Promise<void> {
  // Delegate to backend so the `minimize_to_tray` config is honoured.
  // The legacy `appWindow.minimize()` direct call no longer works for
  // the post-PR-228 borderless+transparent window because Windows
  // stops emitting the `Resized(0, 0)` signal `tray::handle_minimize_to_tray`
  // listens for. The new command checks the config and either
  // hides+shows-tray or falls through to a plain minimize.
  const result = await commands.minimizeMainWindow()
  if (result.status === 'error') {
    console.error('[TitleBar] minimize_main_window failed:', result.error)
    appWindow.minimize()
  }
}

function handleClose(): void {
  appWindow.close()
}
</script>

<template>
  <div class="bf-titlebar" @mousedown="handleDrag">
    <div class="bf-titlebar__left">
      <el-icon class="bf-titlebar__icon" aria-hidden="true">
        <component :is="titleIconComponent" />
      </el-icon>
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
        <el-icon aria-hidden="true"><Minus /></el-icon>
      </button>
      <button
        type="button"
        class="bf-titlebar__btn bf-titlebar__btn--close"
        :title="t('titleBar.close')"
        @mousedown.stop
        @click="handleClose"
      >
        <el-icon aria-hidden="true"><Close /></el-icon>
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
  min-width: 0;
  pointer-events: none;
}
.bf-titlebar__icon {
  font-size: 20px;
  color: var(--bf-primary-container, #ff8201);
  flex: 0 0 20px;
}
.bf-titlebar__title {
  min-width: 0;
  font-size: 13px;
  font-weight: 600;
  color: var(--bf-on-surface, #221a11);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.bf-titlebar__right {
  display: flex;
  align-items: center;
  gap: 2px;
  flex: 0 0 auto;
}
.bf-titlebar__actions {
  display: flex;
  align-items: center;
  gap: 2px;
  margin-right: 4px;
  flex: 0 0 auto;
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
.bf-titlebar__btn .el-icon {
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
