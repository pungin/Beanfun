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
 * # Update flow
 *
 * Check the selected Stable or Beta channel, request confirmation,
 * then download and verify the matching release through the backend.
 * Installation restarts the app; progress remains visible until exit.
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

import { computed, inject, onMounted, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { onBeforeRouteLeave, useRouter } from 'vue-router'
import { ElButton, ElIcon, ElMessage, ElProgress } from 'element-plus'
import { ArrowLeft, InfoFilled, Message } from '@element-plus/icons-vue'

import { commands } from '../types/bindings'
import { safeInvoke } from '../services/invoke'
import {
  APP_UPDATE_CONTROLLER,
  createAppUpdateController,
} from '../composables/useAppUpdateController'
import iconUrl from '../assets/icon.png'
import TitleBar from '../components/TitleBar.vue'
import UpdateFallback from '../components/UpdateFallback.vue'

defineOptions({ name: 'AboutPage' })

const { t } = useI18n()
const router = useRouter()

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

const { isUpdating, phase, percentage, updateBusy, manualDownload, checkForUpdate } =
  inject(APP_UPDATE_CONTROLLER, null) ?? createAppUpdateController()
const updateStatus = computed(() => {
  switch (phase.value) {
    case 'preparing':
      return t('appUpdate.preparing')
    case 'downloading':
      return t('appUpdate.downloading')
    case 'verifying':
      return t('appUpdate.verifying')
    case 'ready':
      return t('appUpdate.ready')
    case 'installing':
      return t('appUpdate.installing')
    case 'failed':
      return manualDownload.value?.reason ? t('appUpdate.failed') : ''
    default:
      return ''
  }
})

onBeforeRouteLeave(() => !isUpdating.value)

async function handleCheckUpdate(): Promise<void> {
  await checkForUpdate()
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

// Copy the QQ contact number.
async function handleCopyQQ(qqNumber: string): Promise<void> {
  try {
    await navigator.clipboard.writeText(qqNumber)
    ElMessage({
      message: t('about.qqCopied', { number: qqNumber }),
      type: 'success',
      duration: 2000,
    })
  } catch (error) {
    console.error('Failed to copy text: ', error)
    ElMessage.error(t('about.copyManually'))
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
  if (isUpdating.value) return
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
              <span class="about__author">{{ t('about.authors') }}</span>
            </div>
            <p class="about__version-row" data-test="about-version-row">
              <span>{{ t('Version') }}</span>
              <span class="about__version-value" data-test="about-version-value">
                {{ versionString }}
              </span>
              <a
                href="#"
                class="about__check-update"
                :class="{ 'about__check-update--disabled': updateBusy }"
                :aria-disabled="updateBusy"
                data-test="about-check-update"
                @click.prevent="handleCheckUpdate"
              >
                {{ t('CheckUpdate') }}
              </a>
            </p>
          </div>
        </header>

        <section
          v-if="updateStatus || manualDownload"
          class="about__update bf-glass-panel"
          aria-live="polite"
          data-test="about-update-status"
        >
          <p v-if="updateStatus">{{ updateStatus }}</p>
          <el-progress
            v-if="phase === 'downloading'"
            :percentage="percentage ?? 0"
            :indeterminate="percentage === null"
            :show-text="percentage !== null"
            data-test="about-update-progress"
          />
          <p v-if="isUpdating" class="about__update-hint">{{ t('appUpdate.keepOpen') }}</p>
          <UpdateFallback
            v-if="manualDownload && !isUpdating"
            :reason="manualDownload.reason"
            :release-tag="manualDownload.releaseTag"
            data-test="about-update-fallback"
          />
        </section>

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

                <!-- Copy the QQ contact number. -->
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
              :disabled="isUpdating"
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

.about__update {
  padding: 1rem 1.25rem;
  font-size: 0.875rem;
}

.about__update p {
  margin: 0 0 0.5rem;
}

.about__update-hint {
  color: var(--bf-on-surface-variant);
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
