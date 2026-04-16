import pluginVue from 'eslint-plugin-vue'
import { defineConfigWithVueTs, vueTsConfigs } from '@vue/eslint-config-typescript'
import skipFormatting from '@vue/eslint-config-prettier/skip-formatting'

export default defineConfigWithVueTs(
  {
    name: 'app/files-to-lint',
    files: ['**/*.{ts,mts,tsx,vue}'],
  },
  {
    name: 'app/files-to-ignore',
    ignores: [
      'dist/**',
      'src-tauri/target/**',
      'src-tauri/gen/**',
      'mockups/**',
      'node_modules/**',
      // Vue + Vite scaffold type shim uses `{}` / `any` by design.
      '**/vite-env.d.ts',
    ],
  },
  pluginVue.configs['flat/essential'],
  vueTsConfigs.recommended,
  skipFormatting,
)
