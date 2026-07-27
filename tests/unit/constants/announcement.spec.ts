/**
 * Specs for the announcement registry — the acknowledgement record, the
 * legacy migration, and which notice pops for a given read state.
 */

import { describe, expect, it } from 'vitest'
import {
  ANNOUNCEMENTS,
  LATEST_ANNOUNCEMENT,
  LEGACY_VALUES_MEAN_ID,
  announcementById,
  isAcknowledgeable,
  isForcedLevel,
  parseSeenIds,
  pendingAnnouncement,
  serializeSeenIds,
  type AnnouncementDef,
} from '../../../src/constants/announcement'

const def = (over: Partial<AnnouncementDef>): AnnouncementDef => ({
  id: 'test-id',
  level: 'info',
  forcedSeconds: 0,
  titleKey: 'announcement.title',
  bodyKeys: [],
  ...over,
})

describe('announcement registry', () => {
  it('is newest-first and free of duplicate ids', () => {
    const ids = ANNOUNCEMENTS.map((a) => a.id)
    expect(new Set(ids).size).toBe(ids.length)
    expect(LATEST_ANNOUNCEMENT).toBe(ANNOUNCEMENTS[0])
  })

  it('looks entries up by id', () => {
    expect(announcementById(LATEST_ANNOUNCEMENT.id)).toBe(LATEST_ANNOUNCEMENT)
    expect(announcementById('never-published')).toBeUndefined()
  })

  it('gives every forced announcement a countdown to enforce', () => {
    for (const a of ANNOUNCEMENTS) {
      if (isForcedLevel(a)) {
        expect(a.forcedSeconds, `${a.id} must define a countdown`).toBeGreaterThan(0)
      }
    }
  })
})

describe('parseSeenIds', () => {
  it('reads the comma-separated form', () => {
    expect(parseSeenIds('a,b , c')).toEqual(new Set(['a', 'b', 'c']))
  })

  it('treats empty / missing values as nothing seen', () => {
    expect(parseSeenIds('')).toEqual(new Set())
    expect(parseSeenIds(null)).toEqual(new Set())
    expect(parseSeenIds(undefined, undefined)).toEqual(new Set())
    expect(parseSeenIds(' , ,')).toEqual(new Set())
  })

  it('folds a legacy id value in', () => {
    expect(parseSeenIds(null, '2026-07-dual-line-development-notice')).toEqual(
      new Set(['2026-07-dual-line-development-notice']),
    )
  })

  it('maps a legacy app-version value to the notice of that era', () => {
    // Pre-registry builds stored the acknowledged app version; everyone
    // holding one had read the inaugural notice.
    for (const version of ['6.0.5', '6.0.3', '6.0.5.2607110250']) {
      expect(parseSeenIds(null, version)).toEqual(new Set([LEGACY_VALUES_MEAN_ID]))
    }
  })

  it('does not mistake near-version strings for versions', () => {
    // Anything that isn't version-shaped is taken as an id verbatim.
    expect(parseSeenIds(null, 'v6.0.5')).toEqual(new Set(['v6.0.5']))
  })

  it('merges both stores', () => {
    expect(parseSeenIds('a,b', '6.0.5')).toEqual(new Set(['a', 'b', LEGACY_VALUES_MEAN_ID]))
  })

  it('round-trips through serializeSeenIds', () => {
    const seen = parseSeenIds('a,b,c')
    expect(parseSeenIds(serializeSeenIds(seen))).toEqual(seen)
    // Serializing dedupes.
    expect(serializeSeenIds(['a', 'a', 'b'])).toBe('a,b')
  })
})

describe('pendingAnnouncement', () => {
  it('returns the newest unacknowledged entry', () => {
    expect(pendingAnnouncement(new Set())).toBe(LATEST_ANNOUNCEMENT)
  })

  it('returns null once everything is acknowledged', () => {
    expect(pendingAnnouncement(new Set(ANNOUNCEMENTS.map((a) => a.id)))).toBeNull()
  })
})

describe('level semantics', () => {
  it('info closes immediately and is remembered', () => {
    const info = def({ level: 'info' })
    expect(isForcedLevel(info)).toBe(false)
    expect(isAcknowledgeable(info)).toBe(true)
  })

  it('forced counts down and is remembered', () => {
    const forced = def({ level: 'forced', forcedSeconds: 10 })
    expect(isForcedLevel(forced)).toBe(true)
    expect(isAcknowledgeable(forced)).toBe(true)
  })

  it('forcedEveryTime counts down and is never remembered', () => {
    const always = def({ level: 'forcedEveryTime', forcedSeconds: 5 })
    expect(isForcedLevel(always)).toBe(true)
    // Never recorded → it must come back on the next launch.
    expect(isAcknowledgeable(always)).toBe(false)
  })
})
