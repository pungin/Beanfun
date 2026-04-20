<script setup lang="ts">
/**
 * Region picker — first stop in the login flow.
 *
 * # WPF parity
 *
 * Mirrors `Beanfun/Windows/LoginRegionSelection.xaml(.cs)`:
 *
 * 1. Two clickable region tiles: TW (`tw.beanfun.com`) and HK
 *    (`hk.beanfun.com`).
 * 2. On click, persists the choice to Config.xml under the legacy
 *    `loginRegion` key (so old WPF installs reading the same file see
 *    the value preserved verbatim).
 * 3. Navigates to the next step in the login funnel — the regular
 *    id-pass form. Future P12.1 D-steps may swap the destination
 *    based on the previously-used login method (the WPF
 *    `MainWindow.Initialize()` logic).
 *
 * # WPF deviation (intentional)
 *
 * WPF only popped this window on `LoadDataError` (catch path);
 * normal boots silently used the cached `App.LoginRegion`. We elevate
 * it to a real route here because:
 *
 * - The mockup design (`mockups/LoginRegionSelection.html`) expects
 *   it to be a deliberate first-time-setup screen.
 * - SPA navigation feels more natural with route-driven UI than with
 *   imperative `ShowDialog()` calls.
 *
 * The "skip picker if already chosen" logic is a P12.1 D10 concern
 * (router `beforeEach` guard); D2 keeps the picker reachable so the
 * first-launch + manual region-switch flows both work.
 */

import { computed, onMounted } from 'vue'
import { useI18n } from 'vue-i18n'
import { useRouter } from 'vue-router'
import { useConfigStore } from '../stores/config'
import type { LoginRegion } from '../types/bindings'

defineOptions({ name: 'LoginRegionSelection' })

const { t } = useI18n()
const router = useRouter()
const config = useConfigStore()

/**
 * Skip the region picker if a region is already saved — go
 * straight to the login form. First-launch users see the picker.
 */
onMounted(() => {
  if (config.get('loginRegion')) {
    void router.replace('/login/id-pass')
  }
})

/**
 * Tile descriptors. Kept declarative so the template stays a flat
 * `v-for` rather than two near-duplicate `<button>` blocks (DRY).
 * Per-region runtime hints (e.g. "TOTP supported on HK") live in
 * `messages.ts` under `loginRegion.*Hint` so they translate.
 */
type RegionTile = {
  region: LoginRegion
  labelKey: string
  hostHint: string
  hintKey: string
  hintIcon: string
}

const TILES: readonly RegionTile[] = [
  {
    region: 'TW',
    labelKey: 'Taiwan',
    hostHint: 'tw.beanfun.com',
    hintKey: 'loginRegion.defaultBadge',
    hintIcon: 'check_circle',
  },
  {
    region: 'HK',
    labelKey: 'HongKong',
    hostHint: 'hk.beanfun.com',
    hintKey: 'loginRegion.totpHint',
    hintIcon: 'info',
  },
]

const heading = computed(() => t('BeanfunRegionSelected'))
const subline = computed(() => t('loginRegion.subline'))
const tip = computed(() => t('loginRegion.tip'))

/** The currently saved region from Config.xml, defaults to TW. */
const currentRegion = computed(() => (config.get('loginRegion') as string | undefined) ?? 'TW')

async function selectRegion(region: LoginRegion): Promise<void> {
  /*
   * Match the WPF `loginRegion` key verbatim so an existing user
   * picking up the new build sees their region choice survive.
   */
  await config.set('loginRegion', region)

  /*
   * Forward to the regular id-pass form by default. The "remember
   * last login method" branch (WPF `loginMethodInit`) lands in the
   * D10 router guard once `/login/qr` and `/login/gamepass` exist as
   * destinations.
   */
  await router.push('/login/id-pass')
}
</script>

<template>
  <section class="region-picker">
    <header class="region-picker__header">
      <h2 class="region-picker__heading">{{ heading }}</h2>
      <p class="region-picker__subline">{{ subline }}</p>
    </header>

    <div class="region-picker__grid">
      <button
        v-for="tile in TILES"
        :key="tile.region"
        type="button"
        class="region-tile"
        :class="{ 'region-tile--current': currentRegion === tile.region }"
        :data-region="tile.region"
        @click="selectRegion(tile.region)"
      >
        <div class="region-tile__icon">
          <span class="material-symbols-outlined region-tile__flag">flag</span>
        </div>
        <div class="region-tile__label">{{ t(tile.labelKey) }}</div>
        <div class="region-tile__host">{{ tile.hostHint }}</div>
        <div class="region-tile__badge">
          <span class="material-symbols-outlined region-tile__badge-icon">{{ tile.hintIcon }}</span>
          {{ t(tile.hintKey) }}
        </div>
      </button>
    </div>

    <div class="region-picker__tip">
      <span class="material-symbols-outlined region-picker__tip-icon">tips_and_updates</span>
      <span>{{ tip }}</span>
    </div>
  </section>
</template>

<style scoped>
.region-picker {
  display: flex;
  flex-direction: column;
  gap: 1.5rem;
}

.region-picker__header {
  text-align: center;
}

.region-picker__heading {
  margin: 0;
  font-size: 1.5rem;
  font-weight: 800;
  letter-spacing: -0.01em;
  color: var(--bf-on-surface, #1f1a16);
}

.region-picker__subline {
  margin: 0.375rem 0 0;
  font-size: 0.8125rem;
  color: #54443a;
}

.region-picker__grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 1rem;
}

.region-tile {
  appearance: none;
  background: rgba(255, 255, 255, 0.7);
  backdrop-filter: blur(20px) saturate(1.2);
  border: 1px solid rgba(255, 255, 255, 0.85);
  border-radius: 10px;
  padding: 1.25rem 1rem;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 0.75rem;
  cursor: pointer;
  font: inherit;
  color: inherit;
  transition:
    transform 180ms ease,
    box-shadow 220ms ease,
    background 150ms ease;
}

.region-tile:hover,
.region-tile:focus-visible {
  transform: translateY(-3px);
  background: rgba(255, 255, 255, 0.92);
  box-shadow: 0 10px 24px color-mix(in srgb, var(--bf-primary, #954a00) 22%, transparent);
  outline: none;
}

.region-tile:focus-visible {
  border-color: var(--bf-primary-container, #ff8201);
}

.region-tile--current {
  border-color: var(--bf-primary-container, #ff8201);
  box-shadow: 0 0 0 2px color-mix(in srgb, var(--bf-primary-container, #ff8201) 30%, transparent);
}

.region-tile__icon {
  width: 72px;
  height: 72px;
  border-radius: 18px;
  background: linear-gradient(
    135deg,
    var(--bf-primary-container, #ff8201),
    var(--bf-primary, #954a00)
  );
  color: #fff;
  display: grid;
  place-items: center;
  box-shadow: 0 8px 20px color-mix(in srgb, var(--bf-primary, #954a00) 35%, transparent);
}

.region-tile__flag {
  font-size: 36px;
}

.region-tile__label {
  font-size: 1.125rem;
  font-weight: 700;
}

.region-tile__host {
  font-size: 0.75rem;
  color: #54443a;
  font-family: 'JetBrains Mono', 'Cascadia Code', 'Consolas', monospace;
}

.region-tile__badge {
  margin-top: 0.25rem;
  font-size: 0.75rem;
  padding: 0.25rem 0.5rem;
  border-radius: 9999px;
  background: color-mix(in srgb, var(--bf-primary-container, #ff8201) 30%, transparent);
  color: var(--bf-primary, #954a00);
  font-weight: 600;
  display: inline-flex;
  align-items: center;
  gap: 0.25rem;
}

.region-tile__badge-icon {
  font-size: 14px;
}

.region-picker__tip {
  margin: 0;
  padding: 0.75rem 1rem;
  border-radius: 8px;
  background: rgba(0, 0, 0, 0.04);
  font-size: 0.75rem;
  color: #54443a;
  display: flex;
  align-items: center;
  gap: 0.5rem;
}

.region-picker__tip-icon {
  font-size: 18px;
  color: var(--bf-primary-container, #ff8201);
  flex-shrink: 0;
}
</style>
