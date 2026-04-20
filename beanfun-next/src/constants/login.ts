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

/**
 * Region-aware external URLs surfaced from the login forms.
 *
 * Mirrors the two URL-launching handlers in WPF
 * `Beanfun/Pages/id-pass_form.xaml.cs`:
 *
 * - `RegAcc_Click` (L39-52) — RegisterAccount button (XAML L73)
 *   opens a new in-app browser window (Tauri equivalent =
 *   `commands.openInAppBrowser` via [`useInAppBrowser`]) pointed
 *   at the region-appropriate signup page.
 * - `FindPwd_Click` (L54-66) — ForgotPassword button (XAML L627)
 *   opens the region-appropriate `forgot_pwd.aspx` page in the
 *   same in-app browser window.
 *
 * Both handlers share the exact same TW/HK URL fork shape, so we
 * factor the constants into a single 2D map `kind × region → url`
 * to (a) keep the WPF parity table in one place and (b) let the
 * frontend `IdPassForm.vue::handleExternalUrl` dispatch generically
 * by `kind` (no per-button URL literal in the template).
 *
 * # Why a constant module instead of inlining in IdPassForm.vue
 *
 * Same SRP rationale as the rest of `src/constants/*` — a Beanfun
 * URL move tomorrow is a one-line change here, not a hunt across
 * the component tree. Tests can also iterate the same map to assert
 * region-aware dispatch without duplicating the URL fixture.
 *
 * # Wire-shape contract
 *
 * Both handlers in WPF call `new WebBrowser(url).Show()` — i.e. the
 * URL is consumed by the in-app `WebBrowser` window. The SPA
 * equivalent is `commands.openInAppBrowser` (followup-B B2)
 * which builds a fresh `tauri::WebviewWindow` per call with the
 * logged-in `BeanfunClient` cookies pre-seeded. Both
 * `tw.beanfun.com` + `bfweb.hk.beanfun.com` sit inside the
 * backend `web_browser::ALLOWED_HOSTS` allowlist so the page
 * renders embedded — same UX as WPF — instead of falling back
 * to the system browser. The same composable also services
 * P12.5 KartTools' six convoy/rider URLs.
 *
 * URLs ported verbatim from `id-pass_form.xaml.cs` L42-50 / L57-64.
 * Do **not** simplify the HK signup URL — `bfweb.hk.beanfun.com`
 * is the correct host (NOT `hk.beanfun.com`); they're different
 * Beanfun sub-properties and the wrong one returns a 404 page.
 */
export const LOGIN_EXTERNAL_URLS = {
  register: {
    TW: 'https://tw.beanfun.com/TW/signup/Join_beanfun_signup.aspx?service=999999_T0',
    HK: 'https://bfweb.hk.beanfun.com/beanfun_web_ap/signup/preregistration.aspx?service=999999_T0',
  },
  forgotPwd: {
    TW: 'https://tw.beanfun.com/member/forgot_pwd.aspx',
    HK: 'https://bfweb.hk.beanfun.com/member/forgot_pwd.aspx',
  },
} as const

/**
 * Discriminator for {@link LOGIN_EXTERNAL_URLS}. Kept as a string
 * literal union (not a numeric enum) because the values appear
 * verbatim in `data-test` attributes — string keys eliminate a
 * stringify step in the template binding.
 */
export type LoginExternalUrlKind = keyof typeof LOGIN_EXTERNAL_URLS
