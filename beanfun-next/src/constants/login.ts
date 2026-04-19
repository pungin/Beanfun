/**
 * Login method enum + companion type — TypeScript-side mirror of the
 * WPF `enum LoginMethod` (`Beanfun/MainWindow.xaml.cs` L1310-1330,
 * declaration around the regional `LoginMethod { Regular = 0, QRCode
 * = 1, GamePass = 2 }` block) and the backend's
 * `Account.method: u8` field (`src-tauri/src/services/storage/users_dat.rs`
 * `Account` struct, exposed across the IPC boundary as a `number`).
 *
 * # Why a separate constants module
 *
 * The integer values are referenced from at least four frontend
 * sites in P12.2 D2 alone — `account.saveLoginCredentials` (skip
 * GamePass / QR), `IdPassForm.vue` (Regular submit path), `LoginTotp`
 * (still Regular per WPF semantics — see below), `VerifyPage.vue`
 * (Regular only). Inlining `0` / `1` / `2` literals in each call
 * site would re-introduce the magic-number footgun that the WPF
 * code itself avoided by declaring the enum. A single typed source
 * keeps every consumer in sync with the backend wire format and
 * makes a future enum extension (e.g. WPF adding a 4th method) a
 * one-line change.
 *
 * # Why TOTP is *not* a separate enum value
 *
 * WPF treats TOTP as a sub-flow of `Regular`, not a distinct
 * method:
 *
 * - `MainWindow.xaml.cs` L1308-1314: `OnLoginCompleted` — the same
 *   single hook fires for both regular-only login and the
 *   regular-then-TOTP path, and it writes `App.LoginMethod`
 *   (already `Regular`) into the saved record without overriding
 *   it.
 * - `totpWorker_RunWorkerCompleted` (L1573-1610) → on success calls
 *   the same `OnLoginCompleted` → `SaveLoginCredentials` chain,
 *   which sets `methodList[i] = App.LoginMethod` (still `Regular`).
 *
 * Net effect: a stored record's `method` field is always one of
 * `Regular` / `QRCode` / `GamePass`; TOTP-pass status lives only
 * in the *transient* in-flight auth flow (handled by `useAuthStore`
 * pending flags), not in `Users.dat`. The `LoginMethod` typed
 * union below intentionally mirrors that — there is no `Totp`
 * variant.
 *
 * # Wire-shape contract
 *
 * Backend `Account.method: u8` (Rust) ↔ frontend `number` (IPC) ↔
 * one of `LOGIN_METHOD.Regular | QrCode | GamePass`. The
 * `LoginMethod` type below is a structural numeric subtype that
 * `account.saveLoginCredentials` callers can pass directly to the
 * IPC boundary without runtime conversion.
 */

/**
 * Numeric values mirror WPF `enum LoginMethod` (`Beanfun/MainWindow.xaml.cs`).
 *
 * - `Regular = 0` — account + password (with optional TOTP / verify
 *   sub-flows, both still serialize as `Regular`).
 * - `QrCode = 1` — `LoginQrStart` / `LoginQrCheck` flow.
 * - `GamePass = 2` — `LoginGamepassStart` flow.
 */
export const LOGIN_METHOD = {
  Regular: 0,
  QrCode: 1,
  GamePass: 2,
} as const

/**
 * Numeric union of every supported login method. Declared as a
 * value-of-typeof so adding a new entry to `LOGIN_METHOD` extends
 * the type automatically.
 */
export type LoginMethod = (typeof LOGIN_METHOD)[keyof typeof LOGIN_METHOD]
