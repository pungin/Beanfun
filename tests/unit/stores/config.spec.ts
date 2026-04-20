import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'

import type { CommandError, Result } from '../../../src/types/bindings'

vi.mock('element-plus', () => ({ ElMessage: { error: vi.fn() } }))

vi.mock('../../../src/types/bindings', () => ({
  commands: {
    getAllConfig: vi.fn(),
    setConfig: vi.fn(),
    getConfigValue: vi.fn(),
  },
}))

import { commands } from '../../../src/types/bindings'
import { useConfigStore } from '../../../src/stores/config'

const mockGetAllConfig = vi.mocked(commands.getAllConfig)
const mockSetConfig = vi.mocked(commands.setConfig)

const ok = <T>(data: T): Promise<Result<T, CommandError>> => Promise.resolve({ status: 'ok', data })

const err = (error: CommandError): Promise<Result<never, CommandError>> =>
  Promise.resolve({ status: 'error', error })

describe('useConfigStore', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    mockGetAllConfig.mockReset()
    mockSetConfig.mockReset()
  })

  it('starts empty and unloaded before loadAll runs', () => {
    const store = useConfigStore()
    expect(store.entries).toEqual({})
    expect(store.loaded).toBe(false)
    expect(store.size).toBe(0)
  })

  it('loadAll populates the cache and flips loaded to true', async () => {
    mockGetAllConfig.mockReturnValueOnce(ok({ ThemeColor: '#FF8201', Locale: 'zh-TW' }))

    const store = useConfigStore()
    await store.loadAll()

    expect(store.loaded).toBe(true)
    expect(store.size).toBe(2)
    expect(store.get('ThemeColor')).toBe('#FF8201')
    expect(store.get('Locale')).toBe('zh-TW')
  })

  it('loadAll filters out non-string values defensively', async () => {
    mockGetAllConfig.mockReturnValueOnce(
      ok({
        Real: 'value',
        Bogus: undefined,
      } as unknown as Partial<Record<string, string>>),
    )

    const store = useConfigStore()
    await store.loadAll()

    expect(store.get('Real')).toBe('value')
    expect(store.get('Bogus')).toBeUndefined()
    expect(store.size).toBe(1)
  })

  it('get returns undefined for missing keys; getOr returns the fallback', () => {
    const store = useConfigStore()
    expect(store.get('Missing')).toBeUndefined()
    expect(store.getOr('Missing', 'default')).toBe('default')
  })

  it('set writes through to the backend and updates the cache', async () => {
    mockSetConfig.mockReturnValueOnce(ok(null))
    const store = useConfigStore()
    await store.set('Locale', 'en-US')
    expect(mockSetConfig).toHaveBeenCalledWith('Locale', 'en-US')
    expect(store.get('Locale')).toBe('en-US')
  })

  it('set(key, null) deletes the cache entry after the IPC succeeds', async () => {
    mockGetAllConfig.mockReturnValueOnce(ok({ ToBeDeleted: 'x' }))
    mockSetConfig.mockReturnValueOnce(ok(null))

    const store = useConfigStore()
    await store.loadAll()
    expect(store.get('ToBeDeleted')).toBe('x')

    await store.set('ToBeDeleted', null)

    expect(mockSetConfig).toHaveBeenCalledWith('ToBeDeleted', null)
    expect(store.get('ToBeDeleted')).toBeUndefined()
    expect('ToBeDeleted' in store.entries).toBe(false)
  })

  it('set updates the in-memory cache even when the backend write fails (read-only file)', async () => {
    mockGetAllConfig.mockReturnValueOnce(ok({ Locale: 'zh-TW' }))
    mockSetConfig.mockReturnValueOnce(
      err({ code: 'config.io_error', message: 'disk full', details: null }),
    )

    const store = useConfigStore()
    await store.loadAll()

    await store.set('Locale', 'en-US')
    expect(store.get('Locale')).toBe('en-US')
  })

  it('loadAll surfaces backend errors via wrapCommand', async () => {
    mockGetAllConfig.mockReturnValueOnce(
      err({ code: 'config.io_error', message: 'corrupt', details: null }),
    )
    const store = useConfigStore()
    await expect(store.loadAll()).rejects.toThrow()
    expect(store.loaded).toBe(false)
  })
})
