/**
 * Announcement registry — every notice the app can show, and the rules
 * that decide how insistent each one is.
 *
 * # Levels, least to most insistent
 *
 * | Level      | Behaviour                                                                                   |
 * | ---------- | ------------------------------------------------------------------------------------------- |
 * | `info`     | Closable at once; closing counts as read.                                                     |
 * | `pinned`   | Locked for `countdownSeconds`, then the × and the backdrop both work; closing counts as read.  |
 * | `blocking` | Same countdown, but only the acknowledge button counts as read — any other close brings it back next launch. |
 *
 * `blocking` is the only level that can nag twice, and it earns that by
 * being the only one whose "I read it" is an explicit act.
 *
 * # Publishing a new announcement
 *
 * 1. Prepend an entry here (newest first) with a fresh `id`
 *    (`"YYYY-MM-description"`), its `date`, and a level.
 * 2. Add its `titleKey` string to all three locales in
 *    `src/i18n/messages.ts`, plus whatever body strings it needs.
 * 3. Add a branch for the id in `components/AnnouncementBody.vue` — the
 *    overlay and the archive both render from there, so they can't
 *    drift apart.
 *
 * Old entries stay in the list: the archive reads from it, so deleting
 * one erases it from the user's record.
 */

/** How insistent an announcement is — see the table in the module doc. */
export type AnnouncementLevel = 'info' | 'pinned' | 'blocking'

export interface AnnouncementDef {
  /** Stable id; also the acknowledgement token and the body's key. */
  id: string
  level: AnnouncementLevel
  /** Publication date, `YYYY-MM-DD`. Shown in the archive list. */
  date: string
  /** i18n key for the subject line (overlay title + archive row). */
  titleKey: string
  /**
   * Seconds the acknowledge button stays disabled. Ignored by `info`,
   * which is closable at once.
   */
  countdownSeconds?: number
}

/** External links the shipped announcement bodies use. */
export const ANNOUNCEMENT_MAPLELINK_URL = 'https://github.com/lshw54/maplelink'
export const ANNOUNCEMENT_MORE_INFO_URL = 'https://github.com/pungin/Beanfun/issues/323'

/**
 * The only places either project is published.
 *
 * Shown in full inside the download-source notice, not just linked: the
 * address is the thing a reader compares their own copy against, and a
 * button labelled "downloads page" gives them nothing to compare.
 */
export const ANNOUNCEMENT_BEANFUN_RELEASES_URL = 'https://github.com/pungin/Beanfun/releases'
export const ANNOUNCEMENT_MAPLELINK_RELEASES_URL = 'https://github.com/lshw54/maplelink/releases'

/**
 * Every announcement, **newest first**. The first entry is the one the
 * banner names.
 */
export const ANNOUNCEMENTS: readonly AnnouncementDef[] = [
  {
    id: '2026-09-download-source',
    // `pinned`: someone handed a repackaged build cannot tell it from
    // ours, and the whole notice is one rule they need before they run
    // it. Ten seconds, then closing counts as read — asked once, not
    // every launch.
    level: 'pinned',
    date: '2026-09-03',
    titleKey: 'announcement.downloadSource.title',
    countdownSeconds: 10,
  },
  {
    id: '2026-07-dual-line-development-notice',
    // Dropped from a 30-second lock to `info`: most people have read
    // this by now, and holding everyone for half a minute again earns
    // nothing. It stays in the archive for anyone who wants it.
    level: 'info',
    date: '2026-07-07',
    titleKey: 'announcement.title',
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
 * Key holding the ids whose **banner** the user dismissed. Separate
 * from the read record: dismissing the strip is a display choice, and a
 * later announcement brings its own banner back.
 */
export const ANNOUNCEMENT_BANNER_KEY = 'announcementBannerDismissedIds'

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
 * announcement that existed then.
 */
export const LEGACY_VALUES_MEAN_ID = '2026-07-dual-line-development-notice'

/** `true` for the app-version shapes pre-registry builds stored. */
function isLegacyVersionValue(value: string): boolean {
  return /^\d+\.\d+(\.\d+)*$/.test(value.trim())
}

/** Parse a comma-separated id list (either store, possibly absent). */
export function parseIdList(stored: string | null | undefined): Set<string> {
  const ids = new Set<string>()
  for (const part of (stored ?? '').split(',')) {
    const id = part.trim()
    if (id) ids.add(id)
  }
  return ids
}

/**
 * Build the acknowledged-id set from the current and legacy stored
 * values (either may be absent or empty).
 */
export function parseSeenIds(
  current: string | null | undefined,
  legacy?: string | null | undefined,
): Set<string> {
  const seen = parseIdList(current)
  const legacyValue = (legacy ?? '').trim()
  if (legacyValue) {
    // A legacy id is itself an acknowledgement; a legacy app version
    // acknowledges the notice that shipped in that era.
    seen.add(isLegacyVersionValue(legacyValue) ? LEGACY_VALUES_MEAN_ID : legacyValue)
  }
  return seen
}

/** Serialize an id set back to the stored form. */
export function serializeIds(ids: Iterable<string>): string {
  return Array.from(new Set(ids)).join(',')
}

/**
 * The announcement that should auto-open now, or `null` when none must:
 * the newest one the user has not read.
 *
 * A `blocking` notice returns until it is acknowledged through its own
 * button, which is simply the read record doing its job — no special
 * case here.
 */
export function pendingAnnouncement(seen: Set<string>): AnnouncementDef | null {
  return ANNOUNCEMENTS.find((a) => !seen.has(a.id)) ?? null
}

/** `true` when opening `def` locks the acknowledge button for a while. */
export function hasCountdown(def: AnnouncementDef): boolean {
  return def.level !== 'info' && (def.countdownSeconds ?? 0) > 0
}

/** Countdown length for `def` (0 when it has none). */
export function countdownFor(def: AnnouncementDef): number {
  return hasCountdown(def) ? (def.countdownSeconds ?? 0) : 0
}

/** How the user closed an announcement overlay. */
export type CloseIntent = 'acknowledge' | 'dismiss'

/**
 * `true` when closing `def` via `intent` counts as having read it.
 *
 * Only `blocking` distinguishes the two: its × and backdrop let the
 * user out without recording anything, so the notice returns on the
 * next launch. Everything else treats any close as read.
 */
export function closingCountsAsRead(def: AnnouncementDef, intent: CloseIntent): boolean {
  return def.level === 'blocking' ? intent === 'acknowledge' : true
}
