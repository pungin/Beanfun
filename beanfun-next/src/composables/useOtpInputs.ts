/**
 * Reusable N-cell OTP / numeric-code input composable.
 *
 * The WPF `LoginTotp.xaml` renders 6 individual single-character
 * `TextBox`es with `PreviewKeyUp` handlers that forward focus to
 * the next box and trigger submit when the final cell fills. That
 * shape is the canonical Beanfun UX users are trained on. We
 * preserve it but lift the input behaviour into this composable so
 * the page is a pure presentational shell (SRP) and a future
 * Captcha / other multi-cell code input (P12.1 D8 verify flow can
 * drop into this) reuses the same logic without re-implementing
 * focus management + paste + digit filtering (DRY).
 *
 * # Why a composable (not an Element Plus wrapper component)
 *
 * Element Plus does not ship an OTP-style multi-cell input. Going
 * the other direction — bundling the cells into a single custom
 * component — would force callers through a narrow `<OtpInputs />`
 * API that can't be styled per-page without CSS `:deep()` escapes.
 * Keeping the composable presentational-agnostic means each page
 * renders whatever markup fits its glassmorphism / spacing design
 * while sharing the focus + paste + filter logic.
 *
 * # Behaviour summary
 *
 * | Event                   | Behaviour                                  |
 * |-------------------------|--------------------------------------------|
 * | Input digit in cell i   | `cells[i] = digit`; focus cell i+1         |
 * | Input non-digit in i    | Filtered out; cell unchanged               |
 * | Input empties cell i    | `cells[i] = ''`; focus stays (no prev)     |
 * | Backspace on empty i    | Focus cell i-1 (defensive UX; no WPF parity loss) |
 * | Paste N digits into i   | Spread across i..i+N-1; focus last filled  |
 * | All `length` cells full | `onComplete(code)` fires (WPF parity: auto-submit on last digit) |
 *
 * Non-digit filtering is intentional: Beanfun TOTP is six decimal
 * digits; accepting letters would round-trip to the server as an
 * invalid code. The alternative (keep WPF's `MaxLength=1` permissive
 * behaviour) would surface server-side errors for typos a client-side
 * filter trivially prevents. Per the D6 design doc: this is an
 * additive UX improvement that never rejects a valid WPF flow.
 */

import { computed, ref, type ComputedRef, type Ref } from 'vue'

/**
 * Minimal focusable surface — matches `HTMLElement`, Element Plus's
 * `ElInput` exposed instance, or any test double with a `.focus()`
 * method. Keeping the contract narrow means the composable stays
 * framework-agnostic (Element Plus is a page-level dependency, not
 * a composable-level one).
 */
export interface FocusableInput {
  focus(): void
}

export interface UseOtpInputsOptions {
  /** Number of cells in the code. Beanfun TOTP uses 6. */
  readonly length: number
  /**
   * Called once with the concatenated code the moment the last
   * cell fills. The callback fires on both normal typing (last
   * cell's handleInput) and paste (if the paste itself fills every
   * cell). Does NOT fire on `reset()` or on programmatic cell
   * writes outside this composable's handlers — SRP: the callback
   * models the user's "I finished entering my code" intent, not
   * state change.
   */
  readonly onComplete?: (code: string) => void
}

export interface UseOtpInputs {
  /**
   * Per-cell string buffer. Each slot is either `''` or exactly
   * one digit character — the handlers maintain this invariant.
   * Exposed as a `Ref` so pages can `v-model` each cell through
   * `:model-value="cells[i]"` and listen on input events.
   */
  readonly cells: Ref<string[]>
  /** Concatenated code across all cells (empty cells stay empty). */
  readonly code: ComputedRef<string>
  /** True when every cell holds a digit. */
  readonly isComplete: ComputedRef<boolean>
  /**
   * Register / unregister the focusable input for cell `index`.
   * Pass the `ElInput` instance (or any `{ focus(): void }`) on
   * mount, and `null` on unmount. The composable uses the handle
   * only to forward focus; it never reads state back out.
   */
  register(index: number, el: FocusableInput | null): void
  /**
   * Process a raw input value from cell `index`. Accepts either
   * a bare digit string or anything else — non-digit characters
   * are stripped, and only the first digit is kept (defence in
   * depth against platforms that bypass `maxlength`, e.g.
   * autofill). Advances focus to the next cell on non-empty
   * input, and fires `onComplete` when the final cell fills.
   */
  handleInput(index: number, rawValue: string): void
  /**
   * Intercept keyboard events for cell `index`. Only `Backspace`
   * on an already-empty cell is handled (focus the previous cell);
   * every other key is left to the native input. The composable
   * does not `preventDefault` on digit keys — that's the filter's
   * job in {@link handleInput}.
   */
  handleKeydown(index: number, event: KeyboardEvent): void
  /**
   * Handle a paste event by spreading the clipboard's digits
   * starting at cell `index`. Non-digit characters are stripped,
   * and the spread is capped at `length - index` so a paste in the
   * middle of the grid doesn't overflow. The paste itself is
   * consumed via `preventDefault` so the browser's default "insert
   * full string in one cell" behaviour never runs.
   */
  handlePaste(index: number, event: ClipboardEvent): void
  /** Focus the first cell. Safe to call before any cell registers. */
  focusFirst(): void
  /**
   * Clear every cell and focus the first. Intended for callers
   * that want a clean slate after a failed submit (WPF's
   * `totpWorker_RunWorkerCompleted` re-renders the page with empty
   * `otp{1..6}.Text`; this is the SPA equivalent).
   */
  reset(): void
}

const DIGIT_PATTERN = /\d/g

/**
 * Return the first digit in `raw`, or `''` if none present.
 *
 * Used for per-cell input filtering: user types `a` → ignored;
 * user types `5` → accepted; autofill injects `123` → first `1`
 * accepted, rest dropped (paste handler has its own spread path).
 */
function firstDigit(raw: string): string {
  const match = raw.match(/\d/)
  return match ? match[0] : ''
}

export function useOtpInputs(options: UseOtpInputsOptions): UseOtpInputs {
  const { length, onComplete } = options
  if (!Number.isInteger(length) || length < 1) {
    throw new RangeError(`useOtpInputs: length must be a positive integer, got ${String(length)}`)
  }

  const cells = ref<string[]>(new Array(length).fill(''))
  const refs: Array<FocusableInput | null> = new Array(length).fill(null)

  const code = computed(() => cells.value.join(''))
  const isComplete = computed(() => cells.value.every((cell) => cell !== ''))

  function focus(index: number): void {
    if (index < 0 || index >= length) return
    refs[index]?.focus()
  }

  function register(index: number, el: FocusableInput | null): void {
    if (index < 0 || index >= length) return
    refs[index] = el
  }

  function handleInput(index: number, rawValue: string): void {
    if (index < 0 || index >= length) return
    const digit = firstDigit(rawValue)
    cells.value[index] = digit
    if (digit === '') return
    if (index + 1 < length) {
      focus(index + 1)
    }
    if (isComplete.value) {
      onComplete?.(code.value)
    }
  }

  function handleKeydown(index: number, event: KeyboardEvent): void {
    if (index < 0 || index >= length) return
    if (event.key === 'Backspace' && cells.value[index] === '' && index > 0) {
      event.preventDefault()
      focus(index - 1)
    }
  }

  function handlePaste(index: number, event: ClipboardEvent): void {
    if (index < 0 || index >= length) return
    const text = event.clipboardData?.getData('text') ?? ''
    const digits = text.match(DIGIT_PATTERN)?.join('') ?? ''
    if (digits === '') return

    event.preventDefault()

    const room = length - index
    const slice = digits.slice(0, room)
    for (let i = 0; i < slice.length; i++) {
      cells.value[index + i] = slice[i]
    }

    const lastWritten = index + slice.length - 1
    const next = lastWritten + 1
    focus(next < length ? next : lastWritten)

    if (isComplete.value) {
      onComplete?.(code.value)
    }
  }

  function focusFirst(): void {
    focus(0)
  }

  function reset(): void {
    for (let i = 0; i < length; i++) {
      cells.value[i] = ''
    }
    focusFirst()
  }

  return {
    cells,
    code,
    isComplete,
    register,
    handleInput,
    handleKeydown,
    handlePaste,
    focusFirst,
    reset,
  }
}
