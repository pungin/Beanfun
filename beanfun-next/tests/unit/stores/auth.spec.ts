import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'

import type {
  CommandError,
  Result,
  SessionInfo,
  QrStart,
  QrStatus,
  VerifySubmit,
} from '../../../src/types/bindings'

vi.mock('element-plus', () => ({ ElMessage: { error: vi.fn() } }))

vi.mock('../../../src/types/bindings', () => ({
  commands: {
    loginRegular: vi.fn(),
    loginTotp: vi.fn(),
    loginQrStart: vi.fn(),
    loginQrCheck: vi.fn(),
    loginGamepassStart: vi.fn(),
    getVerifyPageInfo: vi.fn(),
    getVerifyCaptcha: vi.fn(),
    submitVerify: vi.fn(),
    logout: vi.fn(),
  },
}))

import { commands } from '../../../src/types/bindings'
import {
  CommandInvocationError,
  __resetInvokeRegistriesForTesting,
} from '../../../src/services/invoke'
import { AUTH_ACTIONS, useAuthStore } from '../../../src/stores/auth'

const mockLoginRegular = vi.mocked(commands.loginRegular)
const mockLoginTotp = vi.mocked(commands.loginTotp)
const mockLoginQrStart = vi.mocked(commands.loginQrStart)
const mockLoginQrCheck = vi.mocked(commands.loginQrCheck)
const mockLoginGamepassStart = vi.mocked(commands.loginGamepassStart)
const mockSubmitVerify = vi.mocked(commands.submitVerify)
const mockLogout = vi.mocked(commands.logout)

const ok = <T>(data: T): Promise<Result<T, CommandError>> => Promise.resolve({ status: 'ok', data })

const err = (error: CommandError): Promise<Result<never, CommandError>> =>
  Promise.resolve({ status: 'error', error })

const SESSION: SessionInfo = {
  region: 'TW',
  account_id: 'alice',
  service_code: '610074',
  service_region: 'T9',
}

describe('useAuthStore', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    __resetInvokeRegistriesForTesting()
    vi.spyOn(console, 'error').mockImplementation(() => {})
    for (const fn of Object.values(commands) as ReturnType<typeof vi.fn>[]) fn.mockReset()
  })

  describe('initial state', () => {
    it('starts logged out with no pending flow', () => {
      const auth = useAuthStore()
      expect(auth.session).toBeNull()
      expect(auth.isLoggedIn).toBe(false)
      expect(auth.pendingTotp).toBe(false)
      expect(auth.pendingVerify).toBe(false)
      expect(auth.qrChallenge).toBeNull()
      expect(auth.advanceCheckUrl).toBeNull()
      expect(auth.pendingAction).toBeNull()
    })
  })

  describe('loginRegular', () => {
    it('returns the session and flips isLoggedIn on success', async () => {
      mockLoginRegular.mockReturnValueOnce(ok(SESSION))
      const auth = useAuthStore()
      const result = await auth.loginRegular('TW', 'alice', 'pw')
      expect(result).toEqual(SESSION)
      expect(auth.session).toEqual(SESSION)
      expect(auth.isLoggedIn).toBe(true)
      expect(auth.pendingAction).toBeNull()
    })

    it('returns null and sets pendingTotp on auth.totp_required', async () => {
      mockLoginRegular.mockReturnValueOnce(
        err({ code: 'auth.totp_required', message: 'need totp', details: null }),
      )
      const auth = useAuthStore()
      const result = await auth.loginRegular('TW', 'alice', 'pw')
      expect(result).toBeNull()
      expect(auth.pendingTotp).toBe(true)
      expect(auth.pendingVerify).toBe(false)
      expect(auth.session).toBeNull()
    })

    it('returns null + sets pendingVerify + carries advanceCheckUrl on auth.advance_check_required (TW)', async () => {
      mockLoginRegular.mockReturnValueOnce(
        err({
          code: 'auth.advance_check_required',
          message: 'need verify',
          details: { url: 'https://tw.newlogin.beanfun.com/LoginCheck/AdvanceCheck.aspx?SN=abc' },
        }),
      )
      const auth = useAuthStore()
      const result = await auth.loginRegular('TW', 'alice', 'pw')
      expect(result).toBeNull()
      expect(auth.pendingVerify).toBe(true)
      expect(auth.pendingTotp).toBe(false)
      expect(auth.advanceCheckUrl).toBe(
        'https://tw.newlogin.beanfun.com/LoginCheck/AdvanceCheck.aspx?SN=abc',
      )
    })

    it('falls back to null advanceCheckUrl when backend omits the URL (HK paths)', async () => {
      mockLoginRegular.mockReturnValueOnce(
        err({
          code: 'auth.advance_check_required',
          message: 'need verify',
          details: { url: null },
        }),
      )
      const auth = useAuthStore()
      await auth.loginRegular('HK', 'bob', 'pw')
      expect(auth.pendingVerify).toBe(true)
      expect(auth.advanceCheckUrl).toBeNull()
    })

    it('handles malformed details payload by defaulting advanceCheckUrl to null', async () => {
      mockLoginRegular.mockReturnValueOnce(
        err({
          code: 'auth.advance_check_required',
          message: 'need verify',
          details: { url: 12345 },
        }),
      )
      const auth = useAuthStore()
      await auth.loginRegular('TW', 'alice', 'pw')
      expect(auth.pendingVerify).toBe(true)
      expect(auth.advanceCheckUrl).toBeNull()
    })

    it('clears stale advanceCheckUrl when login then succeeds on retry', async () => {
      const auth = useAuthStore()
      auth.advanceCheckUrl = 'https://stale.example/AdvanceCheck.aspx'
      mockLoginRegular.mockReturnValueOnce(ok(SESSION))
      await auth.loginRegular('TW', 'alice', 'pw')
      expect(auth.advanceCheckUrl).toBeNull()
    })

    it('throws CommandInvocationError for other error codes (and toasts)', async () => {
      mockLoginRegular.mockReturnValueOnce(
        err({ code: 'beanfun.bad_credentials', message: 'wrong', details: null }),
      )
      const auth = useAuthStore()
      await expect(auth.loginRegular('TW', 'alice', 'pw')).rejects.toBeInstanceOf(
        CommandInvocationError,
      )
      expect(auth.pendingTotp).toBe(false)
      expect(auth.pendingVerify).toBe(false)
    })
  })

  describe('loginTotp', () => {
    it('clears pending flags on success', async () => {
      const auth = useAuthStore()
      auth.pendingTotp = true
      mockLoginTotp.mockReturnValueOnce(ok(SESSION))
      const result = await auth.loginTotp('123456')
      expect(result).toEqual(SESSION)
      expect(auth.pendingTotp).toBe(false)
      expect(auth.pendingVerify).toBe(false)
      expect(auth.session).toEqual(SESSION)
    })

    it('chains TOTP → verify + carries advanceCheckUrl when advance_check_required follows TOTP', async () => {
      const auth = useAuthStore()
      auth.pendingTotp = true
      mockLoginTotp.mockReturnValueOnce(
        err({
          code: 'auth.advance_check_required',
          message: 'need verify',
          details: { url: 'https://tw.newlogin.beanfun.com/LoginCheck/AdvanceCheck.aspx?SN=xyz' },
        }),
      )
      const result = await auth.loginTotp('123456')
      expect(result).toBeNull()
      expect(auth.pendingTotp).toBe(false)
      expect(auth.pendingVerify).toBe(true)
      expect(auth.advanceCheckUrl).toBe(
        'https://tw.newlogin.beanfun.com/LoginCheck/AdvanceCheck.aspx?SN=xyz',
      )
    })
  })

  describe('loginQrStart / loginQrCheck', () => {
    it('stores the QR challenge on start and clears on approval', async () => {
      const challenge: QrStart = {
        bitmap_base64: 'data:image/png;base64,xx',
        deeplink: 'beanfun://qr',
      }
      mockLoginQrStart.mockReturnValueOnce(ok(challenge))
      const auth = useAuthStore()
      const result = await auth.loginQrStart('TW')
      expect(result).toEqual(challenge)
      expect(auth.qrChallenge).toEqual(challenge)

      const approved: QrStatus = { status: 'approved', session: SESSION }
      mockLoginQrCheck.mockReturnValueOnce(ok(approved))
      const status = await auth.loginQrCheck()
      expect(status).toEqual({ ok: true, data: approved })
      expect(auth.session).toEqual(SESSION)
      expect(auth.qrChallenge).toBeNull()
    })

    it('clears qrChallenge on expiry but leaves session untouched', async () => {
      const auth = useAuthStore()
      auth.qrChallenge = { bitmap_base64: 'x', deeplink: null }
      const expired: QrStatus = { status: 'expired' }
      mockLoginQrCheck.mockReturnValueOnce(ok(expired))
      const status = await auth.loginQrCheck()
      expect(status).toEqual({ ok: true, data: expired })
      expect(auth.qrChallenge).toBeNull()
      expect(auth.session).toBeNull()
    })

    it('keeps qrChallenge intact on pending status', async () => {
      const auth = useAuthStore()
      const ch: QrStart = { bitmap_base64: 'x', deeplink: null }
      auth.qrChallenge = ch
      mockLoginQrCheck.mockReturnValueOnce(ok({ status: 'pending' }))
      await auth.loginQrCheck()
      expect(auth.qrChallenge).toEqual(ch)
    })

    it('returns the error result without toasting on backend failure', async () => {
      const auth = useAuthStore()
      const ch: QrStart = { bitmap_base64: 'x', deeplink: null }
      auth.qrChallenge = ch
      const failure = {
        code: 'beanfun.qr_json_parse_failed',
        message: 'JSON parse failed',
        details: null,
      }
      const { ElMessage } = await import('element-plus')
      vi.mocked(ElMessage.error).mockClear()
      mockLoginQrCheck.mockReturnValueOnce(err(failure))
      const status = await auth.loginQrCheck()
      expect(status).toEqual({ ok: false, error: failure })
      expect(auth.qrChallenge).toEqual(ch)
      expect(ElMessage.error).not.toHaveBeenCalled()
    })
  })

  describe('loginGamepassStart', () => {
    it('resolves to void on backend ok (no session yet — skey stashed server-side)', async () => {
      mockLoginGamepassStart.mockReturnValueOnce(ok(null))
      const auth = useAuthStore()
      const result = await auth.loginGamepassStart('TW')
      expect(result).toBeUndefined()
      expect(auth.session).toBeNull()
      expect(auth.isLoggedIn).toBe(false)
      expect(auth.pendingAction).toBeNull()
    })

    it('throws CommandInvocationError on backend refusal (e.g. HK region guard)', async () => {
      mockLoginGamepassStart.mockReturnValueOnce(
        err({
          code: 'auth.gamepass_unsupported_region',
          message: 'HK not supported',
          details: null,
        }),
      )
      const auth = useAuthStore()
      await expect(auth.loginGamepassStart('HK')).rejects.toBeInstanceOf(CommandInvocationError)
      expect(auth.pendingAction).toBeNull()
    })

    it('flags pendingAction during the in-flight call', async () => {
      let resolve!: (r: Result<null, CommandError>) => void
      mockLoginGamepassStart.mockReturnValueOnce(
        new Promise<Result<null, CommandError>>((res) => {
          resolve = res
        }),
      )
      const auth = useAuthStore()
      const inFlight = auth.loginGamepassStart('TW')
      expect(auth.pendingAction).toBe(AUTH_ACTIONS.LoginGamepassStart)
      resolve({ status: 'ok', data: null })
      await inFlight
      expect(auth.pendingAction).toBeNull()
    })
  })

  describe('submitVerify', () => {
    it('clears pendingVerify + advanceCheckUrl only when result is success', async () => {
      const auth = useAuthStore()
      auth.pendingVerify = true
      auth.advanceCheckUrl = 'https://tw.newlogin.beanfun.com/LoginCheck/AdvanceCheck.aspx?SN=ok'
      const success: VerifySubmit = { result: 'success' }
      mockSubmitVerify.mockReturnValueOnce(ok(success))
      const r = await auth.submitVerify('1234', 'CAP1')
      expect(r).toEqual(success)
      expect(auth.pendingVerify).toBe(false)
      expect(auth.advanceCheckUrl).toBeNull()
    })

    it('keeps pendingVerify + advanceCheckUrl on wrong_captcha', async () => {
      const auth = useAuthStore()
      auth.pendingVerify = true
      auth.advanceCheckUrl = 'https://tw.newlogin.beanfun.com/LoginCheck/AdvanceCheck.aspx?SN=keep'
      const wrong: VerifySubmit = { result: 'wrong_captcha' }
      mockSubmitVerify.mockReturnValueOnce(ok(wrong))
      const r = await auth.submitVerify('1234', 'CAPX')
      expect(r).toEqual(wrong)
      expect(auth.pendingVerify).toBe(true)
      expect(auth.advanceCheckUrl).toBe(
        'https://tw.newlogin.beanfun.com/LoginCheck/AdvanceCheck.aspx?SN=keep',
      )
    })
  })

  describe('clearSession', () => {
    it('wipes session, pendingTotp, pendingVerify, qrChallenge and advanceCheckUrl synchronously', () => {
      const auth = useAuthStore()
      auth.session = SESSION
      auth.pendingTotp = true
      auth.pendingVerify = true
      auth.qrChallenge = { bitmap_base64: 'x', deeplink: null }
      auth.advanceCheckUrl = 'https://tw.newlogin.beanfun.com/LoginCheck/AdvanceCheck.aspx?SN=keep'

      auth.clearSession()

      expect(auth.session).toBeNull()
      expect(auth.isLoggedIn).toBe(false)
      expect(auth.pendingTotp).toBe(false)
      expect(auth.pendingVerify).toBe(false)
      expect(auth.qrChallenge).toBeNull()
      expect(auth.advanceCheckUrl).toBeNull()
    })

    it('does not invoke the backend logout command (D10 session-expired path skips IPC)', () => {
      const auth = useAuthStore()
      auth.session = SESSION
      auth.clearSession()
      expect(mockLogout).not.toHaveBeenCalled()
    })

    it('does not occupy the pendingAction slot (callable while another action is in flight)', async () => {
      let resolveFirst!: (r: Result<SessionInfo, CommandError>) => void
      mockLoginRegular.mockReturnValueOnce(
        new Promise<Result<SessionInfo, CommandError>>((resolve) => {
          resolveFirst = resolve
        }),
      )

      const auth = useAuthStore()
      const inFlight = auth.loginRegular('TW', 'a', 'p')
      expect(auth.pendingAction).toBe(AUTH_ACTIONS.LoginRegular)

      // session-expired bridge can fire mid-flight without deadlocking
      expect(() => auth.clearSession()).not.toThrow()

      resolveFirst({
        status: 'error',
        error: { code: 'beanfun.transport', message: 'x', details: null },
      })
      await expect(inFlight).rejects.toThrow()
    })
  })

  describe('logout', () => {
    it('calls backend logout then clears all session state via clearSession', async () => {
      const auth = useAuthStore()
      auth.session = SESSION
      auth.pendingTotp = true
      auth.pendingVerify = true
      auth.qrChallenge = { bitmap_base64: 'x', deeplink: null }
      auth.advanceCheckUrl = 'https://tw.newlogin.beanfun.com/LoginCheck/AdvanceCheck.aspx?SN=z'
      mockLogout.mockReturnValueOnce(ok(null))
      await auth.logout()
      expect(mockLogout).toHaveBeenCalledTimes(1)
      expect(auth.session).toBeNull()
      expect(auth.pendingTotp).toBe(false)
      expect(auth.pendingVerify).toBe(false)
      expect(auth.qrChallenge).toBeNull()
      expect(auth.advanceCheckUrl).toBeNull()
    })
  })

  describe('withGuard', () => {
    it('rejects concurrent actions while one is in flight', async () => {
      let resolveFirst!: (r: Result<SessionInfo, CommandError>) => void
      mockLoginRegular.mockReturnValueOnce(
        new Promise<Result<SessionInfo, CommandError>>((resolve) => {
          resolveFirst = resolve
        }),
      )

      const auth = useAuthStore()
      const inFlight = auth.loginRegular('TW', 'a', 'p')

      expect(auth.pendingAction).toBe(AUTH_ACTIONS.LoginRegular)

      await expect(auth.loginRegular('TW', 'a', 'p')).rejects.toThrow(/already in progress/)

      resolveFirst({ status: 'ok', data: SESSION })
      await inFlight
      expect(auth.pendingAction).toBeNull()
    })

    it('clears pendingAction even when the action throws', async () => {
      mockLoginRegular.mockReturnValueOnce(
        err({ code: 'beanfun.bad_credentials', message: 'wrong', details: null }),
      )
      const auth = useAuthStore()
      await expect(auth.loginRegular('TW', 'a', 'p')).rejects.toThrow()
      expect(auth.pendingAction).toBeNull()
    })
  })
})
