import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'

import type {
  CommandError,
  GameInfoBundle,
  GameService,
  GameIniEntry,
  Result,
} from '../../../src/types/bindings'

vi.mock('element-plus', () => ({ ElMessage: { error: vi.fn() } }))

vi.mock('../../../src/types/bindings', () => ({
  commands: {
    listGames: vi.fn(),
  },
}))

import { commands } from '../../../src/types/bindings'
import {
  UNCONNECTED_GAME_CODES,
  gameCodeOf,
  imageUrl,
  useGameStore,
} from '../../../src/stores/game'

const ok = <T>(data: T): Promise<Result<T, CommandError>> => Promise.resolve({ status: 'ok', data })
const err = (error: CommandError): Promise<Result<never, CommandError>> =>
  Promise.resolve({ status: 'error', error })

const MAPLE: GameService = {
  name: 'MapleStory',
  service_code: '610074',
  service_region: 'T9',
  website_url: 'https://maplestory.beanfun.com/',
  xlarge_image_name: '610074.jpg',
  large_image_name: '610074_large.jpg',
  small_image_name: '610074_small.jpg',
}

const KART: GameService = {
  name: 'KartRider',
  service_code: '610075',
  service_region: 'T9',
  website_url: 'https://kart.beanfun.com/',
  xlarge_image_name: '610075.jpg',
  large_image_name: '610075_large.jpg',
  small_image_name: '610075_small.jpg',
}

const UNCONNECTED: GameService = {
  name: 'Unconnected Sample',
  service_code: '610153',
  service_region: 'TN',
  website_url: '',
  xlarge_image_name: '610153.jpg',
  large_image_name: '610153_large.jpg',
  small_image_name: '610153_small.jpg',
}

const MAPLE_INI: GameIniEntry = {
  exe: 'C:\\MapleStory\\MapleStory.exe',
  login_action_type: '8',
  win_class_name: 'MapleStoryClass',
  dir_value_name: 'ExecPath',
  dir_reg: 'SOFTWARE\\Gamania\\MapleStory',
}

const KART_INI: GameIniEntry = {
  exe: '',
  login_action_type: '',
  win_class_name: '',
  dir_value_name: '',
  dir_reg: '',
}

const BUNDLE: GameInfoBundle = {
  ini: { '610074_T9': MAPLE_INI, '610075_T9': KART_INI } as Record<string, GameIniEntry>,
  services: [MAPLE, KART, UNCONNECTED],
}

const SAMPLE_ERROR: CommandError = {
  code: 'game.service_list_missing',
  message: 'Services.ServiceList literal not found',
  details: null,
}

describe('useGameStore — pure helpers (no Pinia)', () => {
  it('gameCodeOf joins service_code and service_region with underscore', () => {
    expect(gameCodeOf('610074', 'T9')).toBe('610074_T9')
    expect(gameCodeOf('610153', 'TN')).toBe('610153_TN')
  })

  it('imageUrl falls back to the unified images.beanfun.com base for a TW bare filename', () => {
    // Bare-filename branch — mirrors WPF L494 else-branch.
    // Live upstream rows ship full URLs (covered below); this
    // case only fires if Beanfun ever regresses to the legacy
    // bare-name shape. Base must match Rust `image_base_url`.
    expect(imageUrl('610074.jpg', 'TW')).toBe('https://images.beanfun.com/GameZone/610074.jpg')
  })

  it('imageUrl falls back to the unified images.beanfun.com base for an HK bare filename', () => {
    // Same single-host base as TW (TW/HK currently share
    // `images.beanfun.com`); the HK arm exists to lock
    // behaviour parity in case of a future host re-split.
    expect(imageUrl('610074.jpg', 'HK')).toBe('https://images.beanfun.com/GameZone/610074.jpg')
  })

  it('imageUrl passes a https:// full URL through unchanged (WPF L494 mirror)', () => {
    // 2026-04 audit: every live `Service*ImageName` ships as a
    // full URL like this. The passthrough branch must NOT wrap
    // it with the base prefix (the F3 first-attempt regression
    // produced `…/game_zone/https://…` and the lenient server
    // returned 200 + 0 byte; this test guards that).
    const full = 'https://images.beanfun.com/GameZone/20170110120804222.jpg'
    expect(imageUrl(full, 'TW')).toBe(full)
    expect(imageUrl(full, 'HK')).toBe(full)
  })

  it('imageUrl passes a http:// full URL through unchanged', () => {
    // Defensive coverage for the http:// branch of the WPF L494
    // condition. The launcher must not silently upgrade the
    // scheme — that's a server-side concern (and the WebView's
    // mixed-content policy is permissive for `<img>`).
    const full = 'http://images.beanfun.com/GameZone/legacy.jpg'
    expect(imageUrl(full, 'TW')).toBe(full)
  })

  it('UNCONNECTED_GAME_CODES contains exactly the WPF-listed pair', () => {
    expect(UNCONNECTED_GAME_CODES.has('610153_TN')).toBe(true)
    expect(UNCONNECTED_GAME_CODES.has('610085_TC')).toBe(true)
    expect(UNCONNECTED_GAME_CODES.has('610074_T9')).toBe(false)
    expect(UNCONNECTED_GAME_CODES.size).toBe(2)
  })
})

describe('useGameStore — load lifecycle', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    vi.spyOn(console, 'error').mockImplementation(() => {})
    vi.mocked(commands.listGames).mockReset()
  })

  it('starts in idle with empty data', () => {
    const store = useGameStore()
    expect(store.loadState).toBe('idle')
    expect(store.loadError).toBeNull()
    expect(store.services).toEqual([])
    expect(store.ini).toEqual({})
    expect(store.selectedGameCode).toBeNull()
  })

  it('loadGames populates ini + services and flips state to loaded', async () => {
    vi.mocked(commands.listGames).mockReturnValueOnce(ok(BUNDLE))
    const store = useGameStore()
    await store.loadGames()
    expect(store.loadState).toBe('loaded')
    expect(store.services).toEqual(BUNDLE.services)
    expect(store.ini).toEqual(BUNDLE.ini)
  })

  it('loadGames is idempotent — second call without force does not re-fetch', async () => {
    vi.mocked(commands.listGames).mockReturnValueOnce(ok(BUNDLE))
    const store = useGameStore()
    await store.loadGames()
    await store.loadGames()
    expect(commands.listGames).toHaveBeenCalledTimes(1)
  })

  it('loadGames(force=true) re-fetches even when already loaded', async () => {
    vi.mocked(commands.listGames)
      .mockReturnValueOnce(ok(BUNDLE))
      .mockReturnValueOnce(ok({ ...BUNDLE, services: [MAPLE] }))
    const store = useGameStore()
    await store.loadGames()
    await store.loadGames(true)
    expect(commands.listGames).toHaveBeenCalledTimes(2)
    expect(store.services).toEqual([MAPLE])
  })

  it('loadGames sets error state and surfaces the message on failure', async () => {
    vi.mocked(commands.listGames).mockReturnValueOnce(err(SAMPLE_ERROR))
    const store = useGameStore()
    await store.loadGames()
    expect(store.loadState).toBe('error')
    expect(store.loadError).toBe(SAMPLE_ERROR.message)
    expect(store.services).toEqual([])
  })

  it('loadGames does not throw on backend error (caller inspects loadState)', async () => {
    vi.mocked(commands.listGames).mockReturnValueOnce(err(SAMPLE_ERROR))
    const store = useGameStore()
    await expect(store.loadGames()).resolves.toBeUndefined()
  })

  it('clearGameData wipes everything back to idle', async () => {
    vi.mocked(commands.listGames).mockReturnValueOnce(ok(BUNDLE))
    const store = useGameStore()
    await store.loadGames()
    store.selectGame('610074', 'T9')
    expect(store.selectedGameCode).toBe('610074_T9')

    store.clearGameData()

    expect(store.loadState).toBe('idle')
    expect(store.loadError).toBeNull()
    expect(store.services).toEqual([])
    expect(store.ini).toEqual({})
    expect(store.selectedGameCode).toBeNull()
  })

  it('clearGameData allows the next loadGames to re-fetch (idle short-circuit reset)', async () => {
    vi.mocked(commands.listGames).mockReturnValue(ok(BUNDLE))
    const store = useGameStore()
    await store.loadGames()
    store.clearGameData()
    await store.loadGames()
    expect(commands.listGames).toHaveBeenCalledTimes(2)
  })
})

describe('useGameStore — selection + computed projections', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    vi.spyOn(console, 'error').mockImplementation(() => {})
    vi.mocked(commands.listGames).mockReset()
  })

  async function loadedStore() {
    vi.mocked(commands.listGames).mockReturnValueOnce(ok(BUNDLE))
    const store = useGameStore()
    await store.loadGames()
    return store
  }

  it('selectGame writes the joined gameCode', async () => {
    const store = await loadedStore()
    store.selectGame('610074', 'T9')
    expect(store.selectedGameCode).toBe('610074_T9')
  })

  it('selectedGame resolves the active GameService row', async () => {
    const store = await loadedStore()
    store.selectGame('610074', 'T9')
    expect(store.selectedGame).toEqual(MAPLE)
  })

  it('selectedGame returns null when nothing is selected', async () => {
    const store = await loadedStore()
    expect(store.selectedGame).toBeNull()
  })

  it('selectedGame returns null when selection is not in catalogue', async () => {
    const store = await loadedStore()
    store.selectGame('999999', 'XX')
    expect(store.selectedGame).toBeNull()
  })

  it('selectedIni resolves the active INI entry', async () => {
    const store = await loadedStore()
    store.selectGame('610074', 'T9')
    expect(store.selectedIni).toEqual(MAPLE_INI)
  })

  it('selectedIni returns null when the active selection has no INI section', async () => {
    const store = await loadedStore()
    store.selectGame('610153', 'TN')
    expect(store.selectedIni).toBeNull()
  })

  it('isUnconnectedGame is true when active selection is in the WPF list', async () => {
    const store = await loadedStore()
    store.selectGame('610153', 'TN')
    expect(store.isUnconnectedGame).toBe(true)
  })

  it('isUnconnectedGame is false for regular games', async () => {
    const store = await loadedStore()
    store.selectGame('610074', 'T9')
    expect(store.isUnconnectedGame).toBe(false)
  })

  it('isUnconnectedGame is false when nothing is selected', async () => {
    const store = await loadedStore()
    expect(store.isUnconnectedGame).toBe(false)
  })
})
