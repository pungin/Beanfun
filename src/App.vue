<script setup lang="ts">
/**
 * Root shell for the Beanfun frontend.
 *
 * # Boot sequence (runs once in `onMounted`)
 *
 * 1. `config.loadAll()` — pull every Config.xml entry into the
 *    in-memory cache. Subsequent `config.get(key)` calls return
 *    instantly without an IPC hop.
 * 2. `account.loadAccounts()` — decrypt `Users.dat` once into the
 *    account store. Mirrors WPF `MainWindow ctor` calling
 *    `accountManager.readRecord()` at startup so login-form
 *    prefill (P12.2 D2.5 / D2.7) and the ManageAccount page
 *    (P12.2 D9) read from a populated cache without a per-mount
 *    IPC hop. Soft-fails the same way `config.loadAll` does — a
 *    corrupt Users.dat or DPAPI failure must not soft-brick boot
 *    (the user can still pick "register a new account" or recover
 *    via Settings).
 * 3. `ui.applyAll()` — push the loaded config values out as DOM
 *    side effects: `setPrimaryColor` for the theme + the
 *    registered locale applier for vue-i18n. Either step
 *    soft-fails to defaults — a corrupt Config.xml entry must
 *    never soft-brick boot.
 *
 * All three calls run sequentially; `ui.applyAll()` reads from the
 * cache `config.loadAll()` populates so step 1 must finish before
 * step 3. Step 2 is independent of steps 1/3 (account state is
 * read by `IdPassForm` / `VerifyPage` mount-time prefill, not the
 * root shell), but lives in the boot sequence rather than a
 * lazy-on-mount hook so the very first navigation to `/login`
 * never paints a half-empty form.
 *
 * # Why `<el-config-provider>` at the root
 *
 * Element Plus components read their locale from the nearest
 * `<el-config-provider>` ancestor; placing it at the app root means
 * every `<el-*>` (across every page in P12) inherits the
 * user-selected language without per-page boilerplate. The locale
 * prop is bound to the UI store's reactive `language` getter, so
 * `setLanguage(...)` flips both the application's vue-i18n locale
 * and Element Plus's component-level translations in one shot.
 */

import { computed, onMounted } from 'vue'
import { ElConfigProvider, ElMessage } from 'element-plus'
import enLocale from 'element-plus/dist/locale/en.mjs'
import zhCnLocale from 'element-plus/dist/locale/zh-cn.mjs'
import zhTwLocale from 'element-plus/dist/locale/zh-tw.mjs'

import { useAccountStore } from './stores/account'
import { useConfigStore } from './stores/config'
import { useUiStore, type AppLocale } from './stores/ui'

const account = useAccountStore()
const config = useConfigStore()
const ui = useUiStore()

/**
 * Map our internal locale code to the matching Element Plus locale
 * pack. Centralized so adding a fourth locale (P12+) is a one-line
 * edit instead of a hunt-through-templates exercise.
 */
const ELP_LOCALE_MAP: Record<AppLocale, typeof zhTwLocale> = {
  'zh-TW': zhTwLocale,
  'zh-CN': zhCnLocale,
  'en-US': enLocale,
}

const elpLocale = computed(() => ELP_LOCALE_MAP[ui.language])

onMounted(async () => {
  try {
    await config.loadAll()
  } catch (err) {
    // The wrapCommand toast already fired; we just keep boot going so
    // the user can still see *something* (default theme + zh-TW) and
    // retry from Settings instead of staring at a blank window.
    console.error('[App.vue] config.loadAll failed; falling back to defaults', err)
    ElMessage.warning('Config.xml 載入失敗，使用預設設定。')
  }

  try {
    await account.loadAccounts()
  } catch (err) {
    /*
     * Same soft-fail pattern as `config.loadAll` — a corrupt
     * `Users.dat` or DPAPI failure must not block boot. The
     * `wrapCommand` toast already fired with the structured error
     * code; the account cache stays empty and downstream prefill
     * paths (IdPassForm / VerifyPage / ManageAccount) treat that
     * as "no saved credentials" rather than crashing.
     */
    console.error('[App.vue] account.loadAccounts failed; starting with empty cache', err)
  }

  ui.applyAll()
})
</script>

<template>
  <el-config-provider :locale="elpLocale">
    <RouterView />
  </el-config-provider>
</template>

<style>
:root {
  font-family:
    'Plus Jakarta Sans',
    'Inter',
    'Noto Sans TC',
    'PingFang TC',
    -apple-system,
    'Segoe UI',
    sans-serif;
  font-size: 14px;
  line-height: 1.5;
  color: #1f2329;
  background-color: #f5f6f8;

  font-synthesis: none;
  text-rendering: optimizeLegibility;
  -webkit-font-smoothing: antialiased;
  -moz-osx-font-smoothing: grayscale;
  -webkit-text-size-adjust: 100%;
}

html,
body,
#app {
  margin: 0;
  padding: 0;
  height: 100%;
  width: 100%;
}
</style>
