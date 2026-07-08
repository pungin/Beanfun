<script setup lang="ts">
/**
 * About page (P12.4 D7).
 *
 * # WPF parity
 *
 * Mirrors `Beanfun/Pages/About.xaml(.cs)` 1:1:
 *
 * | WPF section / control                       | SPA equivalent                                        |
 * |---------------------------------------------|-------------------------------------------------------|
 * | App icon (`Resources/icon.ico`)             | `<img src="/icon.png">` from public/                  |
 * | `t_AppName` `Label`                         | `<h1>{{ t('AppName') }}</h1>`                         |
 * | `t_Author` `Label` ("By Pungin")            | `<span>By Pungin and YCC3741</span>` (co-maintainer credit) |
 * | `t_Version` + `version` `TextBlock`         | `<p>{{ t('Version') }} {{ versionString }}</p>`       |
 * | `UpdateCheck_Click` `Hyperlink`             | `<a @click="handleCheckUpdate">{{ t('CheckUpdate') }}</a>` |
 * | `AboutText` formatted `TextBlock`           | `<RichText :value="t('AboutText')">` (WPF mini-markup parser) |
 * | `Contact` `Run`                             | `<p>{{ t('Contact') }}</p>`                           |
 * | `MailContact_Click` `Hyperlink`             | Two `<a @click>` links (Pungin + YCC3741), vertically stacked |
 * | `Github_Click` `Hyperlink` ("Github")       | `<a @click="handleGithub">Github</a>` → links to repo root |
 * | `Button_Click` Back button                  | `<el-button @click="handleBack">{{ t('Back') }}</el-button>` |
 *
 * # Update-check flow (D7 wiring)
 *
 * `UpdateCheck_Click` in WPF calls `App.MainWnd.CheckUpdates(true)`,
 * which spawns a background thread that fetches GitHub releases,
 * compares versions, and either shows a `NewVersionDetected` /
 * `NoUpdatesDetected` MessageBox. We mirror the same control flow
 * in the SPA via the existing `commands.checkUpdate` IPC (P10.3
 * already implements the GitHub fetch + version comparison server-
 * side, returning `Some(UpdateInfo)` for a newer release or `None`
 * otherwise — see `services/updater/checker.rs`):
 *
 * 1. Disable the link while a check is in flight.
 * 2. Fire `commands.checkUpdate(channel, null)` with the user's
 *    saved `updateChannel` preference.
 * 3. `Some(UpdateInfo)` → confirm dialog with body text + OK opens
 *    `download_url` via `commands.openUrl` (mirrors WPF
 *    `MessageBox.OK → Process.Start(downloadUrl)`).
 * 4. `None` → info toast (`NoUpdatesDetected`). WPF showed an OK
 *    MessageBox; an `ElMessage.info` is the equivalent low-noise
 *    affordance for the explicit-user-initiated "I checked, found
 *    nothing" case.
 *
 * # Why not embed the WPF mini-markup parser inline
 *
 * The `AboutText` resource uses the WPF custom `TextBlockHelper.
 * FormattedText` mini-markup with `<R Foreground="Red">` /
 * `<L/>` (line break) / `<B>` / `<R>` tags. The legacy parser
 * lives in `Beanfun/TextBlockHelper.cs` and is too WPF-specific
 * to port verbatim. We render the text as plain `<pre>` with the
 * tags stripped — colour emphasis is lost but the user-facing
 * meaning ("this is NOT the official client …") is preserved.
 * A future enhancement could route the mini-markup through a
 * Vue render function; for P12.4 the plain-text rendering
 * matches the WPF intent without introducing a parser dependency.
 */

import { computed, onMounted, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useRouter } from 'vue-router'
import { ElButton, ElIcon, ElMessage, ElMessageBox } from 'element-plus'
import { ArrowLeft, InfoFilled, Message } from '@element-plus/icons-vue'

import { useUiStore } from '../stores/ui'
import { commands } from '../types/bindings'
import { safeInvoke } from '../services/invoke'
import iconUrl from '../assets/icon.png'
import TitleBar from '../components/TitleBar.vue'

defineOptions({ name: 'AboutPage' })

const { t } = useI18n()
const router = useRouter()
const ui = useUiStore()

/* --------------- version --------------- */

/**
 * App version string. Hydrated on mount via `commands.version()`
 * which returns `{ app, tauri }` from compile-time env vars.
 * Empty string until hydrated so the template can render the
 * label without a flash of `undefined`.
 *
 * Mirrors WPF `version.Text = App.AssemblyVersion` (L17 in the
 * About code-behind), where `App.AssemblyVersion` is the C#
 * compile-time `AssemblyName.Version` string.
 */
const versionString = ref<string>('')

async function loadVersion(): Promise<void> {
  try {
    const info = await commands.version()
    versionString.value = info.app
  } catch (err) {
    /*
     * `commands.version` does not return a `Result<T, CommandError>`
     * (it never fails server-side — the env vars are baked at
     * compile time), so a thrown error here is an IPC bridge
     * failure (Tauri channel not yet mounted). Surface a structured
     * console warning rather than crashing the page; the user can
     * still read every other About-page affordance.
     */
    console.warn('[About] failed to read app version', err)
  }
}

/* --------------- update check --------------- */

const checkingUpdate = ref<boolean>(false)

/**
 * Fetch the latest release info from the configured channel and
 * present the user with the WPF-equivalent dialog.
 *
 * # Behaviour
 *
 * - `Some(UpdateInfo)` from backend → `ElMessageBox.confirm` with
 *   the localized `NewVersionDetected` body (interpolates new /
 *   current versions + release body). OK → open `download_url`
 *   in the system browser via `commands.openUrl`.
 * - `None` → `ElMessage.info` with `NoUpdatesDetected` (matches
 *   WPF `show=true` branch L185-191).
 * - Backend rejection (network failure, GitHub rate limit) →
 *   silent log + generic info toast. WPF's catch-all (L194-197)
 *   only `Debug.WriteLine`s the failure with no UI surface; we
 *   surface a low-key info toast so a user-initiated check at
 *   least gets a "we tried" acknowledgement instead of a silent
 *   no-op.
 */
async function handleCheckUpdate(): Promise<void> {
  if (checkingUpdate.value) return
  checkingUpdate.value = true
  try {
    const info = await commands.checkUpdate(ui.updateChannel, null)
    if (info === null) {
      ElMessage.info(t('NoUpdatesDetected'))
      return
    }

    /*
     * The locale string uses `\r\n` literal escapes inside the
     * JSON (WPF `Regex.Unescape` decoded these at runtime). Vue
     * i18n returns the raw escape, so we collapse `\r\n` /
     * `\n` to real newlines before piping into the dialog body.
     * `ElMessageBox.confirm` accepts a string argument and
     * pre-wraps `\n` to <br> automatically when `dangerouslyUseHTMLString`
     * is unset, so plain newlines is sufficient.
     */
    const rawBody = t('NewVersionDetected', [
      info.new_version_display,
      versionString.value,
      info.body,
    ])
    const body = rawBody.replace(/\\r\\n/g, '\n').replace(/\\n/g, '\n')

    try {
      await ElMessageBox.confirm(body, t('CheckUpdate'), {
        confirmButtonText: t('Yes'),
        cancelButtonText: t('No'),
        type: 'info',
      })
    } catch {
      return
    }
    await safeInvoke(commands.openUrl(info.download_url))
  } catch (err) {
    console.warn('[About] update check failed', err)
    ElMessage.info(t('NoUpdatesDetected'))
  } finally {
    checkingUpdate.value = false
  }
}

/* --------------- email / github / back --------------- */

/**
 * Open the maintainer's email client with a pre-filled subject /
 * body. Mirrors WPF `MailContact_Click` (L48-70):
 *
 * ```cs
 * string mailtoUrl = $"mailto:{to}?subject={subject}&body={body}";
 * Process.Start(new ProcessStartInfo { FileName = mailtoUrl, UseShellExecute = true });
 * ```
 *
 * - `subject` from `t('Feedback')` — already `URI`-encoded by the
 *   browser when fed to a `mailto:` URL (we still call
 *   `encodeURIComponent` to be defensive against future copy
 *   updates that contain `&` / `?` characters).
 * - `body` from `t('FeedbackText', [versionString.value])` — same
 *   defensive encoding.
 *
 * `commands.openUrl` is the SPA's `Process.Start(useShellExecute=true)`
 * equivalent; it routes `mailto:` URLs through the OS handler
 * (Windows `ShellExecuteW` on the backend).
 */
async function handleEmail(to: string): Promise<void> {
  const subject = encodeURIComponent(t('Feedback'))
  const bodyTemplate = t('FeedbackText', [versionString.value])
  const body = encodeURIComponent(bodyTemplate)
  const mailtoUrl = `mailto:${to}?subject=${subject}&body=${body}`
  await safeInvoke(commands.openUrl(mailtoUrl))
}

// 處理複製 QQ 號碼
async function handleCopyQQ(qqNumber: string): Promise<void> {
  try {
    await navigator.clipboard.writeText(qqNumber)

    // 如果你有設定多國語言，可以將 '已複製' 換成 t('Copied') 等等
    ElMessage({
      message: `QQ ${qqNumber} 已複製`,
      type: 'success',
      duration: 2000,
    })
  } catch (error) {
    console.error('Failed to copy text: ', error)
    ElMessage.error('複製失敗，請手動複製')
  }
}

/**
 * Open the maintainer's GitHub issue-template page in the user's
 * default browser. Mirrors WPF `Github_Click` (L72-82) — same
 * URL literal verbatim.
 */
async function handleGithub(): Promise<void> {
  await safeInvoke(commands.openUrl('https://github.com/pungin/Beanfun'))
}

/**
 * Back button — same `router.back()` strategy as Settings (D6).
 * See `pages/Settings.vue::handleBack` for the WPF parity
 * rationale.
 */
function handleBack(): void {
  if (window.history.length > 1) {
    router.back()
    return
  }
  void router.push('/login')
}

/* --------------- about-text rendering --------------- */

/**
 * The `AboutText` resource is wrapped in WPF's mini-markup tags
 * (`<R>` root, `<B>` bold, `<R Foreground="Red">` colored, `<L/>`
 * line break). The legacy parser is `Beanfun/TextBlockHelper.cs`;
 * porting it would add ~150 lines of XML / regex code for one
 * resource string.
 *
 * We strip the tags down to plain text + real line breaks here.
 * Color / weight emphasis is lost; the user-facing message ("this
 * is NOT the official client") still reads cleanly. Future work
 * could route the mini-markup through a Vue render function for
 * pixel-perfect parity — out of scope for P12.4.
 */
const aboutTextPlain = computed<string>(() => {
  const raw = t('AboutText')
  return raw
    .replace(/<L\s*\/?>/g, '\n')
    .replace(/<\/?[BR][^>]*>/g, '')
    .replace(/<\/?R[^>]*>/g, '')
    .trim()
})

/* --------------- mount --------------- */

onMounted(() => {
  void loadVersion()
})
</script>

<template>
  <main class="about bf-glass-window" data-window-root>
    <TitleBar />
    <div class="about__scroll">
      <div class="about__container" data-window-content>
        <!-- Header: app icon + name + author -->
        <header class="about__header bf-glass-panel">
          <img
            :src="iconUrl"
            alt="Beanfun"
            class="about__icon"
            width="56"
            height="56"
            data-test="about-icon"
          />
          <div class="about__header-text">
            <div class="about__title-row">
              <h1 class="about__title bf-text-gradient">{{ t('AppName') }}</h1>
              <span class="about__author">By Pungin, YCC3741 and lshw54</span>
            </div>
            <p class="about__version-row" data-test="about-version-row">
              <span>{{ t('Version') }}</span>
              <span class="about__version-value" data-test="about-version-value">
                {{ versionString }}
              </span>
              <a
                href="#"
                class="about__check-update"
                :class="{ 'about__check-update--disabled': checkingUpdate }"
                data-test="about-check-update"
                @click.prevent="handleCheckUpdate"
              >
                {{ t('CheckUpdate') }}
              </a>
            </p>
          </div>
        </header>

        <!-- Body: about text + contact + footer -->
        <section class="about__body bf-glass-panel">
          <p class="about__text" data-test="about-text">{{ aboutTextPlain }}</p>

          <div class="about__separator" />

          <div class="about__contact" data-test="about-contact">
            <span class="about__contact-label">{{ t('Contact') }}</span>
            <div class="about__contact-col">
              <a
                href="#"
                class="about__contact-link"
                data-test="about-email-pungin"
                @click.prevent="handleEmail('pungin@msn.com')"
              >
                <el-icon><Message /></el-icon>
                <span>Pungin</span>
              </a>
              <a
                href="#"
                class="about__contact-link"
                data-test="about-email-ycc3741"
                @click.prevent="handleEmail('mo0307b1006@gmail.com')"
              >
                <el-icon><Message /></el-icon>
                <span>YCC3741</span>
              </a>
              <div style="display: inline-flex; align-items: center; gap: 8px">
                <!-- 原本的 Email 連結 -->
                <a
                  href="#"
                  class="about__contact-link"
                  data-test="about-email-lshw54"
                  @click.prevent="handleEmail('lshw.5454@gmail.com')"
                >
                  <el-icon><Message /></el-icon>
                  <span>lshw54</span>
                </a>

                <!-- QQ 複製連結 -->
                <a
                  href="#"
                  class="about__contact-link"
                  style="font-size: 0.9em"
                  @click.prevent="handleCopyQQ('2157875454')"
                >
                  <span>QQ 2157875454</span>
                </a>
              </div>
              <a
                href="#"
                class="about__contact-link"
                data-test="about-github"
                @click.prevent="handleGithub"
              >
                <el-icon><InfoFilled /></el-icon>
                <span>Github</span>
              </a>
            </div>
          </div>

          <footer class="about__footer">
            <el-button
              class="bf-btn-secondary about__back-btn"
              data-test="about-back"
              @click="handleBack"
            >
              <el-icon><ArrowLeft /></el-icon>
              <span>{{ t('Back') }}</span>
            </el-button>
          </footer>
        </section>
      </div>
    </div>
  </main>
</template>

<style scoped>
.about {
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.about__scroll {
  flex: 1;
  overflow-y: auto;
  padding: 1rem 1.5rem;
}

.about__container {
  width: 100%;
  max-width: 560px;
  display: flex;
  flex-direction: column;
  gap: 1rem;
}

/* --------------- header --------------- */

.about__header {
  padding: 1rem 1.25rem;
  display: flex;
  align-items: center;
  gap: 0.875rem;
}

.about__icon {
  width: 56px;
  height: 56px;
  border-radius: var(--bf-radius-button);
  flex-shrink: 0;
  box-shadow: var(--bf-shadow-card);
}

.about__header-text {
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 0.25rem;
}

.about__title-row {
  display: flex;
  align-items: baseline;
  gap: 0.5rem;
  flex-wrap: wrap;
}

.about__title {
  margin: 0;
  font-size: 1.5rem;
  font-weight: 800;
  letter-spacing: -0.01em;
  line-height: 1.15;
}

.about__author {
  font-size: 0.8125rem;
  color: var(--bf-on-surface-variant);
}

.about__version-row {
  margin: 0;
  display: flex;
  align-items: center;
  gap: 0.5rem;
  font-size: 0.8125rem;
  color: var(--bf-on-surface-variant);
  flex-wrap: wrap;
}

.about__version-value {
  font-family: ui-monospace, SFMono-Regular, monospace;
  color: var(--bf-on-surface);
}

.about__check-update {
  color: var(--bf-primary);
  text-decoration: underline;
  cursor: pointer;
}

.about__check-update--disabled {
  pointer-events: none;
  opacity: 0.5;
}

/* --------------- body --------------- */

.about__body {
  padding: 1rem 1.25rem;
  display: flex;
  flex-direction: column;
  gap: 0.875rem;
}

.about__text {
  margin: 0;
  font-size: 0.875rem;
  color: var(--bf-on-surface);
  white-space: pre-wrap;
  line-height: 1.5;
}

.about__separator {
  height: 1px;
  background: color-mix(in srgb, var(--bf-outline-variant) 25%, transparent);
}

.about__contact {
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
}

.about__contact-label {
  font-size: 0.875rem;
  font-weight: 700;
  color: var(--bf-on-surface);
}

.about__contact-col {
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
}

.about__contact-link {
  display: inline-flex;
  align-items: center;
  gap: 0.375rem;
  color: var(--bf-primary);
  text-decoration: none;
  font-size: 0.875rem;
  cursor: pointer;
}

.about__contact-link:hover {
  text-decoration: underline;
}

.about__footer {
  display: flex;
  justify-content: flex-end;
  margin-top: 0.5rem;
}

.about__back-btn {
  display: inline-flex;
  align-items: center;
  gap: 0.375rem;
}
</style>
