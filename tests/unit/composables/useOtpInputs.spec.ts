import { describe, expect, it, vi } from 'vitest'

import { useOtpInputs } from '../../../src/composables/useOtpInputs'

/**
 * Lightweight test double for any `{ focus(): void }` surface. The
 * composable only calls `focus()`; we spy on invocations so every
 * focus-management contract can be asserted without relying on
 * jsdom's real focus semantics (which differ from real browsers in
 * several edge cases).
 */
function makeFocusable(): { focus: ReturnType<typeof vi.fn>; focusCount(): number } {
  const focus = vi.fn()
  return { focus, focusCount: () => focus.mock.calls.length }
}

function registerAll(
  otp: ReturnType<typeof useOtpInputs>,
  length: number,
): ReturnType<typeof makeFocusable>[] {
  const doubles = Array.from({ length }, () => makeFocusable())
  doubles.forEach((d, i) => otp.register(i, d))
  return doubles
}

function pasteEvent(text: string): ClipboardEvent {
  return {
    clipboardData: {
      getData: () => text,
    },
    preventDefault: vi.fn(),
  } as unknown as ClipboardEvent
}

describe('useOtpInputs — invariants', () => {
  it('rejects a non-positive-integer length', () => {
    expect(() => useOtpInputs({ length: 0 })).toThrow(RangeError)
    expect(() => useOtpInputs({ length: -1 })).toThrow(RangeError)
    expect(() => useOtpInputs({ length: 1.5 })).toThrow(RangeError)
  })

  it('initialises every cell empty and reports isComplete=false / code=""', () => {
    const otp = useOtpInputs({ length: 6 })
    expect(otp.cells.value).toEqual(['', '', '', '', '', ''])
    expect(otp.code.value).toBe('')
    expect(otp.isComplete.value).toBe(false)
  })
})

describe('useOtpInputs — handleInput', () => {
  it('writes a single digit into the target cell and advances focus', () => {
    const otp = useOtpInputs({ length: 6 })
    const inputs = registerAll(otp, 6)
    otp.handleInput(0, '1')
    expect(otp.cells.value[0]).toBe('1')
    expect(inputs[1].focusCount()).toBe(1)
  })

  it('filters non-digit characters so the cell stays empty', () => {
    const otp = useOtpInputs({ length: 6 })
    const inputs = registerAll(otp, 6)
    otp.handleInput(0, 'a')
    expect(otp.cells.value[0]).toBe('')
    expect(inputs[1].focusCount()).toBe(0)
  })

  it('keeps only the first digit if multiple characters arrive (autofill defence)', () => {
    const otp = useOtpInputs({ length: 6 })
    const inputs = registerAll(otp, 6)
    otp.handleInput(0, 'x7y')
    expect(otp.cells.value[0]).toBe('7')
    expect(inputs[1].focusCount()).toBe(1)
  })

  it('empty input (delete) clears the cell without moving focus', () => {
    const otp = useOtpInputs({ length: 6 })
    const inputs = registerAll(otp, 6)
    otp.handleInput(0, '1')
    inputs[1].focus.mockClear()
    otp.handleInput(0, '')
    expect(otp.cells.value[0]).toBe('')
    expect(inputs[1].focusCount()).toBe(0)
  })

  it('does not advance focus past the final cell', () => {
    const otp = useOtpInputs({ length: 2 })
    const inputs = registerAll(otp, 2)
    otp.handleInput(0, '1')
    otp.handleInput(1, '2')
    expect(inputs[1].focusCount()).toBe(1)
    expect(otp.isComplete.value).toBe(true)
  })

  it('fires onComplete exactly once when the final cell fills', () => {
    const onComplete = vi.fn()
    const otp = useOtpInputs({ length: 3, onComplete })
    registerAll(otp, 3)
    otp.handleInput(0, '1')
    otp.handleInput(1, '2')
    expect(onComplete).not.toHaveBeenCalled()
    otp.handleInput(2, '3')
    expect(onComplete).toHaveBeenCalledTimes(1)
    expect(onComplete).toHaveBeenCalledWith('123')
  })

  it('does not fire onComplete while any cell is still empty', () => {
    const onComplete = vi.fn()
    const otp = useOtpInputs({ length: 3, onComplete })
    registerAll(otp, 3)
    otp.handleInput(0, '1')
    otp.handleInput(2, '3')
    expect(onComplete).not.toHaveBeenCalled()
  })
})

describe('useOtpInputs — handleKeydown (Backspace)', () => {
  it('focuses the previous cell when Backspace hits an empty non-first cell', () => {
    const otp = useOtpInputs({ length: 6 })
    const inputs = registerAll(otp, 6)
    const event = { key: 'Backspace', preventDefault: vi.fn() } as unknown as KeyboardEvent
    otp.handleKeydown(3, event)
    expect(event.preventDefault).toHaveBeenCalledTimes(1)
    expect(inputs[2].focusCount()).toBe(1)
  })

  it('does nothing on Backspace in the first cell', () => {
    const otp = useOtpInputs({ length: 6 })
    const inputs = registerAll(otp, 6)
    const event = { key: 'Backspace', preventDefault: vi.fn() } as unknown as KeyboardEvent
    otp.handleKeydown(0, event)
    expect(event.preventDefault).not.toHaveBeenCalled()
    inputs.forEach((d) => expect(d.focusCount()).toBe(0))
  })

  it('does nothing on Backspace in a non-empty cell (let native delete handle it)', () => {
    const otp = useOtpInputs({ length: 6 })
    const inputs = registerAll(otp, 6)
    otp.handleInput(2, '5')
    inputs.forEach((d) => d.focus.mockClear())
    const event = { key: 'Backspace', preventDefault: vi.fn() } as unknown as KeyboardEvent
    otp.handleKeydown(2, event)
    expect(event.preventDefault).not.toHaveBeenCalled()
    inputs.forEach((d) => expect(d.focusCount()).toBe(0))
  })

  it('ignores non-Backspace keys', () => {
    const otp = useOtpInputs({ length: 6 })
    const inputs = registerAll(otp, 6)
    const event = { key: 'ArrowLeft', preventDefault: vi.fn() } as unknown as KeyboardEvent
    otp.handleKeydown(3, event)
    expect(event.preventDefault).not.toHaveBeenCalled()
    inputs.forEach((d) => expect(d.focusCount()).toBe(0))
  })
})

describe('useOtpInputs — handlePaste', () => {
  it('spreads digits from the target index and focuses the cell after the last write', () => {
    const otp = useOtpInputs({ length: 6 })
    const inputs = registerAll(otp, 6)
    const event = pasteEvent('123456')
    otp.handlePaste(0, event)
    expect(otp.cells.value).toEqual(['1', '2', '3', '4', '5', '6'])
    expect(event.preventDefault).toHaveBeenCalledTimes(1)
    expect(inputs[5].focusCount()).toBe(1) // last written (no room for i+1)
  })

  it('strips non-digit characters from the pasted text', () => {
    const otp = useOtpInputs({ length: 6 })
    registerAll(otp, 6)
    otp.handlePaste(0, pasteEvent('12 3-45X6'))
    expect(otp.cells.value).toEqual(['1', '2', '3', '4', '5', '6'])
  })

  it('caps the spread at the remaining room when pasting into the middle', () => {
    const otp = useOtpInputs({ length: 6 })
    registerAll(otp, 6)
    otp.handlePaste(3, pasteEvent('999999999'))
    expect(otp.cells.value).toEqual(['', '', '', '9', '9', '9'])
  })

  it('fires onComplete once when a paste fills every cell', () => {
    const onComplete = vi.fn()
    const otp = useOtpInputs({ length: 4, onComplete })
    registerAll(otp, 4)
    otp.handlePaste(0, pasteEvent('5678'))
    expect(onComplete).toHaveBeenCalledExactlyOnceWith('5678')
  })

  it('is a no-op when the clipboard text has no digits', () => {
    const otp = useOtpInputs({ length: 6 })
    const inputs = registerAll(otp, 6)
    const event = pasteEvent('hello')
    otp.handlePaste(0, event)
    expect(otp.cells.value).toEqual(['', '', '', '', '', ''])
    expect(event.preventDefault).not.toHaveBeenCalled()
    inputs.forEach((d) => expect(d.focusCount()).toBe(0))
  })

  it('focuses the next empty cell after a partial paste that leaves room', () => {
    const otp = useOtpInputs({ length: 6 })
    const inputs = registerAll(otp, 6)
    otp.handlePaste(0, pasteEvent('42'))
    expect(otp.cells.value).toEqual(['4', '2', '', '', '', ''])
    expect(inputs[2].focusCount()).toBe(1)
  })
})

describe('useOtpInputs — focus + reset + register', () => {
  it('focusFirst focuses cell 0', () => {
    const otp = useOtpInputs({ length: 6 })
    const inputs = registerAll(otp, 6)
    otp.focusFirst()
    expect(inputs[0].focusCount()).toBe(1)
  })

  it('reset clears every cell and focuses the first', () => {
    const otp = useOtpInputs({ length: 3 })
    const inputs = registerAll(otp, 3)
    otp.handleInput(0, '1')
    otp.handleInput(1, '2')
    inputs.forEach((d) => d.focus.mockClear())

    otp.reset()

    expect(otp.cells.value).toEqual(['', '', ''])
    expect(otp.isComplete.value).toBe(false)
    expect(inputs[0].focusCount()).toBe(1)
  })

  it('register(null) clears the stored handle so later focus calls are no-ops', () => {
    const otp = useOtpInputs({ length: 2 })
    const [first, second] = registerAll(otp, 2)
    otp.register(1, null)
    otp.handleInput(0, '1')
    expect(first.focusCount()).toBe(0)
    expect(second.focusCount()).toBe(0)
  })

  it('out-of-range index calls are safely ignored', () => {
    const otp = useOtpInputs({ length: 3 })
    registerAll(otp, 3)
    expect(() => otp.handleInput(-1, '1')).not.toThrow()
    expect(() => otp.handleInput(10, '1')).not.toThrow()
    expect(otp.cells.value).toEqual(['', '', ''])
  })
})
