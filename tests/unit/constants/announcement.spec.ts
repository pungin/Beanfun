/**
 * Specs for the announcement registry — the read record, the legacy
 * migration, and what each level implies.
 */

import { describe, expect, it } from 'vitest'
import {
  ANNOUNCEMENTS,
  LATEST_ANNOUNCEMENT,
  LEGACY_VALUES_MEAN_ID,
  announcementById,
  closingCountsAsRead,
  countdownFor,
  hasCountdown,
  parseIdList,
  parseSeenIds,
  pendingAnnouncement,
  serializeIds,
  type AnnouncementDef,
} from '../../../src/constants/announcement'

const def = (over: Partial<AnnouncementDef>): AnnouncementDef => ({
  id: 'test-id',
  level: 'info',
  date: '2026-01-01',
  titleKey: 'announcement.title',
  ...over,
})

describe('announcement registry', () => {
  it('is newest-first and free of duplicate ids', () => {
    const ids = ANNOUNCEMENTS.map((a) => a.id)
    expect(new Set(ids).size).toBe(ids.length)
    expect(LATEST_ANNOUNCEMENT).toBe(ANNOUNCEMENTS[0])
  })

  it('dates every entry so the archive can show one', () => {
    for (const a of ANNOUNCEMENTS) {
      expect(a.date, `${a.id} needs a date`).toMatch(/^\d{4}-\d{2}-\d{2}$/)
    }
  })

  it('gives every countdown level a countdown to enforce', () => {
    for (const a of ANNOUNCEMENTS) {
      if (a.level !== 'info') {
        expect(a.countdownSeconds, `${a.id} must define a countdown`).toBeGreaterThan(0)
      }
    }
  })

  it('looks entries up by id', () => {
    expect(announcementById(LATEST_ANNOUNCEMENT.id)).toBe(LATEST_ANNOUNCEMENT)
    expect(announcementById('never-published')).toBeUndefined()
  })
})

describe('level semantics', () => {
  it('info has no countdown and any close counts as read', () => {
    const info = def({ level: 'info', countdownSeconds: 30 })
    // Even a stray countdown value is ignored for info.
    expect(hasCountdown(info)).toBe(false)
    expect(countdownFor(info)).toBe(0)
    expect(closingCountsAsRead(info, 'acknowledge')).toBe(true)
    expect(closingCountsAsRead(info, 'dismiss')).toBe(true)
  })

  it('pinned counts down, then any close counts as read', () => {
    const pinned = def({ level: 'pinned', countdownSeconds: 10 })
    expect(hasCountdown(pinned)).toBe(true)
    expect(countdownFor(pinned)).toBe(10)
    expect(closingCountsAsRead(pinned, 'acknowledge')).toBe(true)
    // The × and the backdrop are legitimate exits once unlocked.
    expect(closingCountsAsRead(pinned, 'dismiss')).toBe(true)
  })

  it('blocking counts down and only the acknowledge button counts as read', () => {
    const blocking = def({ level: 'blocking', countdownSeconds: 10 })
    expect(hasCountdown(blocking)).toBe(true)
    expect(closingCountsAsRead(blocking, 'acknowledge')).toBe(true)
    // Leaving any other way means it returns next launch.
    expect(closingCountsAsRead(blocking, 'dismiss')).toBe(false)
  })
})

describe('parseSeenIds / parseIdList', () => {
  it('reads the comma-separated form', () => {
    expect(parseIdList('a,b , c')).toEqual(new Set(['a', 'b', 'c']))
  })

  it('treats empty / missing values as nothing seen', () => {
    expect(parseSeenIds('')).toEqual(new Set())
    expect(parseSeenIds(null)).toEqual(new Set())
    expect(parseSeenIds(undefined, undefined)).toEqual(new Set())
    expect(parseIdList(' , ,')).toEqual(new Set())
  })

  it('folds a legacy id value in', () => {
    expect(parseSeenIds(null, LEGACY_VALUES_MEAN_ID)).toEqual(new Set([LEGACY_VALUES_MEAN_ID]))
  })

  it('maps a legacy app-version value to the notice of that era', () => {
    for (const version of ['6.0.5', '6.0.3', '6.0.5.2607110250']) {
      expect(parseSeenIds(null, version)).toEqual(new Set([LEGACY_VALUES_MEAN_ID]))
    }
  })

  it('does not mistake near-version strings for versions', () => {
    expect(parseSeenIds(null, 'v6.0.5')).toEqual(new Set(['v6.0.5']))
  })

  it('merges both stores', () => {
    expect(parseSeenIds('a,b', '6.0.5')).toEqual(new Set(['a', 'b', LEGACY_VALUES_MEAN_ID]))
  })

  it('round-trips through serializeIds and dedupes', () => {
    const seen = parseSeenIds('a,b,c')
    expect(parseSeenIds(serializeIds(seen))).toEqual(seen)
    expect(serializeIds(['a', 'a', 'b'])).toBe('a,b')
  })
})

describe('pendingAnnouncement', () => {
  it('returns the newest unread entry', () => {
    expect(pendingAnnouncement(new Set())).toBe(LATEST_ANNOUNCEMENT)
  })

  it('returns null once everything is read', () => {
    expect(pendingAnnouncement(new Set(ANNOUNCEMENTS.map((a) => a.id)))).toBeNull()
  })
})
