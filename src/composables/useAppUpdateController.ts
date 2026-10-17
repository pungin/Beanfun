import { computed, onScopeDispose, ref, watch, type InjectionKey, type Ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { ElMessage, ElMessageBox } from 'element-plus'
import { useUiStore } from '../stores/ui'
import { commands } from '../types/bindings'
import { useAppUpdate } from './useAppUpdate'

interface UpdateControllerOptions {
  promptBlocked?: Readonly<Ref<boolean>>
  showUpdatePage?: () => Promise<boolean>
}

export const APP_UPDATE_CONTROLLER: InjectionKey<ReturnType<typeof createAppUpdateController>> =
  Symbol('app-update-controller')

/** Share one check, confirmation and download across startup and manual actions. */
export function createAppUpdateController(options: UpdateControllerOptions = {}) {
  const { t } = useI18n()
  const ui = useUiStore()
  const update = useAppUpdate()
  const checkingUpdate = ref(false)
  const updateBusy = computed(() => checkingUpdate.value || update.isUpdating.value)
  let startupChecked = false
  let disposed = false
  let stopWaiting: (() => void) | undefined

  onScopeDispose(() => {
    disposed = true
    stopWaiting?.()
  })

  async function waitForPrompt(automatic: boolean, channel: string): Promise<void> {
    if (!options.promptBlocked?.value) return
    await new Promise<void>((resolve) => {
      const stop = watch(
        () => [options.promptBlocked?.value, ui.askUpdate, ui.updateChannel] as const,
        ([blocked, enabled, currentChannel]) => {
          if (!blocked || (automatic && (!enabled || currentChannel !== channel))) finish()
        },
      )
      function finish() {
        stop()
        stopWaiting = undefined
        resolve()
      }
      stopWaiting = finish
    })
  }

  async function checkForUpdate(automatic = false): Promise<void> {
    if (disposed || updateBusy.value || (automatic && startupChecked)) return
    startupChecked = true
    if (automatic && !ui.askUpdate) return

    checkingUpdate.value = true
    // Retain the exact channel the user checked and subsequently approved.
    const channel = ui.updateChannel
    const cancelled = () =>
      disposed || (automatic && (!ui.askUpdate || ui.updateChannel !== channel))
    try {
      const outcome = await commands.checkUpdate(channel, null)
      if (cancelled()) return
      if (outcome.status === 'error') {
        if (!automatic) ElMessage.warning(t('UpdateCheckFailed'))
        return
      }
      const info = outcome.data
      if (info === null) {
        if (!automatic) ElMessage.info(t('NoUpdatesDetected'))
        return
      }

      const version = await commands.version()
      if (cancelled()) return
      await waitForPrompt(automatic, channel)
      if (cancelled()) return
      try {
        await ElMessageBox.confirm(
          t('appUpdate.confirmBody', {
            version: info.new_version_display,
            current: version.app,
            notes: info.body,
          }),
          t('CheckUpdate'),
          {
            confirmButtonText: t('appUpdate.downloadAndRestart'),
            cancelButtonText: t('Cancel'),
            type: 'info',
          },
        )
      } catch {
        return
      }
      if (cancelled()) return
      if (options.showUpdatePage && !(await options.showUpdatePage())) return
      if (cancelled()) return
      await update.downloadAndInstall(channel, info.tag_name)
    } catch (error) {
      if (disposed) return
      console.warn('[AppUpdate] update check failed', error)
      if (!automatic) ElMessage.warning(t('UpdateCheckFailed'))
    } finally {
      checkingUpdate.value = false
    }
  }

  return { ...update, checkingUpdate, updateBusy, checkForUpdate }
}
