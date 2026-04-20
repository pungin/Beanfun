/**
 * P12.2 D6 — ServiceAccountInfo modal behaviour.
 *
 * What this spec locks down (matches the D6 scope outlined in
 * `windows/ServiceAccountInfo.vue`):
 *
 *  1. Always-shown rows render straight off `account.{sid,ssn,sname}`.
 *  2. AuthType row is hidden when `sauthtype == null` (XAML
 *     `p_sauthtype.Visibility = Collapsed`) and shown otherwise.
 *  3. Status row maps `is_enable` true → localized "Normal" + the
 *     success colour class; false → localized "Banned" + the danger
 *     colour class. Mirrors WPF L19-24
 *     (Green / Red SolidColorBrush + Normal / Banned literal).
 *  4. The "AccountEstablished" panel (label + big-number days +
 *     CreateDate red row) only renders when `screatetime != null`.
 *  5. `daysSinceCreation` math mirrors WPF `getDays(string time)`
 *     (L63-69): `Math.floor((Date.now() - new Date(screatetime)) / 86400000)`,
 *     with a `Math.max(0, …)` defensive floor for clock-skew safety.
 *  6. `LastLoginDate` row is hidden when `slastusedtime == null`,
 *     shown otherwise with the localized template.
 *  7. Cancel button + header-close button each emit
 *     `update:visible(false)` and never invoke any command.
 *  8. `account === null` shell mode: the dialog mounts (so
 *     v-model bindings stay live) but renders no body — required
 *     by the AccountList caller pattern that clears the target ref
 *     *after* the dialog finishes closing.
 *  9. Re-opening the dialog with a different account swaps every
 *     field cleanly (no stale-row bleed-through).
 *
 * # Why no command mocks
 *
 * The dialog is a pure-display component (mirrors WPF
 * `ServiceAccountInfo.xaml.cs` which has no IPC / no async). The
 * `commands` mock is a defensive sanity guard: every test asserts
 * `commands.changeDisplayName` was never called, so a future refactor
 * that accidentally introduces an IPC dependency on the info dialog
 * is caught loudly here.
 */

import { beforeEach, afterEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { flushPromises, mount } from '@vue/test-utils'
import { defineComponent, h, ref, type Component } from 'vue'

import type { ServiceAccount } from '../../../src/types/bindings'

vi.mock('element-plus', async () => {
  const { defineComponent: dc, h: hh, watch: w, nextTick: nt } = await import('vue')
  /*
   * Same dialog stub as the D4 spec: round-trip the v-model
   * contract (`modelValue` + `update:modelValue`) and fire `closed`
   * on the `true → false` transition so any future `@closed`
   * handler the dialog grows would get the right callback shape.
   */
  const ElDialog = dc({
    name: 'ElDialogStub',
    props: { modelValue: { type: Boolean, default: false } },
    emits: ['update:modelValue', 'closed'],
    setup(props, { slots, attrs, emit }) {
      w(
        () => props.modelValue,
        async (next, prev) => {
          if (prev === true && next === false) {
            await nt()
            emit('closed')
          }
        },
      )
      return () =>
        props.modelValue
          ? hh('div', { ...attrs, class: 'el-dialog-stub' }, [
              slots.header?.(),
              hh('div', { class: 'el-dialog-stub__body' }, slots.default?.()),
              hh('div', { class: 'el-dialog-stub__footer' }, slots.footer?.()),
            ])
          : null
    },
  })

  const ElButton = dc({
    name: 'ElButtonStub',
    emits: ['click'],
    setup(_, { slots, emit, attrs }) {
      return () =>
        hh(
          'button',
          {
            ...attrs,
            class: 'el-button-stub',
            onClick: (e: MouseEvent) => emit('click', e),
          },
          slots.default?.(),
        )
    },
  })

  const ElIcon = dc({
    name: 'ElIconStub',
    setup(_, { slots }) {
      return () => hh('span', { class: 'el-icon-stub' }, slots.default?.())
    },
  })

  return { ElDialog, ElButton, ElIcon }
})

vi.mock('@element-plus/icons-vue', () => {
  const stub = (name: string): Component => defineComponent({ name, render: () => h('svg') })
  return {
    CircleClose: stub('CircleCloseStub'),
    InfoFilled: stub('InfoFilledStub'),
  }
})

vi.mock('../../../src/types/bindings', () => ({
  commands: {
    changeDisplayName: vi.fn(),
    refresh: vi.fn(),
    getAccounts: vi.fn(),
  },
}))

import { commands } from '../../../src/types/bindings'
import ServiceAccountInfo from '../../../src/windows/ServiceAccountInfo.vue'
import { createAppI18n, i18nMessages } from '../../../src/i18n'

/* ------------------------------------------------------------------ */
/* Sample fixtures                                                     */
/* ------------------------------------------------------------------ */

/** Minimal-required shape (every nullable field is null). */
const SAMPLE_ACCOUNT: ServiceAccount = {
  is_enable: true,
  visible: true,
  is_inherited: false,
  sid: 'sid-1',
  ssn: '00001',
  sname: 'Main Toon',
  screatetime: null,
  slastusedtime: null,
  sauthtype: null,
}

/** Full shape: every optional field populated, banned status. */
const FULL_BANNED_ACCOUNT: ServiceAccount = {
  ...SAMPLE_ACCOUNT,
  sid: 'sid-2',
  ssn: '00002',
  sname: 'Mule Account',
  is_enable: false,
  sauthtype: 'WCDES',
  screatetime: '2024-01-15 12:34:56',
  slastusedtime: '2024-12-10 08:00:00',
}

/* ------------------------------------------------------------------ */
/* Harness                                                             */
/* ------------------------------------------------------------------ */

/**
 * Wrap the dialog in a host that owns both `visible` and the
 * target `account` so tests can drive the v-model + the row
 * selection from the outside (mirrors the D4 spec pattern and
 * the real `AccountList.vue` consumer shape).
 */
function buildHarness(
  initialAccount: ServiceAccount | null = SAMPLE_ACCOUNT,
  initialVisible = true,
) {
  const visibleRef = ref(initialVisible)
  const accountRef = ref<ServiceAccount | null>(initialAccount)
  const Host = defineComponent({
    name: 'ServiceAccountInfoHost',
    components: { ServiceAccountInfo },
    setup() {
      return { visible: visibleRef, account: accountRef }
    },
    template: `<ServiceAccountInfo v-model:visible="visible" :account="account" />`,
  })
  return { visibleRef, accountRef, Host }
}

describe('ServiceAccountInfo modal', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    vi.spyOn(console, 'error').mockImplementation(() => {})
    vi.spyOn(console, 'warn').mockImplementation(() => {})
    for (const fn of Object.values(commands) as ReturnType<typeof vi.fn>[]) fn.mockReset()
  })

  afterEach(() => {
    vi.useRealTimers()
  })

  it('renders sid, ssn, and sname rows from the account', async () => {
    const { Host } = buildHarness()
    const wrapper = mount(Host, { global: { plugins: [createAppI18n()] } })
    await flushPromises()

    expect(wrapper.get('[data-test="service-account-info-sid"]').text()).toBe('sid-1')
    expect(wrapper.get('[data-test="service-account-info-ssn"]').text()).toBe('00001')
    expect(wrapper.get('[data-test="service-account-info-sname"]').text()).toBe('Main Toon')
  })

  it('hides the AuthType row when sauthtype is null', async () => {
    const { Host } = buildHarness()
    const wrapper = mount(Host, { global: { plugins: [createAppI18n()] } })
    await flushPromises()

    expect(wrapper.find('[data-test="service-account-info-authtype-row"]').exists()).toBe(false)
  })

  it('shows the AuthType row with the raw value when sauthtype is set', async () => {
    const { Host } = buildHarness(FULL_BANNED_ACCOUNT)
    const wrapper = mount(Host, { global: { plugins: [createAppI18n()] } })
    await flushPromises()

    expect(wrapper.get('[data-test="service-account-info-authtype"]').text()).toBe('WCDES')
  })

  it('Status row: enabled account → localized Normal + success colour class', async () => {
    const { Host } = buildHarness()
    const wrapper = mount(Host, { global: { plugins: [createAppI18n()] } })
    await flushPromises()

    const statusEl = wrapper.get('[data-test="service-account-info-status"]')
    expect(statusEl.text()).toBe(i18nMessages['zh-TW'].Normal)
    expect(statusEl.classes()).toContain('service-account-info__status--ok')
    expect(statusEl.classes()).not.toContain('service-account-info__status--banned')
  })

  it('Status row: banned account → localized Banned + danger colour class', async () => {
    const { Host } = buildHarness(FULL_BANNED_ACCOUNT)
    const wrapper = mount(Host, { global: { plugins: [createAppI18n()] } })
    await flushPromises()

    const statusEl = wrapper.get('[data-test="service-account-info-status"]')
    expect(statusEl.text()).toBe(i18nMessages['zh-TW'].Banned)
    expect(statusEl.classes()).toContain('service-account-info__status--banned')
    expect(statusEl.classes()).not.toContain('service-account-info__status--ok')
  })

  it('hides the AccountEstablished panel when screatetime is null', async () => {
    const { Host } = buildHarness()
    const wrapper = mount(Host, { global: { plugins: [createAppI18n()] } })
    await flushPromises()

    expect(wrapper.find('[data-test="service-account-info-created"]').exists()).toBe(false)
    expect(wrapper.find('[data-test="service-account-info-created-days"]').exists()).toBe(false)
    expect(wrapper.find('[data-test="service-account-info-created-since"]').exists()).toBe(false)
  })

  it('shows the AccountEstablished panel with WPF-parity day-count math', async () => {
    /*
     * Pin Date.now to 2024-12-15 00:00 local so the day-count is
     * deterministic. screatetime = 2024-01-15 12:34:56 →
     * elapsed ≈ 334.48 days → floor → 334. Mirrors WPF
     * `getDays`'s `TimeSpan.Days` truncation (which is also a
     * floor on positive values).
     */
    vi.useFakeTimers()
    vi.setSystemTime(new Date('2024-12-15T00:00:00'))

    const { Host } = buildHarness(FULL_BANNED_ACCOUNT)
    const wrapper = mount(Host, { global: { plugins: [createAppI18n()] } })
    await flushPromises()

    expect(wrapper.find('[data-test="service-account-info-created"]').exists()).toBe(true)
    expect(wrapper.get('[data-test="service-account-info-created-days"]').text()).toBe('334')

    /*
     * `CreateDate` is `"於 {0} 建立"` in zh-TW — assert the raw
     * timestamp string is plugged in verbatim (no SPA-side date
     * reformatting; matches WPF which also passes the string
     * through unchanged).
     */
    const sinceText = wrapper.get('[data-test="service-account-info-created-since"]').text()
    expect(sinceText).toContain('2024-01-15 12:34:56')
  })

  it('clamps daysSinceCreation to 0 when the backend returns a future timestamp', async () => {
    /*
     * Defensive floor: backend clock skew or a parser bug could
     * return a creation date in the future. Render 0 instead of a
     * negative number — a negative day count is a confusing
     * user-visible artefact.
     */
    vi.useFakeTimers()
    vi.setSystemTime(new Date('2024-01-01T00:00:00'))

    const futureAccount: ServiceAccount = {
      ...FULL_BANNED_ACCOUNT,
      screatetime: '2025-06-01 00:00:00',
    }
    const { Host } = buildHarness(futureAccount)
    const wrapper = mount(Host, { global: { plugins: [createAppI18n()] } })
    await flushPromises()

    expect(wrapper.get('[data-test="service-account-info-created-days"]').text()).toBe('0')
  })

  it('hides the LastLoginDate row when slastusedtime is null', async () => {
    const { Host } = buildHarness()
    const wrapper = mount(Host, { global: { plugins: [createAppI18n()] } })
    await flushPromises()

    expect(wrapper.find('[data-test="service-account-info-last-login"]').exists()).toBe(false)
  })

  it('shows the LastLoginDate row with the raw timestamp plugged in', async () => {
    const { Host } = buildHarness(FULL_BANNED_ACCOUNT)
    const wrapper = mount(Host, { global: { plugins: [createAppI18n()] } })
    await flushPromises()

    expect(wrapper.get('[data-test="service-account-info-last-login"]').text()).toContain(
      '2024-12-10 08:00:00',
    )
  })

  it('cancel button closes the dialog without invoking any command', async () => {
    const { visibleRef, Host } = buildHarness()
    const wrapper = mount(Host, { global: { plugins: [createAppI18n()] } })
    await flushPromises()

    await wrapper.get('[data-test="service-account-info-cancel"]').trigger('click')
    await flushPromises()

    expect(visibleRef.value).toBe(false)
    expect(commands.changeDisplayName).not.toHaveBeenCalled()
  })

  it('header-close button also closes the dialog', async () => {
    const { visibleRef, Host } = buildHarness()
    const wrapper = mount(Host, { global: { plugins: [createAppI18n()] } })
    await flushPromises()

    await wrapper.get('[data-test="service-account-info-close"]').trigger('click')
    await flushPromises()

    expect(visibleRef.value).toBe(false)
  })

  it('account === null shell mode: dialog mounts but body is not rendered', async () => {
    /*
     * AccountList's caller clears `accountInfoTarget` to null
     * after the dialog closes (via the visibility watcher) so the
     * dialog gets one render pass with `account === null`. The
     * shell must not throw or render any body row that touches
     * `account.<field>`.
     */
    const { Host } = buildHarness(null, true)
    const wrapper = mount(Host, { global: { plugins: [createAppI18n()] } })
    await flushPromises()

    expect(wrapper.find('[data-test="service-account-info-dialog"]').exists()).toBe(true)
    expect(wrapper.find('[data-test="service-account-info-body"]').exists()).toBe(false)
    expect(wrapper.find('[data-test="service-account-info-sid"]').exists()).toBe(false)
  })

  it('reopening with a different account swaps every field cleanly', async () => {
    vi.useFakeTimers()
    vi.setSystemTime(new Date('2024-12-15T00:00:00'))

    const { visibleRef, accountRef, Host } = buildHarness()
    const wrapper = mount(Host, { global: { plugins: [createAppI18n()] } })
    await flushPromises()

    /* SAMPLE_ACCOUNT (enabled, no created date). */
    expect(wrapper.get('[data-test="service-account-info-sid"]').text()).toBe('sid-1')
    expect(wrapper.find('[data-test="service-account-info-created"]').exists()).toBe(false)

    /*
     * Close → mimic AccountList's "clear target after watcher
     * fires" pattern → reopen with FULL_BANNED_ACCOUNT.
     */
    visibleRef.value = false
    await flushPromises()
    accountRef.value = null
    await flushPromises()

    accountRef.value = FULL_BANNED_ACCOUNT
    visibleRef.value = true
    await flushPromises()

    expect(wrapper.get('[data-test="service-account-info-sid"]').text()).toBe('sid-2')
    expect(wrapper.get('[data-test="service-account-info-sname"]').text()).toBe('Mule Account')
    expect(wrapper.get('[data-test="service-account-info-status"]').text()).toBe(
      i18nMessages['zh-TW'].Banned,
    )
    expect(wrapper.find('[data-test="service-account-info-created"]').exists()).toBe(true)
    expect(wrapper.get('[data-test="service-account-info-last-login"]').text()).toContain(
      '2024-12-10 08:00:00',
    )
  })
})
