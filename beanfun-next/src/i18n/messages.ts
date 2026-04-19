/**
 * Frontend-only i18n messages — keys that don't exist in the legacy
 * WPF XAML resource dictionaries.
 *
 * # Why a separate module
 *
 * `src/locales/{zh-TW,zh-CN,en-US}.json` are auto-generated from
 * `Beanfun/Lang/*.xaml` by `scripts/convert-lang.mjs`. Editing those
 * files directly would lose the changes on the next regeneration.
 * This module sits beside them and gets deep-merged at i18n boot
 * (see `src/i18n/index.ts`), so:
 *
 * - WPF translation drift is a one-line `node scripts/convert-lang.mjs`
 *   away (no manual reconciliation).
 * - Frontend additions live in TypeScript with full type-checking
 *   (one missing key in any locale = build error via the
 *   `KeysMatch<T, U>` constraint below).
 *
 * # Namespace conventions
 *
 * - `loginShell.*` — strings rendered by `pages/LoginPage.vue` (the
 *   parent shell that hosts every login sub-route). Per-form copy
 *   (region picker, id-pass, qr, etc.) lives under its own namespace
 *   added by the relevant P12.1 D-step.
 * - `loginRegion.*` — region picker copy in
 *   `pages/LoginRegionSelection.vue`; the region labels themselves
 *   (`Taiwan` / `HongKong`) come from the WPF locale JSON via the
 *   regenerated `Lang.*.xaml` files.
 * - `loginQr.*` — QR login form copy (`pages/QrForm.vue`): scan
 *   prompt + the HK-unsupported redirect toast + the polling-error
 *   inline fallback string. WPF has no direct equivalent: the legacy
 *   UI relied on a hover-tip image (`Resources/QRCode_Tip.png`) for
 *   the "how to scan" affordance and silently killed the poll timer
 *   on errors without surfacing any text. We render the same meaning
 *   as localizable strings because an SPA can't ship a bitmap tip
 *   trivially across locales.
 * - `loginTotp.*` — TOTP challenge form copy (`pages/LoginTotp.vue`):
 *   page-scoped title and subtitle. WPF `LoginTotp.xaml` renders only
 *   the single `InputTotp` label (reused as the subtitle source of
 *   truth in the WPF locale JSON) plus the `Login` / `Cancel` buttons
 *   that every login child already reuses. We add a dedicated
 *   `title` because the WPF shell (`MainWindow`) does not show a page
 *   heading — the surrounding `LoginPage` shell in the SPA has a
 *   generic "請登入" subline that needs an inline TOTP-specific
 *   heading to signal the sub-flow. The `back` affordance reuses the
 *   WPF `Back` key (shared across every login child's top-left link),
 *   and `errors.auth.invalid_totp` already lives under `errors.auth.*`
 *   so no form-scoped key is needed for the error toast.
 * - `loginVerify.*` — AdvanceCheck verify page copy (`pages/VerifyPage.vue`):
 *   page-scoped title, subtitle, and the post-success toast that
 *   informs the user the AdvanceCheck cleared and they should
 *   re-enter credentials (the no-secrets-over-IPC backend policy
 *   means the SPA can't auto-resume login — see `VerifyPage.vue`
 *   docblock for the rationale). All field labels / placeholders /
 *   error toasts (`AuthInfoNeed` / `CaptchaCodeNeed` /
 *   `YourAuthInfoTip` / `MsgAuthInfoEmpty` / `MsgCaptchaCodeEmpty` /
 *   `WrongCaptcha` / `WrongAuthInfo` / `LoadCaptchaFailed` /
 *   `RefreshCaptcha` / `AuthConfirm` / `Remember` / `Back`) reuse
 *   WPF locale keys verbatim, so no form-scoped duplicates live
 *   here — only the namespace-level chrome the SPA layout
 *   introduces (heading + post-success affirmation toast that
 *   doesn't exist in WPF because `do_Login` re-runs synchronously
 *   without surfacing a "verify cleared" message).
 * - `loginGamepass.*` — GamePass login form copy (`pages/GamepassForm.vue`):
 *   page-scoped title/subtitle, 4-step progress labels, HK-unsupported
 *   redirect toast, connection-lost fallback, refresh button label,
 *   window-error fallback (CP3 `openGamepassWindow` failure / Tauri
 *   `gamepass-login-failed` event — shown as a fixed UX-level banner
 *   rather than the backend's raw `CommandError.message`, matching the
 *   `connectionLost` convention so the user sees a consistent
 *   "something went wrong; press Refresh" affordance regardless of
 *   the underlying error code).
 *   WPF `gamepass_form.xaml` is a bare "Open GamePass / Cancel"
 *   dialog — we enhance with a step tracker so the user can tell
 *   which phase of the WebView-driven flow is stuck. `GamePassLogin`
 *   / `GamePassWaiting` / `GamePassOpen` stay as WPF-owned keys
 *   (used by `IdPassForm` as the switch-link label); the form-scoped
 *   strings here are the frontend-only additions.
 *
 *   Cancellation (user closes the WebView window) is intentionally
 *   silent — matches WPF `GamePassBrowser` which emits no dialog when
 *   the user hits the close button — so no `cancelled` key lives here.
 * - `addAccountDialog.*` / `changeAccountDialog.*` — Stored Beanfun
 *   credential CRUD dialogs (`windows/AddAccount.vue` /
 *   `windows/ChangeAccount.vue`, P12.2 D8). WPF resx never declared
 *   keys for the field labels (`Region` / `AccountID` / `AccountName`
 *   / `Verify` / `Save` / `ChangeAccount` / `AlreadyExists`) — the
 *   WPF dialog used `tbBeanfun*` placeholders inside the input boxes
 *   and a `ComboBox` showing only `{Taiwan} / {HongKong}` items, so
 *   external "Region" / "Account ID" labels never existed. We add
 *   explicit labels here for accessibility (screen readers / form
 *   semantics) without shadowing any WPF key. The
 *   `addAccountDialog.duplicateExists` toast is the UX-improvement
 *   guard wired in D8 Q8 = B (block over-write instead of WPF's
 *   silent upsert) — see `windows/AddAccount.vue` docblock.
 *   `changeAccountDialog.accountIdReadonlyHint` documents the
 *   "delete + re-add to change ID" workaround the SPA picked over
 *   WPF's "remove-then-add at index" sequence (D8 Q5 = mockup
 *   parity, see `windows/ChangeAccount.vue` docblock for the
 *   detailed rationale).
 * - `gameList.*` — Game-picker dialog (`windows/GameList.vue`,
 *   P12.3 D5). The dialog title reuses the WPF `GameSelected`
 *   resource verbatim (parity with the legacy `Window.Title`
 *   binding on `GameList.xaml` L9). The 4-state load machine
 *   (loading / error / empty / loaded) is a frontend addition —
 *   WPF assumed `GameList[region]` was already populated by the
 *   prior `reLoadGameInfo()` call when the dialog opened, so the
 *   user never saw a loading or error state inside the dialog
 *   itself; if the prior fetch had failed they got the empty
 *   `WrapPanel` with no recovery affordance. The SPA surfaces
 *   each state explicitly so the user can retry without
 *   navigating back to the parent shell. The `subtitle` /
 *   `imageAlt` strings are accessibility-only additions (mockup
 *   chrome + `<img alt>`) and have no WPF equivalent. Action
 *   strings (`Retry`) reuse the existing `accountList.retry` key
 *   to stay phrasing-aligned with the AccountList load-failure
 *   banner.
 * - `unconnectedGameAddAccount.*` — Unconnected-game add-account
 *   dialog (`windows/UnconnectedGame_AddAccount.vue`, P12.3 D6).
 *   Frontend-only addition: a brief loading placeholder shown
 *   while [`commands.unconnectedGameInitAddAccountPayload`] is in
 *   flight on first open. WPF blocked the constructor on the same
 *   call (no UI rendered until the payload returned), and on
 *   failure showed `UnknownError` MessageBox before the window was
 *   even painted. The SPA renders the dialog chrome immediately
 *   for snappier perceived performance, then swaps in this
 *   placeholder until the IPC resolves. Every other localized
 *   string (game name interpolations, validation messages,
 *   hyperlink labels, ToS phrasing) reuses the existing WPF
 *   `UnconnectedGame_AddAccount_{1..27}` resource keys verbatim so
 *   the dialog stays exactly translated to WPF's wording.
 * - `accRecovery.*` — AES backup / restore dialog copy
 *   (`windows/AccRecovery.vue`, P12.2 D10.3). The dialog itself
 *   reuses every WPF resource key it needs (`DataRecovery` /
 *   `Password` / `Data` / `Export` / `Recovery` / `ExportDone` /
 *   `RecoverySuccess` / `RecoveryFailed` / `MsgDecryptFailed`)
 *   verbatim — they're the WPF surface this dialog ports 1:1 from.
 *   The frontend-only key here is the textarea placeholder hint
 *   that WPF's empty `t_Data` `TextBox` lacked: WPF gave the user
 *   no clue what the field was for. The redesign adds a single
 *   placeholder ("Export auto-fills; for Recovery paste your
 *   existing ciphertext") so first-time users know the field is
 *   bidirectional. Behaviour parity preserved — placeholder is
 *   pure visual hint with no functional impact.
 * - `manageAccount.*` — Stored-credential management page copy
 *   (`pages/ManageAccount.vue`, P12.2 D9). The page heading
 *   (`ManageAccount`), per-row action labels (`Edit` / `Delete`),
 *   region chips (`Taiwan` / `HongKong`), the toolbar add button
 *   (`Add`), the destructive-confirm chrome (`Cancel` / `Yes` /
 *   `No` / `DeleteAccount` / `MsgDeleteAccountMng` /
 *   `MsgDeleteAccountSingle`), and the data-backup label
 *   (`DataBackup`) all live in the WPF locale tree and are reused
 *   verbatim — they're shared with the legacy `ManageAccount.xaml`
 *   surface so SPA + WPF stay phrasing-aligned. The frontend-only
 *   keys here cover what WPF never declared:
 *     • Mockup chrome (`subtitle` / `searchPlaceholder` /
 *       `totalAccounts` / column headers / `footerHint`) —
 *       WPF used a bare `ListView` with `GridViewColumn` headers
 *       hard-coded to single words (`Account`, `AccName`, `SavePwd`,
 *       …); the redesign needs full localizable column titles plus
 *       a search bar, stats card, and footer that WPF lacked.
 *     • Toolbar actions (`import` / `export`) — kept distinct from
 *       the WPF `DataBackup` key because that label fronts the
 *       AES-encrypted recovery flow (`Beanfun/Windows/AccRecovery.xaml`,
 *       a P12.2 D10 concern); the D9 plaintext file-picker path is
 *       a separate UX with its own button labels.
 *     • Empty / no-search-result placeholders (`empty` /
 *       `noSearchResult` / `lastLoginUnknown` / `remarkEmpty`) —
 *       WPF rendered the empty list as a literally empty `ListView`
 *       with no surrounding text; the SPA needs explicit copy for
 *       discoverability.
 *     • Drag handle tooltip (`dragDisabledTip`) — the redesign
 *       reserves a drag column visually but D9 does not wire
 *       reorder (the Rust `save_account` command is by-key upsert,
 *       not indexed insertion; reorder lands in a future P12.X
 *       backend D-step). The tooltip explains the disabled state
 *       so users don't think the handle is broken.
 *     • Row-icon tooltips (`editAction` / `copyIdAction` /
 *       `deleteAction`) — accessibility (screen readers) and
 *       hover affordance for the icon-only mockup row controls.
 *     • Copy-to-clipboard toast (`idCopied`) — the mockup adds a
 *       Copy ID button WPF never had; we follow the
 *       `accountList.copyOtp` pattern for parity with how OTP copy
 *       reports success (D5 `clipboardWriteOtp` flow).
 *     • Import overwrite confirm (`importOverwriteConfirm` /
 *       `importOverwriteConfirmTitle`) — the backend's
 *       `commands::import_records` is a full-file overwrite (it
 *       parses the JSON and replaces every entry in `Users.dat`,
 *       not a merge); a destructive-action guard before
 *       overwriting is essential UX. WPF prompts for an AES
 *       password instead, which acts as the implicit confirmation;
 *       D9 needs its own explicit dialog because the file-picker
 *       flow has no equivalent gate.
 *     • Toasts (`importSuccess` / `exportSuccess`) — WPF surfaced
 *       the same outcomes via `MsgRecoveryAccountSuccess` /
 *       `MsgRecoveryAccountFailed` for the AES path; the plaintext
 *       D9 path needs its own copy because "recovery" implies the
 *       AES-encrypted backup semantic that this file picker does
 *       not provide.
 *     • Export filename suggestion (`exportDefaultFilename`) — the
 *       `tauri-plugin-dialog::save({ defaultPath })` API requires a
 *       filename string; WPF's `SaveFileDialog` had a hard-coded
 *       `Beanfun.json` literal in the C# code, which we hoist into
 *       the locale tree so en-US users see an English suggestion.
 * - `accountList.*` — Account list page copy
 *   (`pages/AccountList.vue`): page heading, list-state strings (4
 *   states: loading / empty / load-failed / non-empty), per-row
 *   chrome (status badges, account count), the secondary panel
 *   labels (Gash balance, member-center / customer-service link
 *   labels, auto-paste checkbox, OTP placeholder + copy affordance).
 *   Existing WPF locale keys reused verbatim — never shadowed:
 *   `GameStart` / `Logout` / `LogoutConfirm` / `Cancel` /
 *   `AddServiceAccount` / `GetOtp`. Action-scoped strings used by
 *   future P12.2 D-step handlers (`MsgSelectAccount`,
 *   `GettingOtp`, `GoToVerify`, …) live exclusively in the WPF
 *   locale tree; the per-action D-step that wires the handler
 *   will read them from there directly. The dead-key audit
 *   (`tests/unit/i18n/key-usage.spec.ts`) enforces this — keys
 *   added speculatively for "future" D-steps fail the build, so
 *   each new copy lands together with the call site that uses it.
 * - `errors.*` — keyed by the backend `CommandError.code` value
 *   (`<domain>.<variant_snake_case>`) so `services/invoke.ts` can do
 *   `t('errors.' + error.code, fallback)` and get the localized
 *   message automatically.
 * - `themePreset.*` — the 8 preset color labels for the Settings
 *   page swatch picker; matches `THEME_PRESETS[i].name` in
 *   `composables/useThemeColor.ts`.
 *
 * P12 will extend this as new error codes / pages appear; the
 * `KeysMatch` type guard plus the vitest "all-locales-match" spec
 * ensure no key drifts silently.
 *
 * # Static key-usage audit (D9)
 *
 * `tests/unit/i18n/key-usage.spec.ts` mechanically enforces two
 * additional invariants over this module + the WPF locale JSON:
 *
 * 1. Every literal `t('some.key')` call site under
 *    `src/{pages,composables,components,stores}/` resolves to a
 *    key declared in the canonical zh-TW message tree (catches
 *    typos at `npm run test` time instead of at live boot).
 * 2. Every leaf key declared here is consumed somewhere — either
 *    via a literal `t('...')` call, or via a `DYNAMIC_KEY_CONSUMERS`
 *    entry in that spec (errors.* via the translator pipeline,
 *    themePreset.* via the future Settings swatch list, region tile
 *    hint keys via TILES[i].hintKey). When you delete a banner /
 *    page that owned a leaf, either remove the key here too, or
 *    register the new dynamic consumer in the spec.
 */

/**
 * Compile-time guard: the three locale objects must declare an
 * identical key tree. A missing nested key in any locale becomes a
 * `Type '…' is missing the following properties from type '…'`
 * error during `vue-tsc --noEmit`.
 */
type KeysMatch<T, U> = keyof T extends keyof U ? (keyof U extends keyof T ? unknown : never) : never

/* -------- zh-TW (canonical / authoritative source) -------- */

const zhTW = {
  loginShell: {
    heading: '繽放 Next',
    subline: '歡迎登入',
  },
  loginRegion: {
    subline: '請選擇您要登入的 Beanfun 服務地區。',
    defaultBadge: '預設',
    totpHint: '支援 TOTP',
    tip: '可在「設定 → 一般」隨時切換。此選擇將會記住至下次啟動。',
  },
  loginQr: {
    title: '使用 Beanfun App 掃描',
    subtitle: '開啟 Beanfun 行動版 App 掃描下方 QR Code 即可登入。',
    unsupportedHK: 'QR 登入僅支援台灣區，已返回登入入口。',
    connectionLost: '無法取得登入狀態，請點選重新整理重試。',
  },
  loginTotp: {
    title: '雙重驗證',
    subtitle: '請輸入驗證器應用程式顯示的 6 位數驗證碼。',
  },
  loginVerify: {
    title: '二次驗證',
    subtitle: '為了確保帳號安全，請完成以下驗證後重新登入。',
    success: '二次驗證已完成，請重新輸入帳號密碼登入。',
  },
  loginGamepass: {
    title: '使用 GamePass 登入',
    subtitle: '請於開啟的視窗中完成 GamePass 登入程序。',
    steps: {
      prepare: '準備登入環境',
      openWindow: '開啟登入視窗',
      authenticate: '等待認證',
      complete: '登入完成',
    },
    prepareDone: '登入環境已備妥。',
    unsupportedHK: 'GamePass 登入僅支援台灣區，已返回登入入口。',
    connectionLost: '無法與 Beanfun 連線，請點選重新整理重試。',
    windowError: '無法完成 GamePass 登入，請點選重新整理重試。',
    refresh: '重新整理',
  },
  accountList: {
    title: '帳號清單',
    subtitle: '選擇遊戲帳號以開始遊戲，或新增、管理帳號。',
    serviceAccountsHeading: '遊戲帳號',
    accountCount: '{count} 個帳號',
    loading: '載入中…',
    empty: '目前沒有任何遊戲帳號，點擊下方按鈕新增。',
    loadFailed: '無法載入帳號清單，請檢查網路後重試。',
    retry: '重試',
    statusOnline: '已連線',
    statusBanned: '已停用',
    gashBalance: 'Gash 點數',
    gashBalancePlaceholder: '—',
    refreshBalance: '更新點數',
    memberCenter: '會員中心',
    customerService: '客服中心',
    autoPaste: '自動貼上',
    autoPasteTip:
      '自動輸入需要遊戲在輸入帳密界面\n\n※ 自動輸入功能可能會由於遊戲限制出現偶爾無法正常進行的問題，請斟酌使用。',
    otpHeading: '一次性密碼',
    otpPlaceholder: '尚未取得',
    copyOtp: '複製密碼',
    toolsButton: '工具',
    changeGame: '切換遊戲',
    moreActions: '更多操作',
    dragHandle: '拖曳排序',
    gamePlaceholder: '尚未選擇遊戲',
    gamePathPickerPending: '遊戲路徑設定畫面尚未完成（P12.4 將補上），請稍候再試。',
  },
  addAccountDialog: {
    subtitle: '將 Beanfun 帳號加入本機，下次可直接從清單快速登入。',
    regionLabel: '服務地區',
    accountIdLabel: '帳號',
    accountNameLabel: '備註（選填）',
    passwordLabel: '密碼（選填）',
    verifyLabel: '認證資訊（選填）',
    save: '新增',
    duplicateExists: '此區域下已有相同帳號，請改用「修改帳號」或先刪除舊資料。',
  },
  changeAccountDialog: {
    title: '修改帳號',
    subtitle: '可調整顯示備註與自動登入設定；其他欄位請刪除後重新新增。',
    regionLabel: '服務地區',
    accountIdLabel: '帳號',
    accountNameLabel: '備註',
    accountIdReadonlyHint: '帳號為唯一鍵，無法直接修改；如需更換請先刪除後再新增。',
    save: '儲存',
  },
  gameList: {
    subtitle: '選擇要登入或啟動的 beanfun! 遊戲。',
    loading: '載入遊戲清單中…',
    empty: '此區域目前沒有可用的遊戲。',
    loadFailed: '無法載入遊戲清單，請檢查網路後重試。',
    imageAlt: '{name} 遊戲封面',
  },
  unconnectedGameAddAccount: {
    loading: '載入中，請稍候…',
  },
  accRecovery: {
    dataPlaceholder: '匯出時將自動填入；若要回復請於此處貼上既有的密文。',
  },
  manageAccount: {
    subtitle: '新增、修改或移除儲存於本機（DPAPI 加密）的 Beanfun 帳號。',
    searchPlaceholder: '搜尋帳號或備註…',
    totalAccounts: '已儲存帳號',
    colAccount: '帳號',
    colRemark: '備註 / 顯示名稱',
    colRegion: '地區',
    colLastLogin: '最近登入',
    colActions: '操作',
    lastLoginUnknown: '—',
    remarkEmpty: '（未設定備註）',
    empty: '目前沒有任何儲存帳號，點擊上方按鈕新增。',
    noSearchResult: '找不到符合條件的帳號。',
    dragDisabledTip: '拖曳排序將於後續版本支援。',
    editAction: '修改',
    copyIdAction: '複製帳號',
    deleteAction: '移除',
    idCopied: '帳號已複製到剪貼簿。',
    import: '匯入',
    export: '匯出',
    importOverwriteConfirmTitle: '匯入帳號',
    importOverwriteConfirm: '匯入將會覆蓋目前所有已儲存的帳號，是否繼續？',
    importSuccess: '帳號資料匯入成功。',
    exportSuccess: '帳號資料匯出成功。',
    exportDefaultFilename: 'Beanfun-Accounts.json',
    footerHint: '所有帳號均以 Windows DPAPI 加密儲存於本機 Users.dat。',
  },
  settings: {
    subtitle: '調整 App 行為與每款遊戲的啟動偏好。',
    aboutLink: '關於',
    gamePathPlaceholder: '尚未設定，點擊以選取遊戲執行檔。',
    gameSectionEmpty: '尚未選擇任何遊戲，遊戲相關設定將於選擇遊戲後出現。',
    disableHardwareAccelerationTip:
      '關閉硬體加速可降低 GPU 使用，但介面動畫會較不流暢。\n更動後需完全重新啟動 Beanfun 才會套用。',
    tradLoginTip: '使用傳統登入流程（多次跳轉），\n適合自動登入失敗或無法跳出登入視窗時使用。',
    killPatcherTip: '阻止 beanfun! 啟動的更新程式 (Patcher.aspx) 自動執行。',
    skipPlayWindowTip: '直接啟動遊戲，跳過 Play 視窗確認步驟。',
  },
  webBrowser: {
    title: '瀏覽器',
    empty: '尚未指定要開啟的網址。',
    cookieRequired: '此頁面需要 Beanfun 登入 Cookie 才能完整顯示，將改以系統預設瀏覽器開啟。',
    openExternally: '在外部瀏覽器開啟',
  },
  errors: {
    auth: {
      session_required: '您的登入狀態已失效，請重新登入。',
      totp_required: '請輸入驗證碼以完成登入。',
      advance_check_required: '伺服器要求進行二次驗證。',
      invalid_totp: '驗證碼錯誤，請重新輸入。',
      not_logged_in: '尚未登入。',
      gamepass_window_already_open: 'GamePass 登入視窗已開啟，請先關閉再重新嘗試。',
    },
    beanfun: {
      bad_credentials: '帳號或密碼錯誤。',
      transport: '網路連線異常，請稍後再試。',
      parse: '伺服器回應格式異常。',
    },
    qr: {
      expired: 'QR Code 已過期，請重新整理。',
      pending: '請於行動裝置完成掃描。',
    },
    config: {
      io_error: 'Config.xml 讀寫失敗。',
    },
    storage: {
      io_error: 'Users.dat 讀寫失敗。',
      invalid_dpapi: '帳號資料解密失敗。',
    },
    process: {
      window_not_found: '找不到目標遊戲視窗。',
      access_denied: '無權存取目標處理程序。',
    },
    system: {
      platform_unsupported: '此功能僅在 Windows 上支援。',
      unknown: '發生未預期的錯誤。',
    },
  },
  themePreset: {
    orange: '橙色',
    green: '綠色',
    lightblue: '淺藍',
    pink: '粉紅',
    gold: '金色',
    silver: '銀色',
    black: '黑色',
    white: '白色',
  },
} as const

/* -------- zh-CN (Simplified) -------- */

const zhCN = {
  loginShell: {
    heading: '缤放 Next',
    subline: '欢迎登录',
  },
  loginRegion: {
    subline: '请选择您要登录的 Beanfun 服务地区。',
    defaultBadge: '默认',
    totpHint: '支持 TOTP',
    tip: '可在「设置 → 通用」随时切换。此选择将记住至下次启动。',
  },
  loginQr: {
    title: '使用 Beanfun App 扫描',
    subtitle: '打开 Beanfun 移动版 App 扫描下方 QR Code 即可登录。',
    unsupportedHK: 'QR 登录仅支持台湾区，已返回登录入口。',
    connectionLost: '无法获取登录状态，请点选重新加载重试。',
  },
  loginTotp: {
    title: '双重验证',
    subtitle: '请输入验证器应用程序显示的 6 位数验证码。',
  },
  loginVerify: {
    title: '二次验证',
    subtitle: '为了确保账号安全，请完成以下验证后重新登录。',
    success: '二次验证已完成，请重新输入账号密码登录。',
  },
  loginGamepass: {
    title: '使用 GamePass 登录',
    subtitle: '请于开启的窗口中完成 GamePass 登录程序。',
    steps: {
      prepare: '准备登录环境',
      openWindow: '开启登录窗口',
      authenticate: '等待认证',
      complete: '登录完成',
    },
    prepareDone: '登录环境已备妥。',
    unsupportedHK: 'GamePass 登录仅支持台湾区，已返回登录入口。',
    connectionLost: '无法与 Beanfun 连接，请点选重新加载重试。',
    windowError: '无法完成 GamePass 登录，请点选重新加载重试。',
    refresh: '重新加载',
  },
  accountList: {
    title: '账号列表',
    subtitle: '选择游戏账号以开始游戏，或新增、管理账号。',
    serviceAccountsHeading: '游戏账号',
    accountCount: '{count} 个账号',
    loading: '加载中…',
    empty: '当前没有任何游戏账号，点击下方按钮新增。',
    loadFailed: '无法加载账号列表，请检查网络后重试。',
    retry: '重试',
    statusOnline: '已连接',
    statusBanned: '已停用',
    gashBalance: 'Gash 点数',
    gashBalancePlaceholder: '—',
    refreshBalance: '刷新点数',
    memberCenter: '会员中心',
    customerService: '客服中心',
    autoPaste: '自动粘贴',
    autoPasteTip:
      '自动输入需要游戏在输入账密界面\n\n※ 自动输入功能可能会由于游戏限制出现偶尔无法正常运行的问题，请斟酌使用。',
    otpHeading: '一次性密码',
    otpPlaceholder: '尚未获取',
    copyOtp: '复制密码',
    toolsButton: '工具',
    changeGame: '切换游戏',
    moreActions: '更多操作',
    dragHandle: '拖动排序',
    gamePlaceholder: '尚未选择游戏',
    gamePathPickerPending: '游戏路径设定画面尚未完成（P12.4 将补上），请稍后再试。',
  },
  addAccountDialog: {
    subtitle: '将 Beanfun 账号加入本机，下次可直接从列表快速登录。',
    regionLabel: '服务地区',
    accountIdLabel: '账号',
    accountNameLabel: '备注（选填）',
    passwordLabel: '密码（选填）',
    verifyLabel: '认证信息（选填）',
    save: '新增',
    duplicateExists: '此区域下已有相同账号，请改用「修改账号」或先删除旧记录。',
  },
  changeAccountDialog: {
    title: '修改账号',
    subtitle: '可调整显示备注与自动登录设定；其他字段请删除后重新新增。',
    regionLabel: '服务地区',
    accountIdLabel: '账号',
    accountNameLabel: '备注',
    accountIdReadonlyHint: '账号为唯一键，无法直接修改；如需更换请先删除后再新增。',
    save: '保存',
  },
  gameList: {
    subtitle: '选择要登录或启动的 beanfun! 游戏。',
    loading: '加载游戏列表中…',
    empty: '此区域目前没有可用的游戏。',
    loadFailed: '无法加载游戏列表，请检查网络后重试。',
    imageAlt: '{name} 游戏封面',
  },
  unconnectedGameAddAccount: {
    loading: '加载中，请稍候…',
  },
  accRecovery: {
    dataPlaceholder: '导出时将自动填入；若要恢复请于此处贴上既有的密文。',
  },
  manageAccount: {
    subtitle: '新增、修改或移除存储于本机（DPAPI 加密）的 Beanfun 账号。',
    searchPlaceholder: '搜索账号或备注…',
    totalAccounts: '已存储账号',
    colAccount: '账号',
    colRemark: '备注 / 显示名称',
    colRegion: '地区',
    colLastLogin: '最近登录',
    colActions: '操作',
    lastLoginUnknown: '—',
    remarkEmpty: '（未设定备注）',
    empty: '当前没有任何存储账号，点击上方按钮新增。',
    noSearchResult: '找不到符合条件的账号。',
    dragDisabledTip: '拖动排序将于后续版本支持。',
    editAction: '修改',
    copyIdAction: '复制账号',
    deleteAction: '移除',
    idCopied: '账号已复制到剪贴板。',
    import: '导入',
    export: '导出',
    importOverwriteConfirmTitle: '导入账号',
    importOverwriteConfirm: '导入将会覆盖当前所有已存储的账号，是否继续？',
    importSuccess: '账号数据导入成功。',
    exportSuccess: '账号数据导出成功。',
    exportDefaultFilename: 'Beanfun-Accounts.json',
    footerHint: '所有账号均以 Windows DPAPI 加密存储于本机 Users.dat。',
  },
  settings: {
    subtitle: '调整 App 行为与每款游戏的启动偏好。',
    aboutLink: '关于',
    gamePathPlaceholder: '尚未设定，点击以选取游戏执行档。',
    gameSectionEmpty: '尚未选择任何游戏，游戏相关设定将于选择游戏后出现。',
    disableHardwareAccelerationTip:
      '关闭硬件加速可降低 GPU 使用，但界面动画会较不流畅。\n更动后需完全重新启动 Beanfun 才会套用。',
    tradLoginTip: '使用传统登录流程（多次跳转），\n适合自动登录失败或无法跳出登录窗口时使用。',
    killPatcherTip: '阻止 beanfun! 启动的更新程序 (Patcher.aspx) 自动执行。',
    skipPlayWindowTip: '直接启动游戏，跳过 Play 窗口确认步骤。',
  },
  webBrowser: {
    title: '浏览器',
    empty: '尚未指定要开启的网址。',
    cookieRequired: '此页面需要 Beanfun 登录 Cookie 才能完整显示，将改以系统默认浏览器开启。',
    openExternally: '在外部浏览器开启',
  },
  errors: {
    auth: {
      session_required: '您的登录状态已失效，请重新登录。',
      totp_required: '请输入验证码以完成登录。',
      advance_check_required: '服务器要求进行二次验证。',
      invalid_totp: '验证码错误，请重新输入。',
      not_logged_in: '尚未登录。',
      gamepass_window_already_open: 'GamePass 登录窗口已开启，请先关闭再重新尝试。',
    },
    beanfun: {
      bad_credentials: '账号或密码错误。',
      transport: '网络连接异常，请稍后再试。',
      parse: '服务器响应格式异常。',
    },
    qr: {
      expired: 'QR Code 已过期，请重新加载。',
      pending: '请于移动设备完成扫描。',
    },
    config: {
      io_error: 'Config.xml 读写失败。',
    },
    storage: {
      io_error: 'Users.dat 读写失败。',
      invalid_dpapi: '账号资料解密失败。',
    },
    process: {
      window_not_found: '找不到目标游戏窗口。',
      access_denied: '无权访问目标进程。',
    },
    system: {
      platform_unsupported: '此功能仅在 Windows 上支持。',
      unknown: '发生未预期的错误。',
    },
  },
  themePreset: {
    orange: '橙色',
    green: '绿色',
    lightblue: '浅蓝',
    pink: '粉红',
    gold: '金色',
    silver: '银色',
    black: '黑色',
    white: '白色',
  },
} as const satisfies KeysMatch<typeof zhTW, typeof zhTW>

/* -------- en-US -------- */

const enUS = {
  loginShell: {
    heading: 'beanfun! Next',
    subline: 'Welcome — please sign in',
  },
  loginRegion: {
    subline: 'Pick the beanfun! region you want to sign in to.',
    defaultBadge: 'Default',
    totpHint: 'TOTP supported',
    tip: 'Change anytime under Settings → General. This choice is remembered for next launch.',
  },
  loginQr: {
    title: 'Scan with the beanfun! app',
    subtitle: 'Open the beanfun! mobile app and scan the QR code below to sign in.',
    unsupportedHK: 'QR login is only available in Taiwan; redirected back to the login entry.',
    connectionLost: 'Unable to reach the login service. Please tap Reload and try again.',
  },
  loginTotp: {
    title: 'Two-factor authentication',
    subtitle: 'Enter the 6-digit code shown in your authenticator app.',
  },
  loginVerify: {
    title: 'Additional verification',
    subtitle:
      'To keep your account safe, please complete the verification below and sign in again.',
    success: 'Verification complete. Please enter your credentials to sign in again.',
  },
  loginGamepass: {
    title: 'Sign in with GamePass',
    subtitle: 'Please complete the GamePass sign-in flow in the window that opens.',
    steps: {
      prepare: 'Prepare login environment',
      openWindow: 'Open sign-in window',
      authenticate: 'Wait for authentication',
      complete: 'Sign-in complete',
    },
    prepareDone: 'Login environment ready.',
    unsupportedHK:
      'GamePass login is only available in Taiwan; redirected back to the login entry.',
    connectionLost: 'Unable to reach Beanfun. Please tap Reload and try again.',
    windowError: 'Unable to complete GamePass sign-in. Please tap Reload and try again.',
    refresh: 'Reload',
  },
  accountList: {
    title: 'Accounts',
    subtitle: 'Pick a game account to start playing, or add and manage accounts.',
    serviceAccountsHeading: 'Game Accounts',
    accountCount: '{count} accounts',
    loading: 'Loading…',
    empty: 'No game accounts yet. Tap the button below to add one.',
    loadFailed: 'Unable to load the account list. Check your connection and try again.',
    retry: 'Retry',
    statusOnline: 'Online',
    statusBanned: 'Disabled',
    gashBalance: 'Gash Balance',
    gashBalancePlaceholder: '—',
    refreshBalance: 'Refresh Balance',
    memberCenter: 'Member Center',
    customerService: 'Support',
    autoPaste: 'Auto Paste',
    autoPasteTip:
      'Auto input requires the game to be on the login screen.\n\n※ Auto input may occasionally fail due to game restrictions. Use at your own discretion.',
    otpHeading: 'One-Time Password',
    otpPlaceholder: 'Not retrieved',
    copyOtp: 'Copy Password',
    toolsButton: 'Tools',
    changeGame: 'Switch Game',
    moreActions: 'More actions',
    dragHandle: 'Drag to reorder',
    gamePlaceholder: 'No game selected',
    gamePathPickerPending:
      'Game path picker is not yet available (coming in P12.4). Please try again later.',
  },
  addAccountDialog: {
    subtitle: 'Save a beanfun! credential locally so you can sign in faster next time.',
    regionLabel: 'Region',
    accountIdLabel: 'Account',
    accountNameLabel: 'Note (optional)',
    passwordLabel: 'Password (optional)',
    verifyLabel: 'Verify info (optional)',
    save: 'Add',
    duplicateExists:
      'An account with this region and ID already exists. Use “Edit Account” or delete the old record first.',
  },
  changeAccountDialog: {
    title: 'Edit Account',
    subtitle:
      'Update the display note or auto-login flag here. Other fields require delete + re-add.',
    regionLabel: 'Region',
    accountIdLabel: 'Account',
    accountNameLabel: 'Note',
    accountIdReadonlyHint:
      'Account ID is the primary key and cannot be edited in place. Delete and re-add to change it.',
    save: 'Save',
  },
  gameList: {
    subtitle: 'Pick a beanfun! game to sign in or launch.',
    loading: 'Loading game list…',
    empty: 'No games are available for this region.',
    loadFailed: 'Unable to load the game list. Check your connection and try again.',
    imageAlt: '{name} cover image',
  },
  unconnectedGameAddAccount: {
    loading: 'Loading, please wait…',
  },
  accRecovery: {
    dataPlaceholder:
      'Export will auto-fill this field; for Recovery, paste your existing ciphertext here.',
  },
  manageAccount: {
    subtitle:
      'Add, edit, or remove the beanfun! credentials stored locally (encrypted with Windows DPAPI).',
    searchPlaceholder: 'Search by account or note…',
    totalAccounts: 'Stored accounts',
    colAccount: 'Account',
    colRemark: 'Note / Display name',
    colRegion: 'Region',
    colLastLogin: 'Last login',
    colActions: 'Actions',
    lastLoginUnknown: '—',
    remarkEmpty: '(no note)',
    empty: 'No stored accounts yet. Tap the button above to add one.',
    noSearchResult: 'No accounts match your search.',
    dragDisabledTip: 'Drag to reorder will be supported in a future release.',
    editAction: 'Edit',
    copyIdAction: 'Copy account ID',
    deleteAction: 'Remove',
    idCopied: 'Account ID copied to clipboard.',
    import: 'Import',
    export: 'Export',
    importOverwriteConfirmTitle: 'Import accounts',
    importOverwriteConfirm: 'Importing will overwrite every account currently stored. Continue?',
    importSuccess: 'Accounts imported successfully.',
    exportSuccess: 'Accounts exported successfully.',
    exportDefaultFilename: 'Beanfun-Accounts.json',
    footerHint: 'All accounts are stored in Users.dat encrypted with Windows DPAPI.',
  },
  settings: {
    subtitle: 'Adjust application behavior and per-game launcher preferences.',
    aboutLink: 'About',
    gamePathPlaceholder: 'Not set — click to choose the game executable.',
    gameSectionEmpty: 'No game selected. Game-specific settings will appear once you pick one.',
    disableHardwareAccelerationTip:
      'Disabling hardware acceleration lowers GPU use but makes UI animations less smooth.\nA full Beanfun restart is required for the change to take effect.',
    tradLoginTip:
      'Use the traditional multi-redirect login flow.\nHandy when auto-login fails or the login window does not appear.',
    killPatcherTip: 'Prevent the beanfun! launcher (Patcher.aspx) from auto-running.',
    skipPlayWindowTip: 'Launch the game directly, skipping the Play window confirmation step.',
  },
  webBrowser: {
    title: 'Browser',
    empty: 'No URL specified.',
    cookieRequired:
      'This page requires the beanfun! login cookie to render fully; opening it in your default browser instead.',
    openExternally: 'Open in external browser',
  },
  errors: {
    auth: {
      session_required: 'Your session expired. Please log in again.',
      totp_required: 'Please enter your TOTP code to continue.',
      advance_check_required: 'Server requires additional verification.',
      invalid_totp: 'Invalid TOTP code. Please try again.',
      not_logged_in: 'Not logged in.',
      gamepass_window_already_open:
        'GamePass login window is already open. Please close it before trying again.',
    },
    beanfun: {
      bad_credentials: 'Wrong account or password.',
      transport: 'Network error. Please try again later.',
      parse: 'Unexpected server response format.',
    },
    qr: {
      expired: 'QR code expired. Please refresh.',
      pending: 'Please complete the scan on your mobile device.',
    },
    config: {
      io_error: 'Config.xml read/write failed.',
    },
    storage: {
      io_error: 'Users.dat read/write failed.',
      invalid_dpapi: 'Failed to decrypt account data.',
    },
    process: {
      window_not_found: 'Target game window not found.',
      access_denied: 'Access to target process denied.',
    },
    system: {
      platform_unsupported: 'This feature is only supported on Windows.',
      unknown: 'An unexpected error occurred.',
    },
  },
  themePreset: {
    orange: 'Orange',
    green: 'Green',
    lightblue: 'Light Blue',
    pink: 'Pink',
    gold: 'Gold',
    silver: 'Silver',
    black: 'Black',
    white: 'White',
  },
} as const satisfies KeysMatch<typeof zhTW, typeof zhTW>

/**
 * Frontend-only translations keyed by locale code. The shape is
 * compile-time-locked to match the canonical `zh-TW` tree via the
 * {@link KeysMatch} marker on `zhCN` / `enUS`.
 */
export const FRONTEND_ONLY_MESSAGES = {
  'zh-TW': zhTW,
  'zh-CN': zhCN,
  'en-US': enUS,
} as const

export type FrontendMessages = typeof zhTW
