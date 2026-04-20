import { defineConfig } from 'vitest/config'
import vue from '@vitejs/plugin-vue'

export default defineConfig({
  plugins: [vue()],
  test: {
    environment: 'jsdom',
    globals: true,
    setupFiles: ['tests/setup.ts'],
    include: ['tests/unit/**/*.spec.ts'],
    coverage: {
      provider: 'v8',
      reporter: ['text', 'html', 'lcov'],
      exclude: ['tests/**', 'src-tauri/**', 'mockups/**', 'dist/**'],
    },
  },
})
