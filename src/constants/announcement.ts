/**
 * Announcement identity & publish knobs (issue #323 notice; mechanism
 * reworked so app updates alone never re-force the read).
 *
 * # How the forced announcement works
 *
 * - The forced notice is identified by {@link ANNOUNCEMENT_ID}, a plain
 *   revision string in `"YYYY-MM-description"` form — **not** the app
 *   version. Acknowledging the notice stores this ID (Config.xml +
 *   localStorage under `announcementSeenVersion`).
 * - On boot, `AnnouncementModal.vue` compares the stored value against
 *   {@link ANNOUNCEMENT_ID}: equal → nothing pops; different → the
 *   notice auto-opens with the {@link ANNOUNCEMENT_FORCED_SECONDS}
 *   forced-read countdown.
 * - So updating the app does NOT re-force an unchanged announcement,
 *   and publishing a new announcement is exactly one edit here:
 *
 * # Publishing a new announcement
 *
 * 1. Bump {@link ANNOUNCEMENT_ID} to a new unique string
 *    (`"2026-09-whatever"`) — this resets every user's seen state.
 * 2. Update the `announcement.*` strings for all three locales in
 *    `src/i18n/messages.ts` (the key sets must stay identical across
 *    locales — a property test enforces it).
 * 3. Adjust {@link ANNOUNCEMENT_FORCED_SECONDS} / the URLs below only
 *    if the new notice needs it.
 */

/**
 * Revision ID of the announcement currently shipped. Bumping this (and
 * only this) re-triggers the forced read for every user.
 */
export const ANNOUNCEMENT_ID = '2026-07-dual-line-development-notice'

/**
 * The ID the pre-ID mechanism maps to. Before this mechanism landed the
 * seen key stored the acknowledged **app version** (`"6.0.5"`, …), and
 * everyone who acknowledged any of those versions read the issue-#323
 * dual-line notice — the only announcement that existed. While
 * {@link ANNOUNCEMENT_ID} still identifies that same notice, a legacy
 * version-shaped value therefore counts as seen (no innocent re-force
 * on update); as soon as the ID is bumped to a *new* announcement the
 * clause self-deactivates and legacy users are forced like everyone
 * else. Do not update this constant when bumping the ID.
 */
export const LEGACY_VERSION_VALUES_MEAN_ID = '2026-07-dual-line-development-notice'

/** Seconds the user must wait before the forced notice can be dismissed. */
export const ANNOUNCEMENT_FORCED_SECONDS = 30

/** External links rendered inside the notice card. */
export const ANNOUNCEMENT_MAPLELINK_URL = 'https://github.com/lshw54/maplelink'
export const ANNOUNCEMENT_MORE_INFO_URL = 'https://github.com/pungin/Beanfun/issues/323'

/**
 * `true` when a stored seen-value acknowledges the **current**
 * announcement — either the ID itself, or (only while the current ID is
 * still {@link LEGACY_VERSION_VALUES_MEAN_ID}) a legacy app-version
 * value written by builds that predate the ID mechanism.
 */
export function isAnnouncementSeenValue(stored: string | null | undefined): boolean {
  if (!stored) return false
  if (stored === ANNOUNCEMENT_ID) return true
  return (
    ANNOUNCEMENT_ID === LEGACY_VERSION_VALUES_MEAN_ID && /^\d+\.\d+(\.\d+)*$/.test(stored.trim())
  )
}
