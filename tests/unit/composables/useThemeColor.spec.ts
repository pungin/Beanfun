import { beforeEach, describe, expect, it } from 'vitest'

import {
  DEFAULT_PRIMARY_COLOR,
  THEME_PRESETS,
  mixHexColor,
  parseHexColor,
  resolvePrimaryColor,
  setPrimaryColor,
  useThemeColor,
} from '../../../src/composables/useThemeColor'

describe('THEME_PRESETS', () => {
  it('exposes the 8 named presets from the WPF mockup design system', () => {
    expect(THEME_PRESETS).toHaveLength(8)
    const names = THEME_PRESETS.map((p) => p.name)
    expect(names).toEqual([
      'orange',
      'green',
      'lightblue',
      'pink',
      'gold',
      'silver',
      'black',
      'white',
    ])
  })

  it('every preset.primary parses as a valid hex color', () => {
    for (const preset of THEME_PRESETS) {
      expect(() => parseHexColor(preset.primary)).not.toThrow()
    }
  })

  it('the default primary equals the first preset (orange)', () => {
    expect(DEFAULT_PRIMARY_COLOR).toBe(THEME_PRESETS[0].primary)
  })
})

describe('parseHexColor', () => {
  it('accepts a 6-digit hex with leading #', () => {
    expect(parseHexColor('#ff8201')).toEqual([0xff, 0x82, 0x01])
  })

  it('accepts a 6-digit hex without leading #', () => {
    expect(parseHexColor('ff8201')).toEqual([0xff, 0x82, 0x01])
  })

  it('expands a 3-digit shorthand', () => {
    expect(parseHexColor('#abc')).toEqual([0xaa, 0xbb, 0xcc])
  })

  it('normalizes case and trims whitespace', () => {
    expect(parseHexColor(' #FF8201 ')).toEqual([0xff, 0x82, 0x01])
  })

  it('throws RangeError for non-hex input', () => {
    expect(() => parseHexColor('not-a-color')).toThrow(RangeError)
    expect(() => parseHexColor('#xyz123')).toThrow(RangeError)
    expect(() => parseHexColor('#1234')).toThrow(RangeError)
  })
})

describe('resolvePrimaryColor', () => {
  it('maps WPF named colors to their P11 preset hex (documented aliases)', () => {
    // Pairs come straight from WPF_NAMED_COLOR_ALIASES; one per entry
    // so adding / removing an alias in the source forces a visible
    // test diff.
    expect(resolvePrimaryColor('White')).toBe('#555555')
    expect(resolvePrimaryColor('Black')).toBe('#1A1A1A')
    expect(resolvePrimaryColor('LightBlue')).toBe('#0B6E99')
    expect(resolvePrimaryColor('Pink')).toBe('#D85A88')
    expect(resolvePrimaryColor('Gold')).toBe('#C9A227')
    expect(resolvePrimaryColor('Silver')).toBe('#7A7A7A')
  })

  it('matches aliases case-insensitively', () => {
    expect(resolvePrimaryColor('lightblue')).toBe('#0B6E99')
    expect(resolvePrimaryColor('LIGHTBLUE')).toBe('#0B6E99')
    expect(resolvePrimaryColor('LiGhTbLuE')).toBe('#0B6E99')
  })

  it('trims leading / trailing whitespace on alias input', () => {
    expect(resolvePrimaryColor('  LightBlue  ')).toBe('#0B6E99')
    expect(resolvePrimaryColor('\tGold\n')).toBe('#C9A227')
  })

  it('passes hex input through untouched (pre or post resolver)', () => {
    // resolvePrimaryColor doesn't validate hex — it only translates
    // legacy named colors. Callers combine it with parseHexColor.
    expect(resolvePrimaryColor('#FF8201')).toBe('#FF8201')
    expect(resolvePrimaryColor('#B6DE8E')).toBe('#B6DE8E')
    expect(resolvePrimaryColor('#abc')).toBe('#abc')
  })

  it('passes unknown strings through so parseHexColor can reject them', () => {
    // Round-trip property: any non-alias string comes back verbatim.
    expect(resolvePrimaryColor('not-a-color')).toBe('not-a-color')
    expect(resolvePrimaryColor('SlateGray')).toBe('SlateGray') // valid WPF color, not in WPF Settings ComboBox
  })
})

describe('mixHexColor', () => {
  it('returns the base when weight is 0', () => {
    expect(mixHexColor('#ff8201', '#ffffff', 0)).toBe('#ff8201')
  })

  it('returns mixWith when weight is 1', () => {
    expect(mixHexColor('#ff8201', '#ffffff', 1)).toBe('#ffffff')
  })

  it('clamps weights below 0 and above 1', () => {
    expect(mixHexColor('#ff8201', '#ffffff', -0.5)).toBe('#ff8201')
    expect(mixHexColor('#ff8201', '#ffffff', 2)).toBe('#ffffff')
  })

  it('mixes 50/50 as the per-channel arithmetic mean (rounded)', () => {
    // (0xff + 0x00) / 2 = 0x7f.5 → rounds to 0x80
    // (0x82 + 0xff) / 2 = 0xc0.5 → rounds to 0xc1
    // (0x01 + 0x00) / 2 = 0x00.5 → rounds to 0x01
    expect(mixHexColor('#ff8201', '#00ff00', 0.5)).toBe('#80c101')
  })
})

describe('setPrimaryColor', () => {
  let target: HTMLElement

  beforeEach(() => {
    target = document.createElement('div')
  })

  it('sets --el-color-primary plus all derived ELP shades', () => {
    setPrimaryColor('#ff8201', target)

    expect(target.style.getPropertyValue('--el-color-primary')).toBe('#ff8201')

    // Derived shades — values verified against the documented mixing formula.
    expect(target.style.getPropertyValue('--el-color-primary-dark-2')).toBe(
      mixHexColor('#ff8201', '#000000', 0.2),
    )
    expect(target.style.getPropertyValue('--el-color-primary-light-3')).toBe(
      mixHexColor('#ff8201', '#ffffff', 0.3),
    )
    expect(target.style.getPropertyValue('--el-color-primary-light-5')).toBe(
      mixHexColor('#ff8201', '#ffffff', 0.5),
    )
    expect(target.style.getPropertyValue('--el-color-primary-light-7')).toBe(
      mixHexColor('#ff8201', '#ffffff', 0.7),
    )
    expect(target.style.getPropertyValue('--el-color-primary-light-8')).toBe(
      mixHexColor('#ff8201', '#ffffff', 0.8),
    )
    expect(target.style.getPropertyValue('--el-color-primary-light-9')).toBe(
      mixHexColor('#ff8201', '#ffffff', 0.9),
    )
  })

  it('normalizes 3-digit hex input to 6-digit lowercase output', () => {
    setPrimaryColor('#abc', target)
    expect(target.style.getPropertyValue('--el-color-primary')).toBe('#aabbcc')
  })

  it('throws RangeError for invalid hex input without mutating the target', () => {
    expect(() => setPrimaryColor('not-a-color', target)).toThrow(RangeError)
    expect(target.style.getPropertyValue('--el-color-primary')).toBe('')
  })

  it('accepts legacy WPF named-color aliases and applies the P11 preset hex', () => {
    // Regression: `Config.xml` written by the old WPF client stores
    // raw WPF color names like "LightBlue"; setPrimaryColor used to
    // throw because only the hex path was wired. Now the alias
    // resolver translates it to the P11 `lightblue` preset
    // (`#0B6E99`) before parseHexColor runs.
    setPrimaryColor('LightBlue', target)
    expect(target.style.getPropertyValue('--el-color-primary')).toBe('#0b6e99')
  })

  it('falls back to document.documentElement when no target is provided', () => {
    setPrimaryColor('#5C8430')
    expect(document.documentElement.style.getPropertyValue('--el-color-primary')).toBe('#5c8430')
  })
})

describe('useThemeColor', () => {
  it('exposes the same presets and setter as the module-level exports', () => {
    const t = useThemeColor()
    expect(t.presets).toBe(THEME_PRESETS)
    expect(t.defaultPrimary).toBe(DEFAULT_PRIMARY_COLOR)
    expect(t.setPrimaryColor).toBe(setPrimaryColor)
  })

  it('getCurrentPrimary reads the applied custom property', () => {
    const target = document.createElement('div')
    target.style.setProperty('--el-color-primary', '#0b6e99')
    document.body.appendChild(target)
    try {
      expect(useThemeColor().getCurrentPrimary(target)).toBe('#0b6e99')
    } finally {
      document.body.removeChild(target)
    }
  })
})
