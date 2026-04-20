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

vi.mock('../../../src/composables/useThemeColor', async () => {
  const actual = await vi.importActual<typeof import('../../../src/composables/useThemeColor')>(
    '../../../src/composables/useThemeColor',
  )
  return {
    ...actual,
    setPrimaryColor: vi.fn(),
  }
})

import { commands } from '../../../src/types/bindings'
import { useConfigStore } from '../../../src/stores/config'
import {
  __resetUiAppliersForTesting,
  DEFAULT_LOCALE,
  DEFAULT_LOGIN_METHOD,
  DEFAULT_UPDATE_CHANNEL,
  registerLocaleApplier,
  SUPPORTED_LOCALES,
  UI_CONFIG_KEYS,
  useUiStore,
} from '../../../src/stores/ui'
import { DEFAULT_PRIMARY_COLOR, setPrimaryColor } from '../../../src/composables/useThemeColor'

const mockGetAllConfig = vi.mocked(commands.getAllConfig)
const mockSetConfig = vi.mocked(commands.setConfig)
const mockSetPrimaryColor = vi.mocked(setPrimaryColor)

const ok = <T>(data: T): Promise<Result<T, CommandError>> => Promise.resolve({ status: 'ok', data })

describe('useUiStore', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    mockGetAllConfig.mockReset()
    mockSetConfig.mockReset()
    mockSetPrimaryColor.mockReset()
    __resetUiAppliersForTesting()
    vi.spyOn(console, 'error').mockImplementation(() => {})
  })

  describe('reactive getters with defaults', () => {
    it('returns the documented defaults when Config.xml is empty', () => {
      const ui = useUiStore()
      expect(ui.themeColor).toBe(DEFAULT_PRIMARY_COLOR)
      expect(ui.language).toBe(DEFAULT_LOCALE)
      expect(ui.minimizeToTray).toBe(false)
      expect(ui.disableHwAccel).toBe(false)
      expect(ui.updateChannel).toBe(DEFAULT_UPDATE_CHANNEL)
      // P12.4 D2 — WPF defaults from Settings.xaml.cs ctor.
      expect(ui.autoStartGame).toBe(false)
      expect(ui.askUpdate).toBe(true)
      expect(ui.tradLogin).toBe(true)
      expect(ui.autoKillPatcher).toBe(true)
      expect(ui.skipPlayWnd).toBe(true)
      expect(ui.loginMethod).toBe(DEFAULT_LOGIN_METHOD)
    })

    it('reflects values from config.entries verbatim', async () => {
      mockGetAllConfig.mockReturnValueOnce(
        ok({
          ThemeColor: '#0B6E99',
          Language: 'en-US',
          minimize_to_tray: 'true',
          disableHardwareAcceleration: 'true',
          updateChannel: 'Beta',
          autoStartGame: 'true',
          ask_update: 'false',
          tradLogin: 'false',
          autoKillPatcher: 'false',
          skipPlayWnd: 'false',
          loginMethod: '1',
        }),
      )
      const config = useConfigStore()
      await config.loadAll()

      const ui = useUiStore()
      expect(ui.themeColor).toBe('#0B6E99')
      expect(ui.language).toBe('en-US')
      expect(ui.minimizeToTray).toBe(true)
      expect(ui.disableHwAccel).toBe(true)
      expect(ui.updateChannel).toBe('Beta')
      expect(ui.autoStartGame).toBe(true)
      expect(ui.askUpdate).toBe(false)
      expect(ui.tradLogin).toBe(false)
      expect(ui.autoKillPatcher).toBe(false)
      expect(ui.skipPlayWnd).toBe(false)
      expect(ui.loginMethod).toBe('1')
    })

    it('falls back to defaults for invalid stored values', async () => {
      mockGetAllConfig.mockReturnValueOnce(
        ok({
          Language: 'fr-FR',
          updateChannel: 'Bogus',
          minimize_to_tray: 'maybe',
          autoStartGame: 'maybe',
          ask_update: '',
          loginMethod: '7',
        }),
      )
      const config = useConfigStore()
      await config.loadAll()

      const ui = useUiStore()
      expect(ui.language).toBe(DEFAULT_LOCALE)
      expect(ui.updateChannel).toBe(DEFAULT_UPDATE_CHANNEL)
      expect(ui.minimizeToTray).toBe(false)
      // P12.4 D2 — boolean fallbacks honour WPF default (autoStartGame
      // defaults to `false`; askUpdate defaults to `true`).
      expect(ui.autoStartGame).toBe(false)
      expect(ui.askUpdate).toBe(true)
      // loginMethod accepts only `'0'` / `'1'` literals — anything else
      // falls back to Regular per `isLoginMethod` guard.
      expect(ui.loginMethod).toBe(DEFAULT_LOGIN_METHOD)
    })
  })

  describe('setters', () => {
    beforeEach(() => mockSetConfig.mockResolvedValue({ status: 'ok', data: null }))

    it('setThemeColor writes Config.xml and applies the color', async () => {
      const ui = useUiStore()
      await ui.setThemeColor('#ff8201')
      expect(mockSetConfig).toHaveBeenCalledWith(UI_CONFIG_KEYS.ThemeColor, '#ff8201')
      expect(mockSetPrimaryColor).toHaveBeenCalledWith('#ff8201')
    })

    it('setLanguage writes Config.xml and invokes the registered applier', async () => {
      const applier = vi.fn()
      registerLocaleApplier(applier)
      const ui = useUiStore()
      await ui.setLanguage('en-US')
      expect(mockSetConfig).toHaveBeenCalledWith(UI_CONFIG_KEYS.Language, 'en-US')
      expect(applier).toHaveBeenCalledWith('en-US')
    })

    it('setLanguage writes Config.xml even if no applier is registered', async () => {
      const ui = useUiStore()
      await ui.setLanguage('zh-CN')
      expect(mockSetConfig).toHaveBeenCalledWith(UI_CONFIG_KEYS.Language, 'zh-CN')
    })

    it('boolean setters serialize to "true"/"false" strings', async () => {
      const ui = useUiStore()
      await ui.setMinimizeToTray(true)
      await ui.setDisableHwAccel(false)
      expect(mockSetConfig).toHaveBeenNthCalledWith(1, UI_CONFIG_KEYS.MinimizeToTray, 'true')
      expect(mockSetConfig).toHaveBeenNthCalledWith(
        2,
        UI_CONFIG_KEYS.DisableHardwareAcceleration,
        'false',
      )
    })

    it('setUpdateChannel writes the literal channel value', async () => {
      const ui = useUiStore()
      await ui.setUpdateChannel('Beta')
      expect(mockSetConfig).toHaveBeenCalledWith(UI_CONFIG_KEYS.UpdateChannel, 'Beta')
    })

    // P12.4 D2 — five new boolean setters round-trip through Config.xml.
    it('setAutoStartGame serializes to "true"/"false" string', async () => {
      const ui = useUiStore()
      await ui.setAutoStartGame(true)
      expect(mockSetConfig).toHaveBeenCalledWith(UI_CONFIG_KEYS.AutoStartGame, 'true')
    })

    it('setAskUpdate serializes to "true"/"false" string', async () => {
      const ui = useUiStore()
      await ui.setAskUpdate(false)
      expect(mockSetConfig).toHaveBeenCalledWith(UI_CONFIG_KEYS.AskUpdate, 'false')
    })

    it('setTradLogin serializes to "true"/"false" string', async () => {
      const ui = useUiStore()
      await ui.setTradLogin(false)
      expect(mockSetConfig).toHaveBeenCalledWith(UI_CONFIG_KEYS.TradLogin, 'false')
    })

    it('setAutoKillPatcher serializes to "true"/"false" string', async () => {
      const ui = useUiStore()
      await ui.setAutoKillPatcher(true)
      expect(mockSetConfig).toHaveBeenCalledWith(UI_CONFIG_KEYS.AutoKillPatcher, 'true')
    })

    it('setSkipPlayWnd serializes to "true"/"false" string', async () => {
      const ui = useUiStore()
      await ui.setSkipPlayWnd(false)
      expect(mockSetConfig).toHaveBeenCalledWith(UI_CONFIG_KEYS.SkipPlayWnd, 'false')
    })

    it('setLoginMethod writes the literal "0"/"1" string', async () => {
      const ui = useUiStore()
      await ui.setLoginMethod('1')
      expect(mockSetConfig).toHaveBeenCalledWith(UI_CONFIG_KEYS.LoginMethod, '1')
    })
  })

  describe('applyAll', () => {
    it('applies themeColor + locale on boot when both are valid', async () => {
      mockGetAllConfig.mockReturnValueOnce(ok({ ThemeColor: '#5C8430', Language: 'en-US' }))
      const config = useConfigStore()
      await config.loadAll()

      const applier = vi.fn()
      registerLocaleApplier(applier)
      const ui = useUiStore()
      ui.applyAll()

      expect(mockSetPrimaryColor).toHaveBeenCalledWith('#5C8430')
      expect(applier).toHaveBeenCalledWith('en-US')
    })

    it('falls back to default theme color when setPrimaryColor throws', async () => {
      mockGetAllConfig.mockReturnValueOnce(ok({ ThemeColor: 'not-a-color' }))
      const config = useConfigStore()
      await config.loadAll()

      mockSetPrimaryColor.mockImplementationOnce(() => {
        throw new RangeError('invalid hex')
      })

      const ui = useUiStore()
      ui.applyAll()

      expect(mockSetPrimaryColor).toHaveBeenCalledTimes(2)
      expect(mockSetPrimaryColor).toHaveBeenLastCalledWith(DEFAULT_PRIMARY_COLOR)
    })

    it('does not throw when no locale applier is registered', () => {
      const ui = useUiStore()
      expect(() => ui.applyAll()).not.toThrow()
    })
  })

  describe('SUPPORTED_LOCALES', () => {
    it('exposes the three vue-i18n locale codes', () => {
      expect(SUPPORTED_LOCALES).toEqual(['zh-TW', 'zh-CN', 'en-US'])
    })
  })
})
