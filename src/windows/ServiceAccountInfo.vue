<script setup lang="ts">
/**
 * Service-account info dialog (P12.2 D6).
 *
 * # WPF parity
 *
 * Mirrors `Beanfun/Windows/ServiceAccountInfo.xaml(.cs)` 1:1. The
 * WPF original is a top-level read-only `Window` opened by
 * `AccountList.xaml.cs::m_AccInfo_Click` (L212-219) via
 * `new ServiceAccountInfo(account).ShowDialog()` — pure display
 * over an already-loaded `ServiceAccount`, no IPC, no async, no
 * mutation. The SPA renders it as an in-page `<el-dialog>` (same
 * "modal vs new Window" rationale as D3 / D4).
 *
 * Field-by-field map (each row's WPF anchor is in the source file's
 * `XAML L<n>` / `.cs L<n>` references):
 *
 * | Field           | WPF source                                        | Render rule                                      |
 * | --------------- | ------------------------------------------------- | ------------------------------------------------ |
 * | Account         | `t_id.Text = account.sid` (L18)                   | always shown                                     |
 * | SerialNumber    | `t_sn.Text = account.ssn` (L16)                   | always shown                                     |
 * | Name            | `t_sname.Text = account.sname` (L17)              | always shown                                     |
 * | AuthType        | `t_sauthtype.Text = account.sauthtype` (L25-32)   | hidden when `sauthtype == null` (XAML `p_sauthtype.Visibility = Collapsed`) |
 * | Status          | `t_status.Content = isEnable ? Normal : Banned` (L19-24) | always shown; green/red text colour              |
 * | AccountEstablished + days big number + CreateDate | `screatetime` panel (L33-44 + XAML L69-86)        | hidden when `screatetime == null`                |
 * | LastLoginDate   | `t_slastusedtime` panel (L45-55 + XAML L87-95)    | hidden when `slastusedtime == null`              |
 *
 * # Mockup conflict resolution
 *
 * `mockups/ServiceAccountInfo.html` ships a 520px glass panel with
 * five **mockup-only** fields that have no backend representation
 * — every one is omitted in this port:
 *
 * - **Bound Devices / VIP / Recent Login IP** — neither
 *   `ServiceAccount` nor any other live store carries these fields;
 *   surfacing fake placeholder values would violate the D1 stub
 *   policy (no fake data the user can't act on).
 * - **Game label badge ("楓之谷 Online ・TW")** — the per-game
 *   metadata isn't on the account record (it's tracked at the
 *   session / launcher layer); the dialog is account-scoped, so
 *   adding session-scoped chrome would be a layering smell.
 * - **Service Code (`610074_T9`)** — same layering concern: this
 *   is per-`useAuthStore.session`, not per-account, and would
 *   imply (incorrectly) that the value is account-bound. The user
 *   pre-flight chose `omit` over the SPA-tighten alternative.
 *
 * The mockup also drops WPF's "X days since creation" big-number
 * affordance in favour of a plain CreateDate row. We **keep** the
 * WPF affordance — it's a distinctive UI feature on a small
 * dialog, and dropping it would be a visual regression for users
 * upgrading from WPF.
 *
 * Title text uses the WPF `ServiceAccountInfo` key ("帳號詳情" /
 * "Account Details") rather than the mockup's "角色資訊" — the
 * mockup phrasing leans on RPG-character vocabulary that drifts
 * from the WPF / business language.
 *
 * # Day-count math (`daysSinceCreation`)
 *
 * Mirrors WPF `getDays(string time)` (L63-69):
 *
 * ```csharp
 * DateTime start = Convert.ToDateTime(time);
 * DateTime end = Convert.ToDateTime(DateTime.Now);
 * TimeSpan sp = end.Subtract(start);
 * return Convert.ToString(sp.Days);
 * ```
 *
 * That's `Math.floor((Date.now() - new Date(screatetime)) / 86400000)`
 * in JS — both interpret the timestamp string in the local timezone
 * (WPF `Convert.ToDateTime`, JS `new Date(string)` for the
 * `"yyyy-MM-dd HH:mm:ss"` format Beanfun returns), so the day count
 * is consistent across both clients on the same machine.
 *
 * `Math.max(0, …)` is a defensive floor: if the backend ever
 * returns a future timestamp (clock skew, parser bug), we'd render
 * a negative day count which is a confusing user-visible artefact;
 * clamping to zero is a strictly better fallback than asserting on
 * the value here.
 *
 * # Why `account === null` is a no-op shell
 *
 * Mirrors the D4 `<ChangeServiceAccountDisplayName>` contract:
 * caller mounts the dialog always (so v-model bindings round-trip
 * cleanly) and clears `account` to `null` after close to avoid
 * leaking stale account references between sessions. The dialog
 * renders a no-content shell when `account === null` so the
 * v-model still emits cleanly without touching `.sid` etc.
 */

import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { ElButton, ElDialog, ElIcon } from 'element-plus'
import { CircleClose, InfoFilled } from '@element-plus/icons-vue'

import type { ServiceAccount } from '../types/bindings'

defineOptions({ name: 'ServiceAccountInfo' })

const props = defineProps<{
  /**
   * Two-way binding for whether the dialog is shown. Caller drives
   * open via `v-model:visible="..."`; the dialog drives close on
   * cancel / Esc / outside-click.
   */
  visible: boolean
  /**
   * The service account to inspect. `null` while the dialog is
   * closed (the caller clears it after the row context-menu
   * handler fires to avoid leaking stale account references
   * between sessions). The dialog renders a no-content shell when
   * `account === null` so the v-model binding round-trips cleanly.
   */
  account: ServiceAccount | null
}>()

const emit = defineEmits<{
  (event: 'update:visible', next: boolean): void
}>()

const { t } = useI18n()

const visible = computed({
  get: (): boolean => props.visible,
  set: (next: boolean): void => emit('update:visible', next),
})

/* --------------- field projections (read-only) --------------- */

/**
 * Days since `screatetime`. See the module docblock "Day-count
 * math" section for the WPF parity rationale.
 */
const daysSinceCreation = computed<number | null>(() => {
  const created = props.account?.screatetime
  if (!created) return null
  const ms = Date.now() - new Date(created).getTime()
  if (Number.isNaN(ms)) return null
  return Math.max(0, Math.floor(ms / 86400000))
})

/**
 * Mirrors WPF L19-24 (`account.isEnable ? Normal : Banned` +
 * Green / Red `SolidColorBrush`). Class wires through
 * `--bf-success` / `--bf-danger` design tokens so future colour-
 * scheme refactors propagate automatically.
 */
const statusText = computed<string>(() => (props.account?.is_enable ? t('Normal') : t('Banned')))
const statusColorClass = computed<string>(() =>
  props.account?.is_enable
    ? 'service-account-info__status--ok'
    : 'service-account-info__status--banned',
)

/* --------------- close --------------- */

function handleCancel(): void {
  visible.value = false
}
</script>

<template>
  <el-dialog
    v-model="visible"
    :show-close="false"
    :width="440"
    align-center
    append-to-body
    class="service-account-info-dialog"
    data-test="service-account-info-dialog"
  >
    <template #header>
      <div class="service-account-info__header">
        <div class="service-account-info__header-meta">
          <el-icon class="service-account-info__header-icon" :size="20">
            <InfoFilled />
          </el-icon>
          <span class="service-account-info__header-title">{{ t('ServiceAccountInfo') }}</span>
        </div>
        <button
          type="button"
          class="service-account-info__header-close"
          :title="t('Cancel')"
          data-test="service-account-info-close"
          @click="handleCancel"
        >
          <el-icon><CircleClose /></el-icon>
        </button>
      </div>
    </template>

    <!--
      `account === null` shell: render an empty body so v-model
      keeps round-tripping cleanly (caller pattern: clear account
      *after* dialog close → dialog briefly re-renders with a null
      account during the close animation; same shape as D4).
    -->
    <div v-if="account" class="service-account-info__body" data-test="service-account-info-body">
      <dl class="service-account-info__rows">
        <div class="service-account-info__row">
          <dt class="service-account-info__label">{{ t('Account') }}</dt>
          <dd class="service-account-info__value" data-test="service-account-info-sid">
            {{ account.sid }}
          </dd>
        </div>
        <div class="service-account-info__row">
          <dt class="service-account-info__label">{{ t('SerialNumber') }}</dt>
          <dd class="service-account-info__value" data-test="service-account-info-ssn">
            {{ account.ssn }}
          </dd>
        </div>
        <div class="service-account-info__row">
          <dt class="service-account-info__label">{{ t('Name') }}</dt>
          <dd class="service-account-info__value" data-test="service-account-info-sname">
            {{ account.sname }}
          </dd>
        </div>
        <div
          v-if="account.sauthtype != null"
          class="service-account-info__row"
          data-test="service-account-info-authtype-row"
        >
          <dt class="service-account-info__label">{{ t('AuthType') }}</dt>
          <dd class="service-account-info__value" data-test="service-account-info-authtype">
            {{ account.sauthtype }}
          </dd>
        </div>
        <div class="service-account-info__row">
          <dt class="service-account-info__label">{{ t('Status') }}</dt>
          <dd
            class="service-account-info__value"
            :class="statusColorClass"
            data-test="service-account-info-status"
          >
            {{ statusText }}
          </dd>
        </div>
      </dl>

      <!--
        Account-established panel — only rendered when the backend
        scrape provided a `screatetime`. Big day count is the WPF
        signature affordance (XAML L69-86 `t_screatedays` Foreground=Blue
        FontSize=30).
      -->
      <div
        v-if="account.screatetime != null"
        class="service-account-info__created"
        data-test="service-account-info-created"
      >
        <p class="service-account-info__created-label">
          {{ t('AccountEstablished') }}
        </p>
        <p class="service-account-info__created-days" data-test="service-account-info-created-days">
          {{ daysSinceCreation }}
        </p>
        <p class="service-account-info__created-unit">{{ t('Days') }}</p>
        <p
          class="service-account-info__created-since"
          data-test="service-account-info-created-since"
        >
          {{ t('CreateDate', [account.screatetime]) }}
        </p>
      </div>

      <p
        v-if="account.slastusedtime != null"
        class="service-account-info__last-login"
        data-test="service-account-info-last-login"
      >
        {{ t('LastLoginDate', [account.slastusedtime]) }}
      </p>
    </div>

    <template #footer>
      <div class="service-account-info__footer">
        <el-button
          class="bf-btn-secondary service-account-info__btn-secondary"
          data-test="service-account-info-cancel"
          @click="handleCancel"
        >
          {{ t('Cancel') }}
        </el-button>
      </div>
    </template>
  </el-dialog>
</template>

<style scoped>
.service-account-info__header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.5rem;
}

.service-account-info__header-meta {
  display: inline-flex;
  align-items: center;
  gap: 0.5rem;
  min-width: 0;
}

.service-account-info__header-icon {
  color: var(--bf-primary);
  flex-shrink: 0;
}

.service-account-info__header-title {
  font-size: 0.9375rem;
  font-weight: 700;
  color: var(--bf-on-surface);
}

.service-account-info__header-close {
  appearance: none;
  border: 0;
  background: transparent;
  width: 28px;
  height: 28px;
  display: grid;
  place-items: center;
  border-radius: var(--bf-radius-input);
  color: var(--bf-on-surface-variant);
  cursor: pointer;
  transition:
    background var(--bf-motion-fast),
    color var(--bf-motion-fast);
}

.service-account-info__header-close:hover {
  background: color-mix(in srgb, var(--bf-danger) 80%, transparent);
  color: var(--bf-on-danger);
}

.service-account-info__body {
  display: flex;
  flex-direction: column;
  gap: 1.25rem;
}

.service-account-info__rows {
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
  margin: 0;
}

.service-account-info__row {
  display: grid;
  grid-template-columns: 6rem 1fr;
  align-items: baseline;
  gap: 0.75rem;
  padding-block: 0.375rem;
  border-bottom: 1px dashed color-mix(in srgb, var(--bf-on-surface) 12%, transparent);
}

.service-account-info__row:last-child {
  border-bottom: none;
}

.service-account-info__label {
  font-size: 0.75rem;
  font-weight: 600;
  color: var(--bf-on-surface-variant);
  margin: 0;
}

.service-account-info__value {
  font-size: 0.875rem;
  font-weight: 700;
  color: var(--bf-on-surface);
  margin: 0;
  word-break: break-all;
  /*
   * Issue #234: use-case is copy-paste of the account / serial-number
   * / name / authType strings the dialog surfaces. App.vue disables
   * text selection globally (-webkit-user-select: none on body) to
   * feel native; this dialog is the one place an explicit override
   * matters because every row is a read-only value the user frequently
   * copies into game support tickets.
   */
  -webkit-user-select: text;
  user-select: text;
  cursor: text;
}

.service-account-info__status--ok {
  color: var(--bf-success);
}

.service-account-info__status--banned {
  color: var(--bf-danger);
}

.service-account-info__created {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 0.125rem;
  padding: 0.75rem 0.5rem;
  border-radius: var(--bf-radius-card);
  background: color-mix(in srgb, var(--bf-primary-container) 10%, transparent);
}

.service-account-info__created-label {
  font-size: 0.6875rem;
  color: var(--bf-on-surface-variant);
  margin: 0;
}

.service-account-info__created-days {
  font-size: 1.875rem;
  font-weight: 800;
  color: var(--bf-primary);
  margin: 0;
  line-height: 1.1;
  font-family: 'JetBrains Mono', ui-monospace, SFMono-Regular, Menlo, monospace;
}

.service-account-info__created-unit {
  font-size: 0.6875rem;
  color: var(--bf-on-surface-variant);
  margin: 0;
}

.service-account-info__created-since {
  font-size: 0.6875rem;
  color: var(--bf-danger);
  margin: 0.125rem 0 0;
}

.service-account-info__last-login {
  text-align: center;
  font-size: 0.75rem;
  color: var(--bf-danger);
  margin: 0;
}

.service-account-info__footer {
  display: flex;
  justify-content: flex-end;
}

.service-account-info__btn-secondary {
  min-width: 88px;
}
</style>
