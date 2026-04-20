import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import type { CommandError, Result } from '../../../src/types/bindings'
import {
  CommandInvocationError,
  __resetInvokeRegistriesForTesting,
  registerErrorTranslator,
  registerSessionExpiredHandler,
  safeInvoke,
  wrapCommand,
} from '../../../src/services/invoke'

vi.mock('element-plus', () => ({
  ElMessage: {
    error: vi.fn(),
  },
}))

import { ElMessage } from 'element-plus'

const elMessageError = vi.mocked(ElMessage.error)

const ok = <T>(data: T): Promise<Result<T, CommandError>> => Promise.resolve({ status: 'ok', data })

const err = (error: CommandError): Promise<Result<never, CommandError>> =>
  Promise.resolve({ status: 'error', error })

describe('wrapCommand', () => {
  let consoleErrorSpy: ReturnType<typeof vi.spyOn>

  beforeEach(() => {
    __resetInvokeRegistriesForTesting()
    elMessageError.mockClear()
    consoleErrorSpy = vi.spyOn(console, 'error').mockImplementation(() => {})
  })

  afterEach(() => {
    consoleErrorSpy.mockRestore()
  })

  it('returns the unwrapped data on success', async () => {
    const data = await wrapCommand(ok({ greeting: 'hello' }))
    expect(data).toEqual({ greeting: 'hello' })
    expect(elMessageError).not.toHaveBeenCalled()
    expect(consoleErrorSpy).not.toHaveBeenCalled()
  })

  it('throws CommandInvocationError preserving the original CommandError', async () => {
    const cause: CommandError = {
      code: 'beanfun.bad_credentials',
      message: 'wrong account or password',
      details: null,
    }

    await expect(wrapCommand(err(cause))).rejects.toBeInstanceOf(CommandInvocationError)
    await expect(wrapCommand(err(cause))).rejects.toMatchObject({
      cause,
      message: 'wrong account or password',
      name: 'CommandInvocationError',
    })
  })

  it('console.errors code and message only (details omitted to prevent leakage)', async () => {
    const cause: CommandError = {
      code: 'config.io_error',
      message: 'failed to write Config.xml',
      details: { path: 'C:\\Users\\test\\Config.xml', kind: 'permission_denied' },
    }

    await expect(wrapCommand(err(cause))).rejects.toThrow()
    expect(consoleErrorSpy).toHaveBeenCalledWith(
      '[invoke] config.io_error: failed to write Config.xml',
    )
  })

  it('shows ElMessage.error with the translated message via the registered translator', async () => {
    registerErrorTranslator((code, fallback) =>
      code === 'errors.auth.invalid_totp' ? '驗證碼錯誤' : fallback,
    )

    await expect(
      wrapCommand(
        err({
          code: 'auth.invalid_totp',
          message: 'invalid TOTP',
          details: null,
        }),
      ),
    ).rejects.toThrow()

    expect(elMessageError).toHaveBeenCalledTimes(1)
    expect(elMessageError).toHaveBeenCalledWith('驗證碼錯誤')
  })

  it('falls back to the backend message when no translation is registered', async () => {
    await expect(
      wrapCommand(
        err({
          code: 'system.unknown',
          message: 'something went wrong',
          details: null,
        }),
      ),
    ).rejects.toThrow()

    expect(elMessageError).toHaveBeenCalledWith('something went wrong')
  })

  it('suppresses ElMessage.error when silent: true (but still logs and throws)', async () => {
    await expect(
      wrapCommand(
        err({
          code: 'qr.expired',
          message: 'QR challenge expired',
          details: null,
        }),
        { silent: true },
      ),
    ).rejects.toBeInstanceOf(CommandInvocationError)

    expect(elMessageError).not.toHaveBeenCalled()
    expect(consoleErrorSpy).toHaveBeenCalled()
  })

  it('invokes the registered session-expired handler when code is auth.session_required', async () => {
    const handler = vi.fn()
    registerSessionExpiredHandler(handler)

    const cause: CommandError = {
      code: 'auth.session_required',
      message: 'session expired',
      details: null,
    }

    await expect(wrapCommand(err(cause))).rejects.toThrow()
    expect(handler).toHaveBeenCalledTimes(1)
    expect(handler).toHaveBeenCalledWith(cause)
  })

  it('does not invoke the handler for non-session error codes', async () => {
    const handler = vi.fn()
    registerSessionExpiredHandler(handler)

    await expect(
      wrapCommand(err({ code: 'auth.invalid_totp', message: 'nope', details: null })),
    ).rejects.toThrow()
    expect(handler).not.toHaveBeenCalled()
  })

  it('still throws even when the session-expired handler itself throws', async () => {
    registerSessionExpiredHandler(() => {
      throw new Error('router not ready')
    })

    await expect(
      wrapCommand(
        err({
          code: 'auth.session_required',
          message: 'session expired',
          details: null,
        }),
      ),
    ).rejects.toBeInstanceOf(CommandInvocationError)

    // Two console.error calls: one for the original failure, one for the handler exception.
    expect(consoleErrorSpy).toHaveBeenCalledTimes(2)
  })
})

describe('safeInvoke', () => {
  beforeEach(() => {
    __resetInvokeRegistriesForTesting()
    elMessageError.mockClear()
  })

  it('returns { ok: true, data } on success', async () => {
    const result = await safeInvoke(ok(42))
    expect(result).toEqual({ ok: true, data: 42 })
  })

  it('returns { ok: false, error } on failure (no throw)', async () => {
    const cause: CommandError = {
      code: 'qr.pending',
      message: 'still waiting',
      details: null,
    }
    const result = await safeInvoke(err(cause))
    expect(result).toEqual({ ok: false, error: cause })
  })

  it('does not invoke ElMessage on failure (escape hatch behavior)', async () => {
    await safeInvoke(err({ code: 'system.unknown', message: 'boom', details: null }))
    expect(elMessageError).not.toHaveBeenCalled()
  })
})
