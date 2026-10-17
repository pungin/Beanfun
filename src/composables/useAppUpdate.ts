import { computed, onScopeDispose, ref } from 'vue'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import { commands, type Channel } from '../types/bindings'
import { safeInvoke, surfaceCommandError } from '../services/invoke'

export interface AppUpdateProgress {
  phase: 'downloading' | 'verifying' | 'ready' | 'installing'
  downloaded: number
  total: number | null
}

type UpdatePhase = AppUpdateProgress['phase'] | 'idle' | 'preparing' | 'failed'

/** Prepare a verified update, then hand installation and restart to the backend. */
export function useAppUpdate() {
  const isUpdating = ref(false)
  const phase = ref<UpdatePhase>('idle')
  const downloaded = ref(0)
  const total = ref<number | null>(null)
  const percentage = computed(() =>
    total.value && total.value > 0
      ? Math.min(100, Math.max(0, Math.floor((downloaded.value / total.value) * 100)))
      : null,
  )
  let unlisten: UnlistenFn | undefined
  let disposed = false

  function stopListening() {
    unlisten?.()
    unlisten = undefined
  }

  onScopeDispose(() => {
    disposed = true
    stopListening()
  })

  async function downloadAndInstall(channel: Channel, tagName: string): Promise<void> {
    if (isUpdating.value || disposed) return
    isUpdating.value = true
    phase.value = 'preparing'
    downloaded.value = 0
    total.value = null
    let installationStarted = false
    let installing = false

    try {
      // Subscribe before the command so even a cached download reports its progress.
      const releaseListener = await listen<AppUpdateProgress>('app-update-progress', (event) => {
        if (disposed) return
        phase.value = event.payload.phase
        downloaded.value = event.payload.downloaded
        total.value = event.payload.total
      })
      if (disposed) {
        releaseListener()
        return
      }
      unlisten = releaseListener

      const prepared = await safeInvoke(commands.prepareUpdate(channel, tagName))
      if (disposed) return
      if (!prepared.ok) {
        surfaceCommandError(prepared.error)
        phase.value = 'failed'
        return
      }

      installing = true
      phase.value = 'installing'
      const installed = await safeInvoke(commands.installUpdate())
      if (disposed) return
      if (!installed.ok) {
        surfaceCommandError(installed.error)
        phase.value = 'failed'
        return
      }
      // Keep navigation locked until the backend exits the application.
      installationStarted = true
    } catch (error) {
      if (disposed) return
      console.warn('[AppUpdate] update command failed', error)
      surfaceCommandError({
        code: installing ? 'update.install_failed' : 'update.prepare_failed',
        message: installing
          ? 'The update could not be installed.'
          : 'The update could not be prepared.',
        details: null,
      })
      phase.value = 'failed'
    } finally {
      stopListening()
      if (!installationStarted) isUpdating.value = false
    }
  }

  return { isUpdating, phase, downloaded, total, percentage, downloadAndInstall }
}
