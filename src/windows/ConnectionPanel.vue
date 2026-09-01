<script setup lang="ts">
/**
 * What the padlock opens: who answered for this page, and with what
 * certificate.
 *
 * Drawn inside the toolbar webview, which grows to make room — see
 * `CHROME_HEIGHTS` in `commands/web_browser.rs` for why a panel cannot
 * simply hang below the bar.
 */
import { useI18n } from 'vue-i18n'

import type { ConnectionInfo } from '../types/bindings'

const props = defineProps<{
  /** Null while the handshake is still in flight. */
  info: ConnectionInfo | null
}>()

defineEmits<{ close: [] }>()

const { t } = useI18n()

/** The backend sends RFC 2822; show it however this machine writes dates. */
function formatDate(value: string): string {
  const parsed = Date.parse(value)
  return Number.isNaN(parsed) ? value : new Date(parsed).toLocaleDateString()
}
</script>

<template>
  <div class="connection-panel">
    <div class="connection-panel__head">
      <!-- Spelled out rather than a ternary inside `t()`: the dead-key
           guard scans for literal keys, and a computed one reads to it as
           a key nobody uses. -->
      <span class="connection-panel__title">
        <template v-if="!props.info">{{ t('browserChrome.checking') }}</template>
        <template v-else-if="props.info.encrypted">{{ t('browserChrome.secure') }}</template>
        <template v-else>{{ t('browserChrome.notSecure') }}</template>
      </span>
      <button class="connection-panel__close" @click="$emit('close')">
        {{ t('browserChrome.closePanel') }}
      </button>
    </div>

    <dl v-if="props.info" class="connection-panel__rows">
      <dt>{{ t('browserChrome.host') }}</dt>
      <dd>{{ props.info.host }}:{{ props.info.port }}</dd>

      <template v-if="props.info.certificate">
        <dt>{{ t('browserChrome.issuedTo') }}</dt>
        <dd>{{ props.info.certificate.subject }}</dd>

        <dt>{{ t('browserChrome.issuedBy') }}</dt>
        <dd>{{ props.info.certificate.issuer }}</dd>

        <dt>{{ t('browserChrome.valid') }}</dt>
        <dd>
          {{ formatDate(props.info.certificate.validFrom) }} —
          {{ formatDate(props.info.certificate.validTo) }}
        </dd>

        <dt>{{ t('browserChrome.serial') }}</dt>
        <dd class="connection-panel__mono">{{ props.info.certificate.serial }}</dd>

        <dt>{{ t('browserChrome.fingerprint') }}</dt>
        <dd class="connection-panel__mono">{{ props.info.certificate.fingerprint }}</dd>
      </template>

      <template v-if="props.info.error">
        <dt>{{ t('browserChrome.checkFailed') }}</dt>
        <dd>{{ props.info.error }}</dd>
      </template>
    </dl>

    <p class="connection-panel__note">{{ t('browserChrome.note') }}</p>
  </div>
</template>

<style scoped>
.connection-panel {
  flex: 1 1 auto;
  min-height: 0;
  overflow-y: auto;
  padding: 12px 16px;
  border-bottom: 1px solid var(--el-border-color);
  background: var(--el-bg-color);
}

.connection-panel__head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 8px;
}

.connection-panel__title {
  font-size: 12px;
  font-weight: 600;
  color: var(--el-text-color-primary);
}

.connection-panel__close {
  padding: 2px 8px;
  border: none;
  border-radius: 4px;
  background: transparent;
  font-size: 11px;
  color: var(--el-text-color-secondary);
  cursor: pointer;
}

.connection-panel__close:hover {
  background: var(--el-fill-color-light);
  color: var(--el-text-color-primary);
}

.connection-panel__rows {
  display: grid;
  grid-template-columns: auto 1fr;
  gap: 4px 16px;
  margin: 0;
  font-size: 11px;
}

.connection-panel__rows dt {
  white-space: nowrap;
  color: var(--el-text-color-secondary);
}

.connection-panel__rows dd {
  margin: 0;
  overflow-wrap: anywhere;
  color: var(--el-text-color-primary);
}

.connection-panel__mono {
  font-family: ui-monospace, 'Cascadia Mono', Consolas, monospace;
  font-size: 10px;
  letter-spacing: -0.02em;
}

.connection-panel__note {
  margin: 12px 0 0;
  font-size: 10px;
  line-height: 1.6;
  color: var(--el-text-color-placeholder);
}
</style>
