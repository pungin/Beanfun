<script setup lang="ts">
/**
 * The body of one announcement, keyed by id.
 *
 * Both places that show a notice — the overlay and the archive — render
 * this, so they can never drift into showing different text for the
 * same announcement. Publishing a new notice is a `v-else-if` branch
 * here plus an entry in `src/constants/announcement.ts`.
 *
 * Only the *body* lives here: the title, level and date come from the
 * registry, and the surrounding chrome (countdown button, close
 * affordances, archive rows) belongs to the caller.
 */

import { useI18n } from 'vue-i18n'

import { commands } from '../types/bindings'
import { safeInvoke } from '../services/invoke'
import { ANNOUNCEMENT_MAPLELINK_URL, ANNOUNCEMENT_MORE_INFO_URL } from '../constants/announcement'

defineOptions({ name: 'AnnouncementBody' })

defineProps<{ id: string }>()

const { t } = useI18n()

async function open(url: string): Promise<void> {
  await safeInvoke(commands.openUrl(url))
}
</script>

<template>
  <div class="ann-body" :data-testid="`announcement-body-${id}`">
    <template v-if="id === '2026-07-dual-line-development-notice'">
      <p class="ann-body__intro">{{ t('announcement.intro') }}</p>

      <div class="ann-body__tracks">
        <div class="ann-body__track">
          <span class="ann-body__dot ann-body__dot--beanfun" aria-hidden="true"></span>
          <div>
            <div class="ann-body__track-name">Beanfun</div>
            <div class="ann-body__track-desc">{{ t('announcement.beanfun') }}</div>
          </div>
        </div>
        <div class="ann-body__track">
          <span class="ann-body__dot ann-body__dot--maple" aria-hidden="true"></span>
          <div>
            <div class="ann-body__track-name">MapleLink</div>
            <div class="ann-body__track-desc">{{ t('announcement.maplelink') }}</div>
          </div>
        </div>
      </div>

      <div class="ann-body__links">
        <a
          class="ann-body__link"
          data-testid="announcement-maplelink"
          @click="open(ANNOUNCEMENT_MAPLELINK_URL)"
        >
          MapleLink ↗
        </a>
        <a
          class="ann-body__link"
          data-testid="announcement-issue"
          @click="open(ANNOUNCEMENT_MORE_INFO_URL)"
        >
          {{ t('announcement.moreInfoLink') }} ↗
        </a>
      </div>
    </template>

    <!-- Unknown id: the registry has an entry whose body was never
         written here. Say so rather than render an empty card. -->
    <p v-else class="ann-body__intro">{{ t('announcement.bodyMissing') }}</p>
  </div>
</template>

<style scoped>
.ann-body__intro {
  margin: 0 0 18px;
  font-size: 0.9rem;
  line-height: 1.7;
  color: var(--bf-on-surface-variant, var(--bf-on-surface, #54443a));
}

.ann-body__tracks {
  display: flex;
  flex-direction: column;
  gap: 10px;
  margin-bottom: 20px;
}

.ann-body__track {
  display: flex;
  gap: 12px;
  padding: 12px 14px;
  border-radius: var(--bf-radius-card, 10px);
  background: color-mix(in srgb, var(--bf-on-surface, #000) 6%, transparent);
}

.ann-body__dot {
  flex: 0 0 auto;
  width: 10px;
  height: 10px;
  margin-top: 5px;
  border-radius: 50%;
}

.ann-body__dot--beanfun {
  background: #ff8201;
}

.ann-body__dot--maple {
  background: #3aa0ff;
}

.ann-body__track-name {
  font-size: 0.9rem;
  font-weight: 800;
  margin-bottom: 3px;
}

.ann-body__track-desc {
  font-size: 0.82rem;
  line-height: 1.6;
  color: var(--bf-on-surface-variant, var(--bf-on-surface, #54443a));
}

.ann-body__links {
  display: flex;
  flex-wrap: wrap;
  gap: 8px 18px;
  margin-bottom: 20px;
}

.ann-body__link {
  cursor: pointer;
  font-size: 0.82rem;
  font-weight: 700;
  color: var(--bf-primary, #954a00);
  text-decoration: none;
}

.ann-body__link:hover {
  text-decoration: underline;
}
</style>
