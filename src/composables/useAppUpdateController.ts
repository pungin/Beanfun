import { computed, h, onScopeDispose, ref, watch, type InjectionKey, type Ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { ElMessage, ElMessageBox } from 'element-plus'
import { useUiStore } from '../stores/ui'
import { commands } from '../types/bindings'
import { translateCommandError } from '../services/invoke'
import { useAppUpdate } from './useAppUpdate'
import UpdateConfirmation from '../components/UpdateConfirmation.vue'
import UpdateFallback from '../components/UpdateFallback.vue'
import '../styles/update-message-box.css'

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
  const manualDownload = ref<{ releaseTag: string | null; reason: string | null } | null>(null)
  let startupChecked = false
  let disposed = false
  let stopWaiting: (() => void) | undefined

  onScopeDispose(() => {
    disposed = true
    stopWaiting?.()
  })

  async function showFailure(
    reason: string,
    releaseTag: string | null,
    title = t('appUpdate.failedTitle'),
  ): Promise<void> {
    manualDownload.value = { releaseTag, reason }
    try {
      await ElMessageBox.alert(h(UpdateFallback, { reason, releaseTag }), title, {
        confirmButtonText: t('Close'),
        type: 'error',
        customClass: 'app-update-confirm',
      })
    } catch {
      // Dismissing the message leaves the manual download link on the About page.
    }
  }

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
    if (!automatic) manualDownload.value = null
    let selectedReleaseTag: string | null = null
    // Retain the exact channel the user checked and subsequently approved.
    const channel = ui.updateChannel
    const cancelled = () =>
      disposed || (automatic && (!ui.askUpdate || ui.updateChannel !== channel))
    try {
      const outcome = await commands.checkUpdate(channel, null)
      if (cancelled()) return
      if (outcome.status === 'error') {
        if (!automatic) {
          await showFailure(translateCommandError(outcome.error), null, t('UpdateCheckFailed'))
        }
        return
      }
      const info = outcome.data
      if (info === null) {
        if (!automatic) ElMessage.info(t('NoUpdatesDetected'))
        return
      }

      selectedReleaseTag = info.tag_name

      const version = await commands.version()
      if (cancelled()) return
      await waitForPrompt(automatic, channel)
      if (cancelled()) return
      manualDownload.value = { releaseTag: info.tag_name, reason: null }
      try {
        await ElMessageBox.confirm(
          h(UpdateConfirmation, {
            currentVersion: version.app,
            newVersion: info.new_version_display,
            releaseTag: info.tag_name,
          }),
          t('CheckUpdate'),
          {
            confirmButtonText: t('appUpdate.downloadAndRestart'),
            cancelButtonText: t('Cancel'),
            type: 'info',
            customClass: 'app-update-confirm',
          },
        )
      } catch {
        return
      }
      if (cancelled()) return
      if (options.showUpdatePage && !(await options.showUpdatePage())) return
      if (cancelled()) return
      const failure = await update.downloadAndInstall(channel, info.tag_name)
      if (!disposed && failure) {
        await showFailure(translateCommandError(failure), info.tag_name)
      }
    } catch (error) {
      if (disposed) return
      console.warn('[AppUpdate] update check failed', error)
      if (!automatic)
        await showFailure(t('UpdateCheckFailed'), selectedReleaseTag, t('UpdateCheckFailed'))
    } finally {
      checkingUpdate.value = false
    }
  }

  return { ...update, checkingUpdate, updateBusy, manualDownload, checkForUpdate }
}
