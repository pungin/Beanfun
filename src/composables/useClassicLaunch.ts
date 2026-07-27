/**
 * MapleStory Classic (懷舊服) launch trigger + outcome feedback.
 *
 * Two pages start Classic — the login form (TW, where Classic is a
 * separate login) and the account list (HK, where the beanfun session
 * carries over) — and both need the same re-entry guard and the same
 * toasts driven by the backend's `classic-*` events. Keeping that in
 * one composable means the event names, the guard lifetime and the
 * "slow is not a failure" rule can't drift apart between them.
 *
 * The backend (`commands::classic`) emits:
 *
 * | Event                        | Meaning                                    | Guard |
 * |------------------------------|--------------------------------------------|-------|
 * | `classic-launched`           | NGM started — done                          | release |
 * | `classic-launch-failed`      | definitive failure (NGM missing / spawn)    | release |
 * | `classic-launch-slow`        | past the soft deadline, STILL watching      | keep    |
 * | `classic-needs-login`        | sign in inside the portal window            | keep    |
 */

import { onBeforeUnmount, ref, type Ref } from 'vue'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { ElMessage } from 'element-plus'
import { useI18n } from 'vue-i18n'

import { commands, type LoginRegion } from '../types/bindings'
import { safeInvoke } from '../services/invoke'
import {
  CLASSIC_FAILED_EVENT,
  CLASSIC_LAUNCHED_EVENT,
  CLASSIC_NEEDS_LOGIN_EVENT,
  CLASSIC_SLOW_EVENT,
} from '../constants/classic'

export interface ClassicLaunch {
  /** `true` while a launch is in flight (drives button disabled state). */
  launching: Ref<boolean>
  /** Start the Classic portal for `region`. */
  launch: (region: LoginRegion) => Promise<void>
}

export function useClassicLaunch(): ClassicLaunch {
  const { t } = useI18n()
  const launching = ref(false)
  const unlistenFns: UnlistenFn[] = []

  async function registerListeners(): Promise<void> {
    // `listen` needs the Tauri IPC bridge; in jsdom specs that don't stub
    // `@tauri-apps/api/event` it rejects, and the page must still mount.
    try {
      unlistenFns.push(
        await listen(CLASSIC_LAUNCHED_EVENT, () => {
          launching.value = false
          ElMessage.success(t('classic.launched'))
        }),
        await listen(CLASSIC_FAILED_EVENT, () => {
          launching.value = false
          ElMessage.warning(t('classic.launchFailed'))
        }),
        // Slow and needs-login are NOT terminal — the backend keeps
        // watching, so the guard stays armed and a later success lands.
        await listen(CLASSIC_SLOW_EVENT, () => {
          ElMessage.info(t('classic.launchSlow'))
        }),
        await listen(CLASSIC_NEEDS_LOGIN_EVENT, () => {
          ElMessage.info(t('classic.needsLogin'))
        }),
      )
    } catch (e) {
      console.warn('[classic] launch event listeners unavailable', e)
    }
  }
  void registerListeners()

  onBeforeUnmount(() => {
    for (const unlisten of unlistenFns) {
      try {
        unlisten()
      } catch (e) {
        console.error('[classic] unlisten threw', e)
      }
    }
    unlistenFns.length = 0
  })

  async function launch(region: LoginRegion): Promise<void> {
    if (launching.value) return
    launching.value = true
    ElMessage.info(t('classic.launching'))
    const result = await safeInvoke(commands.openClassicLogin(region))
    if (!result.ok) launching.value = false
  }

  return { launching, launch }
}
