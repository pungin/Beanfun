/**
 * Announcement registry — every notice the app can show, and the rules
 * that decide when each one pops.
 *
 * # Levels
 *
 * | Level             | Auto-opens              | Countdown            | Remembered |
 * | ----------------- | ----------------------- | -------------------- | ---------- |
 * | `info`            | once, until acknowledged | none (close anytime) | yes        |
 * | `forced`          | once, until acknowledged | `forcedSeconds`      | yes        |
 * | `forcedEveryTime` | **every launch**         | `forcedSeconds`      | never      |
 *
 * `forcedEveryTime` is for notices that must be re-read on every start
 * (a service outage, a data-loss warning); it deliberately never
 * records an acknowledgement, so it comes back next launch.
 *
 * # Publishing a new announcement
 *
 * 1. Prepend an entry to {@link ANNOUNCEMENTS} (newest first) with a
 *    fresh `id` in `"YYYY-MM-description"` form. A new id is unseen by
 *    everyone, which is what makes it pop.
 * 2. Add its `titleKey` / `bodyKeys` strings to **all three** locales in
 *    `src/i18n/messages.ts` (a property test enforces identical key
 *    sets; the key-usage guard knows `announcement.*` is consumed
 *    dynamically from this registry).
 * 3. Pick the level and, for the forced ones, `forcedSeconds`.
 *
 * Old entries stay in the list: the announcement history dialog reads
 * from it, so removing one erases it from the user's record.
 */

/** How insistent an announcement is — see the table in the module doc. */
export type AnnouncementLevel = 'info' | 'forced' | 'forcedEveryTime'

/** An external link rendered under an announcement's body. */
export interface AnnouncementLink {
  /** i18n key for the link label. */
  labelKey: string
  url: string
}

export interface AnnouncementDef {
  /** Stable id; also the acknowledgement token. Never reuse one. */
  id: string
  level: AnnouncementLevel
  /**
   * Seconds the dismiss button stays disabled. Ignored for `info`
   * (which is closable immediately).
   */
  forcedSeconds: number
  /** i18n key for the card title. */
  titleKey: string
  /** i18n keys rendered as body paragraphs, in order. */
  bodyKeys: readonly string[]
  links?: readonly AnnouncementLink[]
  /**
   * Optional bespoke card body. `dualLine` renders the two-track
   * Beanfun / MapleLink layout the #323 notice shipped with; anything
   * else (the default) renders `bodyKeys` as plain paragraphs.
   */
  layout?: 'dualLine'
}

/** External links used by the shipped announcements. */
export const ANNOUNCEMENT_MAPLELINK_URL = 'https://github.com/lshw54/maplelink'
export const ANNOUNCEMENT_MORE_INFO_URL = 'https://github.com/pungin/Beanfun/issues/323'

/**
 * Every announcement, **newest first**. The first entry is what the
 * title-bar banner names.
 */
export const ANNOUNCEMENTS: readonly AnnouncementDef[] = [
  {
    id: '2026-07-dual-line-development-notice',
    // Read once, then out of the way. The countdown is deliberately
    // short: by now the dual-line notice is old news to most users, and
    // the history dialog keeps it one click away forever.
    level: 'forced',
    forcedSeconds: 10,
    titleKey: 'announcement.title',
    bodyKeys: ['announcement.intro'],
    links: [
      { labelKey: 'announcement.maplelinkLink', url: ANNOUNCEMENT_MAPLELINK_URL },
      { labelKey: 'announcement.moreInfoLink', url: ANNOUNCEMENT_MORE_INFO_URL },
    ],
    layout: 'dualLine',
  },
]

/** The newest announcement — the one the banner names. */
export const LATEST_ANNOUNCEMENT: AnnouncementDef = ANNOUNCEMENTS[0]

/** Look an announcement up by id. */
export function announcementById(id: string): AnnouncementDef | undefined {
  return ANNOUNCEMENTS.find((a) => a.id === id)
}

/**
 * Config.xml / localStorage key holding the acknowledged announcement
 * ids, comma-separated.
 */
export const ANNOUNCEMENT_SEEN_KEY = 'announcementSeenIds'

/**
 * The key earlier builds wrote. It held a single value: first the
 * acknowledged **app version** (`"6.0.5"`), later the single
 * announcement id. Read-only now — {@link parseSeenIds} folds it into
 * the id set so nobody is re-forced by the format change.
 */
export const LEGACY_ANNOUNCEMENT_SEEN_KEY = 'announcementSeenVersion'

/**
 * The announcement a legacy value acknowledges. Everyone holding an
 * app-version value had read the issue-#323 dual-line notice — the only
 * announcement that existed then — so that id (and only that id) counts
 * as seen for them.
 */
export const LEGACY_VALUES_MEAN_ID = '2026-07-dual-line-development-notice'

/** `true` for the app-version shapes pre-ID builds stored. */
function isLegacyVersionValue(value: string): boolean {
  return /^\d+\.\d+(\.\d+)*$/.test(value.trim())
}

/**
 * Build the acknowledged-id set from the current and legacy stored
 * values (either store may be absent or empty).
 */
export function parseSeenIds(
  current: string | null | undefined,
  legacy?: string | null | undefined,
): Set<string> {
  const seen = new Set<string>()
  for (const part of (current ?? '').split(',')) {
    const id = part.trim()
    if (id) seen.add(id)
  }
  const legacyValue = (legacy ?? '').trim()
  if (legacyValue) {
    // A legacy id is itself an acknowledgement; a legacy app version
    // acknowledges the notice that shipped in that era.
    seen.add(isLegacyVersionValue(legacyValue) ? LEGACY_VALUES_MEAN_ID : legacyValue)
  }
  return seen
}

/** Serialize an acknowledged-id set back to the stored form. */
export function serializeSeenIds(seen: Iterable<string>): string {
  return Array.from(new Set(seen)).join(',')
}

/**
 * The announcement that should auto-open now, or `null` when none must.
 *
 * A `forcedEveryTime` notice always wins — that is the whole point of
 * the level. Otherwise the newest unacknowledged announcement pops,
 * whatever its level.
 */
export function pendingAnnouncement(seen: Set<string>): AnnouncementDef | null {
  const always = ANNOUNCEMENTS.find((a) => a.level === 'forcedEveryTime')
  if (always) return always
  return ANNOUNCEMENTS.find((a) => !seen.has(a.id)) ?? null
}

/** `true` when opening `def` should count down before it can be closed. */
export function isForcedLevel(def: AnnouncementDef): boolean {
  return def.level === 'forced' || def.level === 'forcedEveryTime'
}

/**
 * `true` when acknowledging `def` should be remembered.
 * `forcedEveryTime` never is — it must return on the next launch.
 */
export function isAcknowledgeable(def: AnnouncementDef): boolean {
  return def.level !== 'forcedEveryTime'
}
