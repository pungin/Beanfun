/**
 * Specs for the announcement seen-value predicate — the piece that
 * decides whether the forced read fires (see
 * src/constants/announcement.ts for the mechanism).
 */

import { describe, expect, it } from 'vitest'
import {
  ANNOUNCEMENT_ID,
  LEGACY_VERSION_VALUES_MEAN_ID,
  isAnnouncementSeenValue,
} from '../../../src/constants/announcement'

describe('isAnnouncementSeenValue', () => {
  it('accepts the current announcement ID', () => {
    expect(isAnnouncementSeenValue(ANNOUNCEMENT_ID)).toBe(true)
  })

  it('rejects empty / missing values', () => {
    expect(isAnnouncementSeenValue('')).toBe(false)
    expect(isAnnouncementSeenValue(null)).toBe(false)
    expect(isAnnouncementSeenValue(undefined)).toBe(false)
  })

  it('rejects a different announcement ID', () => {
    expect(isAnnouncementSeenValue('2020-01-some-older-announcement')).toBe(false)
  })

  it('accepts legacy app-version values while the inaugural ID is shipped', () => {
    // Pre-ID builds stored the acknowledged app version; those users read
    // the current (issue #323) notice and must not be re-forced by the
    // mechanism change itself.
    expect(ANNOUNCEMENT_ID).toBe(LEGACY_VERSION_VALUES_MEAN_ID)
    expect(isAnnouncementSeenValue('6.0.5')).toBe(true)
    expect(isAnnouncementSeenValue('6.0.5.2607110250')).toBe(true)
    expect(isAnnouncementSeenValue('6.0.3')).toBe(true)
  })

  it('rejects values that merely resemble versions loosely', () => {
    expect(isAnnouncementSeenValue('6')).toBe(false)
    expect(isAnnouncementSeenValue('v6.0.5')).toBe(false)
    expect(isAnnouncementSeenValue('6.0.5-beta')).toBe(false)
  })
})
