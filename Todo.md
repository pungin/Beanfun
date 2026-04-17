# PR #210 Review Follow-ups — Todo

> Follow-up work for findings raised while reviewing #210 after merge.
> Security finding (#1) intentionally skipped per maintainer decision.

## Status: DONE

- Issue: https://github.com/pungin/Beanfun/issues/212
- PR: https://github.com/pungin/Beanfun/pull/213

## Scope

| # | Severity | File | Item | Status |
|---|----------|------|------|--------|
| 2 | SUGGESTION | `ApplicationUpdater.cs` | `_cachedProxy` thread-safety | Fixed (C) |
| 3 | SUGGESTION | `ApplicationUpdater.cs` | UI freeze up to 20s on manual update check | Fixed (D) |
| 4 | SUGGESTION | `ApplicationUpdater.cs` | DRY: probe logic duplicated | Fixed (A) |
| 5 | NIT | `ApplicationUpdater.cs` | UserAgent inconsistency between probe and fetch | Fixed (A) |
| 6 | NIT | `ApplicationUpdater.cs` | Unrelated explanatory comments removed | Fixed (B) |
| 7 | NIT | `id-pass_form.xaml` | `btn_login` lost `MinWidth` binding | Fixed (E) |
| 8 | NIT | `id-pass_form.xaml.cs`, `qr_form.xaml.cs` | Duplicate `btn_StartGame_Click` handler | Accepted as-is |
| 1 | CRITICAL | `ApplicationUpdater.cs` | Third-party proxy supply chain risk | Deferred (documented in #212 out-of-scope) |

## Plan

### Step 1 — Open tracking issue on GitHub
- [x] Issue #212 created

### Step 2 — Create branch
- [x] Branch: `fix/updater-thread-safe-and-dry`

### Step 3 — ApplicationUpdater refactor commits
- [x] Commit A (`cc9fd59`): `refactor(updater): extract TryProbe helper and unify UserAgent` (fixes #4, #5)
- [x] Commit B (`df72007`): `style(updater): restore explanatory comments removed in #210` (fixes #6)
- [x] Commit C (`1d05e15`): `fix(updater): make GetProxy thread-safe using Lazy<string>` (fixes #2)
- [x] Commit D (`275df88`): `perf(updater): run update check on background thread` (fixes #3)

### Step 4 — UI fix commits
- [x] Commit E (`04a828e`): `fix(ui): restore btn_login MinWidth binding in id-pass_form` (fixes #7)
- [~] #8 (duplicate handler) left as-is; WPF code-behind constraints make a real dedupe impossible without over-engineering

### Step 5 — Verify & PR
- [x] `dotnet csharpier check .` — passes
- [x] `dotnet build` — 0 warnings, 0 errors
- [x] Push branch to remote
- [x] Open issue #212
- [x] Open PR #213 (Fixes #212)

## Diff summary

```
 Beanfun/Pages/id-pass_form.xaml      |  2 +-
 Beanfun/Update/ApplicationUpdater.cs | 87 ++++++++++++++++++++++++++----------
 2 files changed, 65 insertions(+), 24 deletions(-)
```

---

# Beanfun → `beanfun-next` 全面重寫 Plan

> 將現行 .NET 8 WPF 版本的 Beanfun 以 **Rust + Tauri v2 + Vue 3 + Element Plus** 重寫，功能與現版 1:1 對齊。
> 舊 `Beanfun/` 目錄在本計畫全部完成前保留不動作為參考。

## Status: PLANNING

## 技術決策（定稿）

| 項目 | 選用 | 備註 |
|---|---|---|
| 殼 | **Tauri v2** | 非 Electron（小/快/原生整合 Rust） |
| 前端 | **Vue 3 + Vite + TypeScript + Element Plus + Pinia + vue-i18n + vue-router** | Element Plus 為指定 UI 庫 |
| 後端 | **Rust**（`reqwest` / `tokio` / `serde` / `des` / `sha2` / `quick-xml` / `regex` / `url` / `tracing` / `anyhow` / `thiserror`） | 全部業務邏輯在 Rust |
| Windows 整合 | `windows` crate（DPAPI / PostMessage / ShellExecute）+ `winreg` + `wmi` | `#[cfg(target_os = "windows")]` 隔離 |
| 測試（後端） | `cargo test`（unit） + `wiremock`（integration）+ `axum`（錯誤邊界 mock） | |
| 測試（前端） | **Vitest + Vue Test Utils**（component）+ Pinia testing | |
| 測試（E2E） | **`tauri-driver` + WebdriverIO** | |
| Platform | **Windows-only** | LocaleRemulator、DPAPI、Registry、WMI 皆 Windows 專屬 |
| 功能範圍 | **與 WPF 版 1:1 對齊** | maplelink 新增功能（多 session、免登入啟動、下載進度條、UU 容錯）不做 |
| 安全升級 | **LR DLL 改用 SHA-256 驗證**（取代現有 `stream.Length` 比對） | 其他功能完全保持一致 |
| 舊版資料 | **Rust handcraft BinaryFormatter parser** 無縫遷移 | `%APPDATA%\Beanfun\Users.dat` / `Config.xml` 與 WPF 版互通 |
| 新專案位置 | **`beanfun-next/`**（與舊 `Beanfun/` 同 repo 並存） | |

## 目錄結構

```
c:\Users\mo030\Desktop\Beanfun\
├── Beanfun/                     # 舊 WPF 原封保留（legacy 參考）
├── Beanfun.sln                  # 舊 solution 保留
├── .github/                     # 舊 CI 保留；新 CI 另建於此
├── Todo.md                      # 本檔
└── beanfun-next/                # 新專案根
    ├── src-tauri/               # Rust + Tauri
    │   ├── src/
    │   │   ├── main.rs
    │   │   ├── lib.rs
    │   │   ├── commands/        # Tauri invoke 入口（薄層：param 驗證 + DTO）
    │   │   │   ├── auth.rs
    │   │   │   ├── account.rs
    │   │   │   ├── otp.rs
    │   │   │   ├── verify.rs
    │   │   │   ├── launcher.rs
    │   │   │   ├── storage.rs
    │   │   │   ├── config.rs
    │   │   │   ├── update.rs
    │   │   │   └── system.rs
    │   │   ├── core/            # 純邏輯，無副作用
    │   │   │   ├── wcdes/       # DES port（對應 C# WCDESComp）
    │   │   │   ├── version/     # 版號比較（對應 ApplicationUpdater.IsNewerVersion）
    │   │   │   ├── parser/      # HTML / VIEWSTATE / akey regex
    │   │   │   ├── legacy/      # BinaryFormatter 手刻 parser
    │   │   │   └── error.rs
    │   │   ├── services/        # 副作用層
    │   │   │   ├── beanfun/     # HTTP login/account/otp/verify
    │   │   │   ├── storage/     # DPAPI + Users.dat
    │   │   │   ├── config/      # Config.xml I/O
    │   │   │   ├── updater/     # GH + proxy probe
    │   │   │   ├── game/        # Launch Normal + LR (SHA-256)
    │   │   │   ├── process/     # WMI / Kill Patcher / PostMessage
    │   │   │   └── registry/    # 遊戲路徑偵測
    │   │   ├── models/          # DTO / DomainModel
    │   │   └── utils/           # SHA-256 helpers
    │   ├── resources/
    │   │   └── locale_remulator/  # 5 個 LR 檔 + SHA-256 常數
    │   ├── tests/               # Integration tests (wiremock)
    │   ├── fixtures/            # 錄製的 HTTP 回應
    │   ├── Cargo.toml
    │   └── tauri.conf.json
    ├── src/                     # Vue 3 + Element Plus
    │   ├── pages/               # 對應 WPF Pages（11 個）
    │   ├── windows/             # 對應 WPF Windows 對話框（16 個）
    │   ├── components/          # 共用（TitleBar / DraggableList 等）
    │   ├── stores/              # Pinia: auth / account / config / ui
    │   ├── services/            # 型別安全的 invoke() 包裝
    │   ├── locales/             # zh-TW.json / zh-CN.json / en-US.json
    │   ├── router/
    │   ├── styles/              # Element Plus 主題色 CSS variable
    │   ├── assets/
    │   ├── App.vue
    │   └── main.ts
    ├── tests/
    │   ├── unit/                # Vitest component / store
    │   └── e2e/                 # tauri-driver + WebdriverIO
    ├── scripts/
    │   └── convert-lang.mjs     # Beanfun/Lang/*.xaml → src/locales/*.json
    ├── package.json
    ├── vite.config.ts
    ├── tsconfig.json
    └── README.md
```

---

## Phases

### P0 — 專案骨架 + CI ✅

> 分三批交付：Chunk 1 = 0.1~0.3 / Chunk 2 = 0.4~0.6 / Chunk 3 = 0.7~0.8。每批完停下 review。

**Chunk 1 — 專案基礎**
- [x] **0.1 Scaffold Tauri v2 + Vue 3 TS**
  - [x] 暫移 `beanfun-next/mockups/` 到 repo 根目錄
  - [x] `npm create tauri-app@latest beanfun-next -- --template vue-ts --manager npm -y --identifier tw.beanfun.next`
  - [x] 搬回 `mockups/`
  - [x] `cd beanfun-next && npm install`
  - [x] `npx tauri icon ../Beanfun/Resources/icon.ico`（沿用舊 logo，產 17 個 icon）
- [x] **0.2 前端相依**
  - [x] runtime: `element-plus` / `@element-plus/icons-vue` / `pinia` / `pinia-plugin-persistedstate` / `vue-i18n@11`（從 v9 升上來，官方停止維護 v9）/ `vue-router@4` / `vuedraggable@4` / `@tauri-apps/api`
  - [x] dev: `vitest@4` / `@vue/test-utils` / `jsdom` / `@types/node`
- [x] **0.3 Rust 相依**（寫入 `src-tauri/Cargo.toml`）
  - [x] runtime: `reqwest` / `reqwest_cookie_store` / `tokio` / `serde` / `serde_json` / `des` / `cipher` / `sha2` / `thiserror@2` / `anyhow` / `tracing` / `tracing-subscriber` / `quick-xml@0.37` / `regex` / `url` / `base64` / `chrono`
  - [x] windows-only: `windows@0.58`（7 個 Win32 feature）/ `winreg` / `wmi`
  - [x] dev: `wiremock` / `axum@0.8` / `assert_matches` / `tempfile` / `pretty_assertions` / `tokio-test`
  - [x] `cargo check` 通過（1m 00s）
- **Chunk 1 驗收** ✅：`npm run tauri dev` 開空白視窗成功（Rust build 47s + Vite 1.6s）、`cargo check` 全綠

**Chunk 2 — 規範 + 測試 + CI**
- [x] **0.4 Lint / Format 設定**
  - [x] `beanfun-next/rustfmt.toml`（max_width=100 / LF / Default heuristics）
  - [x] `beanfun-next/src-tauri/clippy.toml`（msrv / thresholds）
  - [x] `beanfun-next/eslint.config.js`（ESLint 9 flat config + `defineConfigWithVueTs` + `skip-formatting`）
  - [x] `beanfun-next/.prettierrc.json` + `.prettierignore`
  - [x] `beanfun-next/.editorconfig`（放 beanfun-next/ 內避免影響舊 WPF 專案）
  - [x] `package.json` scripts: `lint` / `lint:fix` / `format` / `format:check` / `typecheck` / `test` / `test:watch`
- [x] **0.5 Smoke tests**
  - [x] 前端：`beanfun-next/tests/unit/smoke.spec.ts` + `vitest.config.ts`（jsdom）— 3 passed
  - [x] 後端：`beanfun-next/src-tauri/tests/smoke.rs`（serde_json / reqwest / sha2 / 算術）— 4 passed
- [x] **0.6 GitHub Actions CI**（`.github/workflows/beanfun-next-ci.yml`）
  - [x] matrix: `windows-latest` + `macos-latest`
  - [x] job: rust fmt + clippy + test（Swatinem/rust-cache）
  - [x] job: frontend lint + format:check + typecheck + test
  - [x] path filter `beanfun-next/**` + `workflow_dispatch` + concurrency cancel-in-progress
- **Chunk 2 驗收** ✅：`cargo fmt --check && cargo clippy -- -D warnings && cargo test && npm run lint && npm run format:check && npm run typecheck && npm run test` 全綠、CI YAML 語法驗證 pass

**Chunk 3 — commitlint + README**
- [x] **0.7 Commitlint**（CI-only）
  - [x] `commitlint.config.js`（repo 根）`@commitlint/config-conventional` + `header-max-length: 120` / `body-max-line-length: 0` / `scope-enum: 0` / `ignores` 略過 dependabot Bump 與 Merge commit
  - [x] `.github/workflows/commitlint.yml` 用 `wagoid/commitlint-github-action@v6`，只在 PR 到 `code` 時跑、`ubuntu-latest`、`fetch-depth: 0`
  - [x] 本機用 `npx` 對最近 4 個 commit 跑 commitlint 皆 0 problems
- [x] **0.8 README 骨架**
  - [x] `beanfun-next/README.md`（zh-TW）— 覆蓋 Tauri 預設模板
  - [x] 內容：專案定位 / 技術棧 / 環境需求 / 快速開始 / 前後端指令表 / 資料夾結構 / 測試說明 / 開發規範 / Roadmap 指向 `Todo.md` / License
- **Chunk 3 驗收** ✅：本機 `npm run lint / format:check / typecheck / test` 全綠、commitlint 對歷史 commit 通過、YAML 語法驗證

- **P0 總驗收** ✅：scaffold + lint/fmt + 前後端 smoke tests + CI matrix (win/mac) + commitlint CI + README 齊全

### P1 — Rust `core/wcdes`（DES/ECB/NoPadding）✅

- [x] `core/wcdes/mod.rs`：`encrypt_hex(plaintext: &str, key: &str) -> Result<String>`、`decrypt_hex(hex_str: &str, key: &str) -> Result<String>` + `WcdesError` typed enum（thiserror）
- [x] 行為對齊 C# `DES.Create() + Mode=ECB + Padding=None + Encoding.ASCII`
  - [x] ASCII 編解碼：非 ASCII code point 用 `?` (0x3F) 取代（對齊 `System.Text.Encoding.ASCII` lossy fallback）
  - [x] hex 輸出大寫（對齊 `BitConverter.ToString(..).Replace("-","")`）、hex 輸入大小寫皆可
  - [x] 不自動 trim `\0`（保留 C# `otp.Trim('\0')` 由呼叫端決定的語意）
- [x] 單元測試：8-byte / 16-byte / 24-byte plaintext + trailing-NUL OTP 情境（共 4 組 roundtrip）
- [x] Fixture 測試：6 組 `(key, plaintext, ciphertext)` 用 Node `crypto.createCipheriv('des-ecb', autoPadding=false)` 產（與 C# DES.Create 位元等同），`encrypt_matches_wpf_fixtures` / `decrypt_matches_wpf_fixtures` 雙向驗證
- [x] 額外錯誤路徑：key 長度 0/5/7/9、plaintext 非 8 倍數、hex 奇數長度 / 非 16 倍數 / 含非法字元（9 個 error-path tests）
- [x] 非 ASCII 容錯行為測試：key / plaintext 含 `é` / `中` 對齊 `?` 取代結果
- **驗收** ✅：`cargo test core::wcdes` 19 passed / 0 failed、`cargo clippy -D warnings` 綠、`cargo fmt --check` 綠、cipher 位元等同 WPF（已以 Node ground-truth fixture 驗證；真實 WPF runtime OTP 回應將於 P3 登入 flow 錄到後再補一組 integration fixture）

### P2 — Rust `core/version` + `core/parser` ✅

- [x] `core/version/mod.rs`：`is_newer(local: &str, remote: &VersionInfo) -> bool`
- [x] 覆蓋 WPF `IsNewerVersion` 所有 case（5.8.9 < 5.8.10、timestamp 相同、舊格式無 patch、regex-miss fallback、i64 overflow）
- [x] `core/parser/viewstate.rs`：`extract_viewstate(html: &str) -> Result<ViewStateForm>`（`__VIEWSTATE` 必填，`__VIEWSTATEGENERATOR` / `__EVENTVALIDATION` 為 `Option`，對齊 WPF 多個呼叫端對同一 parser 的不同要求）
- [x] `core/parser/account.rs`：從 `game_server_account_list.aspx` HTML 抽出 `ServiceAccountRow { is_enable, sid, ssn, sname }` 清單 + `extract_account_limit_notice`（帳號上限提示）
- [x] `core/parser/akey.rs`：從 redirect URL / JSON 字串抓 `akey=...`（對齊 WPF 貪婪 `(.*)` 行為，docs 已註記）
- [x] `core/parser/token.rs`：從 HTML 抓 `__RequestVerificationToken`（支援 `name="..."` 與 `id="..."` 兩種 emit 形式）
- [x] 單元測試：每個 parser ≥ 5 cases，共 28 個 parser tests
  - viewstate 7、account 8（5 rows + 2 notice + 1 empty）、akey 7、token 6
- [x] SRP/DRY 自我 review：共用 `ParserError` enum、`capture_first` / `compile_field` helper 去除 dispatch-with-panic 分支
- **驗收** ✅：`cargo test core::parser` 28 passed / 0 failed、`cargo test core::version` 15 passed / 0 failed、全套 62 lib tests + 4 smoke 綠、`cargo clippy -D warnings` 綠、`cargo fmt --check` 綠。Fixture 全為 hand-crafted WPF-aligned HTML snippet；真實 login-flow response 將於 P3 錄到後補 integration fixture。

### P3 — Rust `services/beanfun` Login

範圍：TW Regular + HK Regular + TOTP + QRCode + Logout。切 5 chunk：

#### Chunk 3.1 — Client skeleton + session_key ✅

- [x] `Cargo.toml` 新增 `zeroize`（reqwest/tokio/url/serde/wiremock 在 P0 已備）
- [x] `services/mod.rs` + `services/beanfun/mod.rs` — layer docs + `pub use` re-export
- [x] `services/beanfun/error.rs` — `LoginError` enum（18 variants 覆蓋所有 WPF errmsg）
- [x] `services/beanfun/session.rs` — `Credentials`（zeroize + redact Debug）/ `Session`（redact Debug）
- [x] `services/beanfun/client.rs` — `BeanfunClient`（雙 reqwest client 共享 cookie store、follow / no-follow redirect）、`ClientConfig`（timeout 30s、body cap 16 MiB、固定 UA）、`Endpoints`（TW / HK / custom）、`LoginRegion` enum、`bounded_text` 串流防 OOM
- [x] `services/beanfun/login/session_key.rs` — region-aware `get_session_key`（TW 抓 redirect URL query / HK 抓 body span）
- [x] Integration tests `tests/session_key.rs`（wiremock）：TW 302→URL 抓 key / TW missing / HK span 抓 key / HK missing span / HK empty body / body-cap / UA 比對
- **驗收** ✅：77 lib tests + 7 session_key integration + 4 smoke = 88 全綠、`clippy -D warnings` 綠、`fmt --check` 綠

#### Chunk 3.2 — TW Regular 完整 flow ✅

- [x] `core/parser/form.rs` — `extract_hidden_inputs` 共用 SendLogin HTML scrape（8 unit tests）
- [x] `BeanfunClient::{login_url, login_url_with_skey, portal_url}` helper + `login::ensure_success` / `login::apply_json_headers` 共用減 DRY
- [x] `login/index.rs` — GET `Login/Index?pSKey=…`、remap `ParserError::MissingRequestVerificationToken` → `LoginError::MissingVerificationToken`
- [x] `login/check_account_type.rs` — POST `Login/CheckAccountType`、JSON body sniff（非 `{` 視同無 captcha）、typed DTO
- [x] `login/account_login.rs` — POST `Login/AccountLogin`、`classify_outcome(code, result, msg)` 純函式 + 7 unit tests 對應 4 分支（含 `("1", "")` / `("1", "2")` ⇒ Ok 的 WPF-permissive case）
- [x] `login/send_login.rs` — GET `Login/SendLogin`、空 form 直接 `SendLoginNoFormData`
- [x] `login/return_aspx.rs` — POST `return.aspx`（no-redirect）、raw Set-Cookie 掃 `bfWebToken`（4 unit tests）
- [x] `login/tw_regular.rs` — orchestrator `login_tw_regular(client, creds) -> Session`
- [x] Integration tests `tests/tw_login.rs`：10 支（7 主流程 + CheckAccountType non-JSON tolerance + captcha step2→step3 propagation + 跨 step session cookie persistence）
- **驗收** ✅：99 lib tests + 7 session_key + 4 smoke + 10 tw_login = 120 全綠、`clippy -D warnings` 綠、`fmt --check` 綠
- **Post-commit polish**（對齊 WPF + 補覆蓋率）：`classify_outcome` match arms 改成 `("1", "1") ⇒ AdvanceCheck`, `("1", _) ⇒ Ok`（WPF L101-107 故意 lenient）、`scan_bfwebtoken` `(?i)` 標記為 intentional divergence

#### Chunk 3.3 — HK Regular + TOTP + LoginCompleted

拆成 4 個 sub-chunk：

##### Chunk 3.3.1 — `login_completed` 共用尾巴 ✅

- [x] `login/completed.rs` — `build_completed_form(session_key, akey)` 純函式 + `login_completed(client, session_key, akey, account_id, service_code, service_region) -> Session`（複用 Chunk 3.2 `post_return_aspx`、對齊 WPF L853-858 `{SessionKey, AuthKey, ServiceCode="", ServiceRegion="", ServiceAccountSN="0"}`）
- [x] Unit tests：欄位順序 / 值 / 長度（6 支）
- [x] Integration tests `tests/login_completed.rs`（5 支）：TW happy / HK region stamp / POST body 含 SessionKey+AuthKey+ServiceAccountSN=0 / ServiceCode & ServiceRegion 空字串 / MissingWebToken 傳遞
- **驗收** ✅：105 lib + 5 login_completed + 7 session_key + 4 smoke + 10 tw_login = 131 全綠、`clippy -D warnings` 綠、`fmt --check` 綠

##### Chunk 3.3.2 — HK Regular flow ✅

- [x] `LoginError::TotpRequired` 從 unit variant → `TotpRequired(Box<TotpChallenge>)`（附新 doc block 說明「continuation 用 Err channel surface 理由」）
- [x] `login/totp_challenge.rs` — `TotpChallenge { totp_url, viewstate, session_key, account_id }`（Debug redact session_key + viewstate、accessor only 給 totp_url + account_id、3 支 unit tests）
- [x] `login/hk_error.rs` — `HkErrorSignal { MsgBox(s) | PollRequest{url,token,param} | Unrecognized }` 純函式 + 9 支 unit tests（含 MsgBox 中文 / MsgBox vs PollRequest 優先序 / 空 token 拒絕等）
- [x] `login/hk_regular.rs` — GET / POST `id-pass_form_newBF.aspx`（loose viewstate regex + HK 強制三欄 Some）、4 路分支照 WPF L247-285 優先序（RELOAD_CAPTCHA_CODE → AdvanceCheck / totpLoginBtn → TotpRequired / final URL akey → `login_completed` / MsgBox or pollRequest → ServerMessage / Unrecognized → MissingAkey）、9 支 unit tests（URL build / form order / is_advance_check / is_totp / classify_missing_akey）
- [x] `login/mod.rs` 暴露 `TotpChallenge`, `login_hk_regular`, `HkErrorSignal`, `extract_hk_error_signal`
- [x] Integration tests `tests/hk_login.rs`（10 支）：HK happy / totp 觸發 + 驗 challenge fields / advance check / MsgBox / pollRequest / MissingViewState / MissingViewStateGenerator / MissingEventValidation / Unrecognized no-akey / POST body 含 credentials + viewstate
- [x] 順手清 3.2 遺漏的 5 支 rustdoc warnings（redundant explicit link + ambiguous fn/mod）
- **驗收** ✅：127 lib + 5 login_completed + 7 session_key + 4 smoke + 10 tw_login + 10 hk_login = 163 全綠、`clippy -D warnings` 綠、`fmt --check` 綠、`cargo doc` 0 warning

##### Chunk 3.3.3 — TOTP flow ✅

- [x] `login/totp.rs` — `login_totp(client, challenge, otp1, otp2, otp3, otp4, otp5, otp6) -> Session`（對齊 WPF 簽章 6 個獨立 `&str` 參數）
- [x] POST 暫存的 `totp_url`（payload：`__EVENTTARGET=""` + `__EVENTARGUMENT=""` + 3 viewstate + `__VIEWSTATEENCRYPTED=""`(HK only, region 從 `client.config().region` 取) + `otpCode1..6` + `totpLoginBtn="登入"`）
- [x] 3 路分支（akey → `login_completed` / RELOAD_CAPTCHA_CODE → AdvanceCheck / MsgBox or pollRequest → ServerMessage）複用 `hk_error`
- [x] 搬 `is_advance_check` + `classify_missing_akey_body` 從 `hk_regular.rs` 到 `hk_error.rs`（DRY，HK Regular + TOTP 共用）
- [x] `TotpChallenge` 拿掉 `#[allow(dead_code)]`（viewstate/session_key 進入 `login_totp` 實際使用）
- [x] Unit tests：form builder × 4（HK 13 欄順序、TW 12 欄不含 `__VIEWSTATEENCRYPTED`、值填充、OTP 位置映射）
- [x] Integration tests：7 支（happy、advance check、MsgBox、pollRequest、unrecognized、HK wire shape、TW wire shape）
- [x] Quality gates：fmt / clippy -D warnings / 175 tests pass / doc 0 warnings

##### Chunk 3.3.4 — CheckIsRegisteDevice ✅

- [x] `LoginError` 新增 `DeviceRegistrationRequired { login_token, poll_url, param }` / `DeviceLoginTimeout` / `DeviceLoginRejected` 三個 variant（對齊 WPF L2400-2441 bfAPPAutoLogin_Tick switch branches）
- [x] 重構 `hk_error::classify_missing_akey_body`：`pollRequest` 路徑改回 `DeviceRegistrationRequired` 並保留 `login_token` + `poll_url` + `param`（原本只丟 display-only `ServerMessage`；chunk 3.3.2 / 3.3.3 測試同步更新）
- [x] 修正 `Endpoints::hk().newlogin_base` → `https://tw.newlogin.beanfun.com/`（對齊 WPF `CheckIsRegisteDevice` L675-676 在 HK region 也硬寫 TW host 的行為；`endpoints_hk_has_production_urls` test 補上 assertion）
- [x] `login/registered_device.rs` — 新模組 + `login_registered_device(client, login_token, session_key, account_id, service_code, service_region) -> Result<Option<Session>, LoginError>`（single-shot API：`Ok(Some(session))` / `Ok(None)` keep-polling / 各 IntResult 錯誤 variant；WPF `CheckIsRegisteDevice` L667-700 + `MainWindow.bfAPPAutoLogin_Tick` L2418-2439 對齊）
- [x] `IntResult=="2"` 路徑：內部呼叫 `login_completed`（side-effect GET `{newlogin_base}login/{StrReslut}` + `extract_akey` on StrReslut）；AKeyParseFailed 時回 `Ok(None)` 保 WPF 靜默 retry 行為
- [x] `login/mod.rs` 註冊 `registered_device` + re-export `login_registered_device`
- [x] Unit tests：`PollResponse` serde shape（2 支）
- [x] Integration tests `tests/registered_device.rs`（11 支）：happy IntResult==2 / akey-less 2 / 0 / 1 / -1 / -2 / -3 / 未知 IntResult / missing IntResult / POST 帶 LT= / host 路由到 newlogin_base
- **驗收** ✅：所有 fmt / clippy -D warnings / cargo test / cargo doc 全綠

#### Chunk 3.4 — QRCode flow

##### 3.4.1 — `qr_init` ✅
- [x] `LoginError::QrUnsupportedRegion` variant（HK region 早退；對齊 WPF `MainWindow.loginMethodInit` L1099-1114 UI guard + `BeanfunClient` QR path 全程硬寫 `https://login.beanfun.com`）
- [x] `login/qr_init.rs` — `init_qr_login(client, session_key) -> Result<QrLoginInit, LoginError>`：region guard → 複用 `get_login_index`（step 1：GET `Login/Index?pSKey=…` + 抓 `__RequestVerificationToken`）→ GET `Login/InitLogin?pSKey=…`（Accept / X-Requested-With / Origin / Referer 四 header 比照 WPF `getQRCodeStrEncryptData` L455-466）→ JSON 解析 → 層層檢查 `Result==0` / `ResultData` / `QRImage` 非空（對齊 WPF L429-441 + L469）
- [x] `QrLoginInit { bitmap_base64, deeplink: Option<String>, verification_token }`：保留 WPF 儲存格式 `bitmapBase64 = "data:image/png;base64,…"` 給前端 `<img>` 直用；`deeplink` 用 `Option` 對齊 WPF null/空字串行為
- [x] `normalize_beanfun_app_deeplink` 純 helper（WPF L478-504）：`play.games.gamania.com/.../deeplink/?url=…` → 解 inner url；非匹配 host/path 或缺 `?url=` → raw 原樣回；host/path 比對 case-insensitive 對齊 WPF `OrdinalIgnoreCase`
- [x] `login/mod.rs` 註冊 `qr_init` + re-export `init_qr_login` / `QrLoginInit` / `normalize_beanfun_app_deeplink`
- [x] Unit tests：`normalize_beanfun_app_deeplink` 8 支邊界 + `InitLoginResponse` serde shape 2 支
- [x] Integration tests `tests/qr_init.rs`（15 支）：happy / data URL prefix / deeplink unwrap / deeplink plain / deeplink missing / deeplink empty / HK region 短路且無 HTTP traffic / step1 缺 token / Result!=0 / Result 缺 / ResultData 缺 / QRImage 缺 / QRImage="" / 非 JSON body / 4 header 完全比對
- **驗收** ✅：fmt / clippy -D warnings / cargo test (216 pass) / cargo doc 全綠
- **Divergence**：JSON parse 失敗用 `LoginError::Json(...)` 取代 WPF `JObject.Parse` 未捕例外（與 P3.3.4 同原則，安全性 strictly better）

##### 3.4.2 — `qr_poll` ✅
- [x] `QrLoginInit` 加 `pub skey: String` 欄位（poll/finalize 都要從中取 skey 重建 Referer URL，對齊 WPF L538/L618 從 `qrcodeclass.skey` 拿；single arg `&QrLoginInit` 取代多個 loose `&str`）
- [x] `login/qr_poll.rs` — `poll_qr_login_status(client, &init) -> Result<QrPollOutcome, LoginError>` single-shot：region guard → POST `https://login.beanfun.com/QRLogin/CheckLoginStatus`（注意是 `/QRLogin/`，不在 `/Login/` 底下）+ 5 header（Accept / Referer / Origin / RequestVerificationToken / Content-Type=`application/x-www-form-urlencoded`，**不**送 X-Requested-With —— 對齊 WPF `SetBaseHeaders` L917 清空 + L615-621 重設後沒加回）+ 空字串 body → JSON 解析 → 4-way 對齊
- [x] `QrPollOutcome` enum 4 個 variant 對齊 WPF L640-653 實際 4 個 `ResultMessage` 字串：`Failed` / `WaitLogin` / `TokenExpired` / `Approved`（option B 不合併；`Approved` 不帶 ResultData，因 WPF L647-648 也沒讀，finalize 從 `init.skey` 取）
- [x] 錯誤對齊：unknown / 缺 ResultMessage → `LoginError::ServerMessage(raw_body)`（WPF L640+L649-652 同分支）；JSON parse fail → `LoginError::QrJsonParseFailed`（WPF L634-638）；HK region → `LoginError::QrUnsupportedRegion`（短路無 HTTP，跟 qr_init 一致）
- [x] `login/mod.rs` 註冊 `qr_poll` + re-export `poll_qr_login_status` / `QrPollOutcome`
- [x] Unit tests（3 支）：`PollResponse` serde shape — 接受額外 ResultData / 鎖大寫 CamelCase 欄名 / 缺欄=None
- [x] Integration tests `tests/qr_poll.rs`（9 支）：4 個 happy `ResultMessage` / unknown / 缺 ResultMessage / 非 JSON / HK 短路 / wire shape（5 header + 空 body + 確認**不**送 X-Requested-With）
- **驗收** ✅：fmt / clippy -D warnings / cargo test (228 pass) / cargo doc 全綠

##### 3.4.3 — `qr_finalize` ✅
- [x] `login/qr_finalize.rs` — `finalize_qr_login(client, &init) -> Result<Session, LoginError>`：region guard → step 1 GET `QRLogin/QRLogin`（handshake，body 丟掉，Accept=`application/json, text/plain, */*` + Referer=`Login/Index?pSKey={skey}`，對齊 WPF L535-541）→ step 2 複用 `send_login` 帶 QR 專用 Accept（`text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8`，對齊 WPF L545，跟 TW Regular L124 多三個 image MIME）→ step 3 複用 `post_return_aspx`（no-redirect，Referer=login_base，POST SendLogin form 並丟掉 transient bfWebToken，對齊 WPF L588-598）→ step 4 複用 `login_completed`（5-field `AuthKey="OK"` form POST，從 cookie jar 重抓 canonical bfWebToken，對齊 WPF L838-882 / L774-782）→ 回 `Session { region: TW, skey, web_token: step4 token, account_id: "", service_code/region: TW defaults }`
- [x] **DRY refactor**：`send_login` 簽名加 `accept: &str` 參數（TW Regular L124 vs QR L545 兩條 Accept 字串不同 → 由 caller 帶；SRP 改進 — Accept 是「自我描述」細節本來就該由 caller 提供）；`tw_regular.rs` callsite 同步更新傳 TW Accept literal
- [x] **DRY 複用 step 4**：直接複用 HK Regular / TOTP 的 `login_completed`（不重抄 5-field form），唯一 QR 專屬參數是 `akey="OK"` sentinel + `account_id=""`
- [x] **DRY 評估通過**：原本擔心 `Origin from login_base` 會超過 Rule of Three，實測 qr_finalize 不需 Origin（WPF 四步都沒設 Origin），維持 qr_init/qr_poll 兩處不抽 helper
- [x] **不新增 LoginError variant**：複用 `QrUnsupportedRegion` / `SendLoginNoFormData` / `MissingWebToken` / `Unknown` / `Http`
- [x] `login/mod.rs` 註冊 `qr_finalize` + re-export `finalize_qr_login`
- [x] Unit tests（2 支）：`QR_SEND_LOGIN_ACCEPT` byte-for-byte 對齊 WPF L545；QR Accept 是 TW Regular Accept + 三個 image MIME 的嚴格擴充（防止未來改錯）
- [x] Integration tests `tests/qr_finalize.rs`（12 支，**+3 by chunk 3.4 review**）：happy（**鎖 web_token == step 4 token，且 != step 3 token**）/ HK 短路 / step1 5xx / step2 空 form / step3 缺 cookie 短路（驗證 step 4 不被觸發）/ **step4 缺 cookie → MissingWebToken（canonical 失敗面）** / step1 wire shape / step2 wire shape / step3 wire shape (SendLogin form body) / **step4 wire shape (5-field AuthKey=OK form, 鎖 step3 不洩漏到 step4)** / **step3→step4 sequencing** / `Session.account_id == ""`（鎖 P3.5 之前的設計）
- **驗收** ✅：fmt / clippy -D warnings / cargo test 全綠 / cargo doc 全綠
- **Documented divergence**：step 3 + step 4 `Accept: */*` vs WPF 完全不送 Accept — reqwest 0.12 (via hyper) 自動注入 `Accept: */*` 沒有 public API 抑制；RFC 9110 §12.5.1 規定 Accept 缺省等於 `*/*` → 語意完全等價，無 Beanfun endpoint 對此分支差異敏感。模組 doc 與 step3 / step4 wire-shape 測試都明確記錄

###### Chunk 3.4 review — 對齊修正
- 初版誤判 WPF `LoginCompleted` 第二次 `return.aspx` POST 為「冗餘」並跳過。Re-read WPF L838-882：`LoginCompleted` 在 POST 完之後 **重抓** cookie jar 的 `bfWebToken`（L868），意味著開發者預期此 POST 可能輪換 token / 影響 session。在無實機 fixture 驗證的前提下，遵守 1:1 對齊原則必須打進此 POST，否則承擔 stale token 風險
- Fix：`finalize_qr_login` 改成 4 step（加 `login_completed("OK", ...)`），`step3` 的 token 顯式 discard。模組 doc 整段重寫（移除「skip redundant POST」段、加「Why we run step 4」說明）
- 同步修 `completed.rs` doc bug：原本誤寫成存在於 `QRCodeCompleted`（不存在的方法），實際上 QR / HK Regular / TOTP 三條都共用 `LoginCompleted`，只有 TW Regular 用 inline `return.aspx`（form 不同所以無法共用）；`akey` 參數說明補上 QR 用 `"OK"` literal sentinel

#### Chunk 3.5 — Logout + 整合 + 收尾

- [x] `login/logout.rs` — 3-step region-aware（GET `remove_bflogin_session.ashx` → GET `logout.aspx?service=999999_T0` → POST `erase_token.ashx`，TW only），best-effort all-steps + return first error
- [x] `BeanfunClient::newlogin_url(path)` helper（對齊既有 `portal_url` / `login_url` API；首批用戶為 logout step 2 TW + step 3）
- [x] Top-level `login_with(client, method, &creds)` dispatcher（`login/orchestrator.rs`）。`LoginMethod` enum 只列 single-shot password flow（`TwRegular` / `HkRegular { service_code, service_region }`）
  - **TOTP / QR 不進 enum**：TOTP 需 mid-flow 互動式 6-digit code 輸入、QR 是 3-step UI-driven flow（init → poll → finalize）。兩者 input/output shape 與單呼叫 dispatcher 不相容；模組 doc 完整說明。device-registered re-login 屬 TOTP 錯誤恢復路徑、不算頂層方法
- ~~cookie jar 清空 helper~~：嚴格對齊 WPF（`Logout()` 從不清自家 `WebClient` cookie jar）。長期隔離靠 drop + 重建 `BeanfunClient`，不另開 `clear_cookies` API
- [x] `tests/logout.rs` — 10 支 per-step 測試（TW happy / HK happy 驗 step3 不被打 / step2 wire shape `service=999999_T0` / step3 wire shape `web_token=1` + form CT / 三 step 各一支 5xx + 驗 best-effort 跑完 / multi-step fail 驗 first error / TW vs HK step2 host routing）
- [x] `tests/login_then_logout.rs` — 2 支 cross-flow（TW Regular login → logout 3 step / HK Regular login → logout 2 step）；額外 lock cookie jar 在 logout 後仍非空（never_clear policy 對齊）
- [x] `tests/orchestrator.rs` — 3 支 dispatch 測試（TW dispatch / HK dispatch 帶 region defaults / HK 自訂 service args plumb-through）
- **驗收**：全 P3 integration test 全綠（合計 ~258 tests）

###### Chunk 3.5 設計決議
- **Logout error policy**：best-effort all-steps + return first error。WPF callers 全部用 `try { } catch { }`（`App.xaml.cs` L72-76、`MainWindow.xaml.cs` L237-241），等同 fire-and-forget；我們改成回 first error 一方面提供 diagnostic value（後續 step 失敗常是同源 cascade），另一方面 caller 想忠實對齊 WPF 直接 `let _ = logout(&client).await;` 即可
- **Cookie jar policy**：never_clear（嚴格對齊 WPF）。WPF `Logout()` 不清 cookie，session 失效靠 server-side 端點處理；我們的 client 想開新 session 就 drop 後重建（已寫進 `client.rs` 模組 doc）。cross-flow test 在 logout 後 assert `bfWebToken` 仍在 jar，鎖死此設計
- **Dispatcher 範圍**：只放 TW Regular + HK Regular。TOTP / QR 因 input/output shape 與單呼叫 dispatcher 不相容（多步 + 互動 / UI-driven）必須直接呼叫對應的 `login_*` / `init_qr_login` / `poll_qr_login_status` / `finalize_qr_login`

###### Chunk 3.5 review — doc 修正
- `orchestrator.rs::LoginMethod::TwRegular` doc 原本誤寫成「TW 從 SendLogin form 的 hidden inputs scrape service_code」。實情：`login_tw_regular` 簽名根本不收 service args、Session 永遠用 region defaults，真正原因是 GetAccounts 整體延到 P4 才會用到 service args；修正為描述「為何簽名沒收」與「P4 GetAccounts 落地後的相容路徑」
- `logout.rs` failure policy doc 原本只講「我們 vs WPF 對 error 的處理差異」，漏講「WPF 內部其實第一個 step 拋 `WebException` 就直接出方法、後續 step 不跑」。修正為明列兩個 intentional divergence（all-steps vs short-circuit、return-first-error vs 全吞），並補一節說明為什麼選 first error 而不是 `Vec<LoginError>`
- `client.rs::cookie_store()` 的 doc 原本寫「(logout)」，但 chunk 3.5 拍板 never_clear 後 logout 已不使用此 API，整個 codebase 唯一 caller 是 cross-flow test。重寫為誠實描述「正常 caller 不該需要、目前唯一實際 caller 是 lock never_clear policy 的 test」、把 invariant rationale 指回 `logout.rs` module doc。`pub` visibility 保留（integ tests 只看得到 `pub`，且未來 P4/P6 多 session 診斷可能合理需要）
- `logout.rs` 模組 doc 原本 hardcode `client.rs` 行號 `L20-22`；改成指向 `client.rs` 的 "Cookie jar" section

### P4 — Rust `services/beanfun` Account / OTP / Verify

切 4 chunks，順序：account read+JSON 管理 → OTP → verify → account WebForms 管理。

#### Chunk 4.1 — `services/beanfun/account.rs` 讀取 + JSON 管理 endpoints
- [x] `get_accounts(client, session, service_code, service_region) -> Result<AccountListResult>`
- [x] `get_create_time(client, session, service_code, service_region, sn) -> Option<String>`（私有 helper；對齊 WPF `try { ... } catch { return null; }` 用 `Option`）
- [x] `get_service_contract(client, session, service_code, service_region) -> Result<String>`
- [x] `add_service_account(client, session, name, service_code, service_region) -> Result<bool>`
- [x] `change_service_account_display_name(client, session, new_name, game_code, account: &ServiceAccount) -> Result<bool>`
- [x] Types：`ServiceAccount` / `AccountListResult` / `AmountLimitNotice` enum
- [x] Integration tests：13 cases（5× `get_accounts`、2× `get_service_contract`、3× `add_service_account`、3× `change_service_account_display_name`）

##### Chunk 4.1 實作 / 設計決議
- **`core/time.rs`**：新建 `dt_compact` (`Y(M-1)DDhhmmssfff`) / `dt_iso` (`yyyyMMddHHmmss.fff`) + `_now` wrappers，移植 WPF `BeanfunClient.cs::GetCurrentTime(2)` / `(1)` 的字串格式。函式收 `chrono::DateTime<Local>` 參數讓單元測試 pin 時間。**不引用舊 WPF 程式**，只依規格重寫
- **`core/parser/account.rs`**：新增 `extract_service_account_create_time` + `service_account_create_time_regex`（`<input ... id="dteCreate" ... value="..."`）對應 WPF 的 inline regex
- **`add_service_account` / `change_service_account_display_name` 空字串短路**：未呼叫網路、直接 `Ok(false)`，對齊 WPF `if (sName == "") { return false; }` / `if (newName == "") { return false; }`
- **`change_service_account_display_name` same-name 短路**：對齊 WPF `if (acc.sname == newName) { return false; }`，UI 層不需要重複防呆
- **`gamezone.ashx` JSON `intResult` 解析**：對齊 WPF `JObject.Parse` + `jsonData["intResult"] == null || (int) jsonData["intResult"] != 1`，empty body / null 都算 `Ok(false)`，invalid JSON 才回 `LoginError::Json`
- **`get_create_time` N+1 失敗靜默**：對齊 WPF `try { ... } catch { return null; }`，不污染 `get_accounts` 的回傳，而是各 row `screatetime: None`

#### Chunk 4.2 — `services/beanfun/otp.rs` ✅
- [x] `get_otp(client, session, account, service_code, service_region) -> Result<String>`：5 HTTP step + WCDES decrypt 共 6 步 orchestration，呼叫 `core/wcdes::decrypt_hex`
- [x] WPF dev artifact 一律不移植（`Expect100Continue = false` 與 reqwest 預設等價、commented `Thread.Sleep` 是 dead code）
- [x] `error.rs` 加 7 個 OTP 專屬 `LoginError` variants（1:1 對應 WPF errmsg：`OTPNoLongPollingKey` / `OTPNoUnkData` / `OTPNoCreateTime` / `OTPNoSecretCode` / `OTPNoResponse` / `GetOtpError` / `DecryptOTPError`）
- [x] `tests/otp.rs` 12 cases pass：TW happy + HK happy + 4 step1 errors + 1 step2 error + 3 step5 errors + 1 step6 decrypt error + 2 wire-shape locks
- [x] Quality gates：fmt / clippy `-D warnings` / cargo test 全綠 / cargo doc 0 warnings

##### chunk 4.2 設計決議
- **5 step 拆 5 個 private helper（SRP）**：`step_1_init` / `step_2_get_secret_code` / `step_3_record_start` / `step_4_long_poll` / `step_5_get_otp` + 純函式 `step_6_decrypt`。每步的純解析邏輯獨立成 `parse_long_polling_key` / `parse_unk_data` / `parse_screatetime_fallback` / `parse_secret_code` 並有 unit test
- **OTP step 2 `loginHost` 區域不對稱**：TW=`tw.newlogin.beanfun.com`、HK=`login.hk.beanfun.com`；既有 `Endpoints` 的 `newlogin_base` 兩 region 都指 TW（給 QR poll 用），所以 `step_2_get_secret_code` 內部 `match client.config().region` 切換 `newlogin_url` (TW) / `login_url` (HK)，**不**改 `Endpoints` schema（單一 caller，wiremock 測試一個 mock server 同時 serve 兩 host 沒問題）
- **`account.screatetime` 缺值 fallback**：WPF 會 mutate `acc.screatetime`（L64），我們改用 local `String` 存於 `Step1Data.screatetime`，`&ServiceAccount` 維持 immutable borrow；fallback 用 `core::parser::extract_service_account_create_time`（DRY，同 P4.1 的 regex）
- **WPF greedy regex `(.*)"` 1:1 移植**：保留 WPF 行為（line-bound greedy match），doc 標注；測試 fixture 用換行讓 greedy 不跨行（生產 response 本身就是多行 JS）
- **`build_get_webstart_otp_url` 用 `format!` 字串拼**：step 5 URL 必須 byte-for-byte match WPF（`CreateTime` 用 `%20` 而非 form-encoded `+`、`ppppp=` 64-char hex literal verbatim），reqwest `.query()` 會把空格編成 `+` 不符；其他參數都已 URL-safe 不需要額外 encode
- **`tick_count_ms()` 對應 `Environment.TickCount`**：用 `chrono::Local::now().timestamp_millis() as i32`（保留 i32 wrap-around 語意）；server 不驗證，純 cache buster
- **`OtpServerRejected.message` 不帶 i18n prefix**：WPF 拼 `(localized GetOtpError) + "\r\n" + serverMsg`；service layer 只回 server 原文，"Get OTP failed:" prefix 留給 UI（同 P4.1 `AmountLimitNotice` 的責任分離）
- **`OtpDecryptionFailed { cause: String }`**：把 `WcdesError::Display` 收進 `cause`，UI 拿到的是 typed error + 結構化 diagnostics（WPF 只給單一 `DecryptOTPError` 字串）
- **`step_3_record_start` / `step_4_long_poll` response 丟棄但仍檢 status**：WPF 在 non-2xx 會 throw 進外層 catch → `errmsg = "GetOtpError" + StackTrace`；我們用 `ensure_success` 把 non-2xx 包成 `LoginError::Unknown`，等價結果
- **`OtpMissingLongPollingKey { snippet }` 截斷至 256 chars**：WPF 把整個 response 塞進 errmsg；我們用 char-boundary-safe truncation 避免 multi-MB HTML 一直留在 error chain
- **`urlencoding` 不引入新 dep**：用 `percent-encoding`（`url` 的 transitive dep）的 `percent_decode_str`，行為與 .NET `Uri.UnescapeDataString` 等價（只解 `%XX`、`+` 視為 literal）
- **regex 用 `std::sync::OnceLock<Regex>`**：對齊 codebase 既有 convention（`core/parser/*` / `services/beanfun/login/*`），不引入 `once_cell` dep

#### Chunk 4.3 — `services/beanfun/verify.rs`
- [x] `get_verify_page_info(client, advance_check_url) -> VerifyPageInfo`：解 `LoginError::AdvanceCheckRequired` 後 caller 走的恢復路徑（接受 `Option<&str>`，None → 用 newlogin_base 預設 URL）
- [x] `get_verify_captcha(client, samplecaptcha) -> Vec<u8>`（PNG bytes，UI 層 base64 / data URL；< 500 bytes → `VerifyCaptchaImageTooSmall { actual }`）
- [x] `submit_verify(client, page_info, verify_code, captcha_code) -> VerifyOutcome`（4 variants：Success / ServerMessage(String) / WrongCaptcha / WrongAuthInfo）
- [x] WPF hardcoded TW domain（HK 雖會觸發 `LoginAdvanceCheck` errmsg 但 `BeanfunClient.advanceCheckUrl` 只在 TW 設置 + 三個 endpoint 全 hardcode TW host → silent dead path）→ `ensure_tw` 嚴格 region guard，HK 一律 `VerifyUnsupportedRegion`
- [x] 6 個 typed `LoginError` variants：`VerifyUnsupportedRegion` / `VerifyMissingViewState` / `VerifyMissingEventValidation` / `VerifyMissingSampleCaptcha` / `VerifyMissingLblAuthType` / `VerifyCaptchaImageTooSmall { actual }`
- [x] Pure helpers + parse / classify 全用 `OnceLock<Regex>` memoized，每個 helper 單獨 SRP，整體覆蓋 18 unit + 15 integration tests

##### Chunk 4.3 設計決議
- **HK 嚴格拒絕（不對齊 WPF dead path）**：WPF `BeanfunClient.Verify.cs` L23-25 / L43-45 / L90-92 + `MainWindow.xaml.cs::reLoadVerifyPage` L797-803 三處全 hardcode `tw.newlogin.beanfun.com`，但 HK regular / TOTP 路徑（`BeanfunClient.Login.cs` L249 / L361）仍會產生 `LoginAdvanceCheck` errmsg。WPF 的「HK + LoginAdvanceCheck」分支走的是會打 TW host 但 cookie 對不上的 silent dead path（無功能、無 UI 提示）。Rust port 改為早期 typed error `VerifyUnsupportedRegion`，UI 收到此錯誤直接導回登入頁，比 WPF 嚴格但功能等價且避免 silent fail
- **`advanceCheckUrl` 透過 `LoginError::AdvanceCheckRequired { url: Option<String> }` 傳遞**：WPF 把 `advanceCheckUrl` 放在 `BeanfunClient` instance field，違背我們 stateless `BeanfunClient` 的設計原則。複用既有 `LoginError::AdvanceCheckRequired` 的 `url` 欄位，由 caller（UI）保管並回傳給 `get_verify_page_info(client, Some(&url))`，符合 SRP（service 純函式 + caller 持狀態）
- **`bounded_bytes` 私有 helper 而非升上 `BeanfunClient`**：captcha 是整個 service surface 唯一回 bytes 的呼叫，升上 client 會誘導誤用。複用 `bounded_text` 的同款 chunk-cap 邏輯但去掉 UTF-8 驗證，私藏在 verify.rs 內部
- **重用 `extract_viewstate`（DRY）**：parse 直接用 `core::parser::viewstate::extract_viewstate`，再把 `event_validation: Option<None>` → typed `VerifyMissingEventValidation`。WPF 對 viewstate / event_validation 是 strict required、viewstate_generator optional，與既有 helper 的 `Option` 語意一致
- **`form_action` 解碼順序**：對應 WPF L800-802 的 `Replace("&amp;", "&")` + 顯式 prepend `https://tw.newlogin.beanfun.com/LoginCheck/`，缺 form action 時 fallback 到預設 URL（與 WPF L797 的 `if (regex.IsMatch(...))` 條件等價）
- **outcome classification 對齊 `verifyWorker_DoWork` L2634-2661**：先 `alert\\('(.*)'\\);` 抓出訊息 → 含 `資料已驗證成功` → `Success`；否則 → `ServerMessage(msg)`。無 alert 再看 `圖形驗證碼輸入錯誤` → `WrongCaptcha` / 否則 → `WrongAuthInfo`

#### Chunk 4.4 — `services/beanfun/account.rs` WebForms 管理 endpoints
- [ ] `unconnected_game_init_add_account_payload(...)`（含私有 `unconnected_game_init_account_payload` helper）
- [ ] `unconnected_game_add_account_check(...)` + `check_nickname(...)`（DRY 候選：兩個只差 `__EVENTTARGET`）
- [ ] `unconnected_game_add_account(...)`
- [ ] `unconnected_game_change_password(...)`（4-step flow）

##### 跨 chunk 設計決議
- **State model**：P4 函式統一 `(client: &BeanfunClient, session: &Session, ...)`，沿用 P3 的 split（`BeanfunClient` 只管 HTTP plumbing、`Session` 由 caller 持有）
- **`AmountLimitNotice` enum**：`None` / `AuthReLoginRequired`（偵測到「進階認證」）/ `Other(String 原文繁體)`。Service layer 不做 i18n / 簡繁轉換（WPF 的 `I18n.ToSimplified()` + `TryFindResource("AuthReLogin")` 都是 UI 層責任）
- **`AccountList.ApplyAccountOrder`（user-defined sort 持久化）**：P4.1 只做 ssn 排序（deterministic, matches WPF first-pass）；user-defined 順序留到 P5 storage / P6 commands；doc 在 `account.rs` 註明
- **OTP `Expect100Continue = false` 全域 mutation**：不移植。WPF 該行的最終結果是「不送 `Expect: 100-continue`」；reqwest 預設「最終結果」也是不送（且沒開關可以反過來開）→ 等價
- **OTP commented `Thread.Sleep` / `Console.WriteLine`**：dead code 不移植
- **OTP `ppppp=...` 64-char hex literal**：1:1 verbatim，doc 說明「protocol required, 來歷不明」
- **Accept-Encoding**：沿用 P3.x 慣例由 reqwest gzip/deflate features 自動處理。WPF 對 `Download/UploadString` 設 `identity`、對 `UploadStringGZip` 設 `gzip, deflate, br`；我們 wire 上會送 `gzip, deflate`（reqwest 預設）。語意等價（response body 內容相同），doc 註明 wire-level divergence

- **驗收**：15+ integration cases pass (P4.1-P4.4 合計)

### P5 — Rust `services/storage` DPAPI + `services/config` XML

- [ ] `services/storage/dpapi.rs`：`protect(plain: &[u8], entropy: &[u8]) -> Vec<u8>` / `unprotect(...)`（`CryptProtectData` / `CryptUnprotectData` + `CurrentUser` scope）
- [ ] `services/storage/entropy.rs`：`winreg` 讀寫 `HKCU\SOFTWARE\BEANFUN\Entropy`（格式與 WPF `ModifyRegistry` 相同）
- [ ] `services/storage/users_dat.rs`：
  - [ ] `save(records: &Records)`：serde_json → DPAPI protect → 寫 `%APPDATA%\Beanfun\Users.dat`
  - [ ] `load() -> Result<Records>`：讀檔 → unprotect → serde_json parse
  - [ ] `import(json: &str)` / `export() -> String`
- [ ] `services/config/xml.rs`：`quick-xml` 讀寫 `AppSettings` 格式；與 .NET `ExeConfigurationFileMap` 相容（`<appSettings><add key="..." value="..."/></appSettings>`）
- [ ] 損毀自動刪除重建（對齊 WPF `ConfigAppSettings` catch 行為）
- [ ] 互操作測試：
  - [ ] WPF 版寫的 `Users.dat` → Rust 讀，資料一致
  - [ ] Rust 版寫的 `Users.dat` → WPF 讀，資料一致（需另啟 WPF 驗證）
  - [ ] WPF 寫的 `Config.xml` → Rust 讀，所有 key 可取
- **驗收**：互操作測試全綠

### P6 — Rust `core/legacy` BinaryFormatter parser

- [ ] 實作 MS-NRBF 最小 parser（只需解 `AccountRecords` / `Records`）
- [ ] `core/legacy/nrbf.rs`：reader + record types（SerializedStreamHeader / ClassWithMembersAndTypes / ObjectNull / ArraySingleString / MemberReference / ...）
- [ ] `core/legacy/migrator.rs`：偵測舊格式 → parse → 轉為新 `Records`
- [ ] Fixture：`fixtures/legacy_users.dat`（用 WPF 版舊 code 產生）
- [ ] 單元測試：parse fixture → `Records` 內容正確
- [ ] 整合測試：storage 層發現舊格式時自動升級 + 立即儲存為 JSON 格式
- **驗收**：能 100% 相容讀取舊版 Users.dat；若 fixture 解析失敗立即停下討論（不得 workaround）

### P7 — Rust `services/updater` + GH proxy

- [ ] `services/updater/proxy_probe.rs`：對應 WPF `_cachedProxy` Lazy + `TryProbe` HEAD（5 秒 timeout）
- [ ] 代理清單常數：`ghproxy.vip` / `ghproxy.net` / `ghfast.top`
- [ ] `services/updater/github.rs`：fetch `api.github.com/repos/pungin/beanfun/releases`（加 `Beanfun(V{version})` UA）
- [ ] `services/updater/checker.rs`：`check_update(channel) -> Option<UpdateInfo>`（Stable/Beta 切換）
- [ ] `services/updater/parser.rs`：TagName `v{major}.{minor}.{patch}.{timestamp}` 解析
- [ ] Integration tests：
  - [ ] 直連成功 → 不用 proxy
  - [ ] 直連失敗 → fallback 到第一個 proxy
  - [ ] 前兩個 proxy 失敗 → 用第三個
  - [ ] 全部失敗 → 回空字串（靜默）
  - [ ] Stable / Beta channel
  - [ ] 版本格式變化（pre-5.8 舊格式、v5.8.13 timestamp 格式）
- **驗收**：8+ cases pass

### P8 — Rust `services/game` 啟動 + LR（SHA-256 安全升級）

- [ ] `services/game/launcher.rs`：
  - [ ] Normal 模式：`std::process::Command::new(path).arg(commandLine)`
  - [ ] 非 ASCII 路徑偵測 → 回傳 Error 訊息（對齊 WPF `MsgGamePathHaveWChar`）
- [ ] `services/game/locale_remulator.rs`：
  - [ ] 內嵌 5 個 LR 檔（`include_bytes!` for LRConfig.xml / LRHookx32.dll / LRHookx64.dll / LRProc.exe / LRSubMenus.dll）
  - [ ] build.rs：計算 LR 檔 SHA-256 並產生 `LR_SHA256: [(&str, [u8; 32]); 5]` 常數
  - [ ] 釋出流程：若目標檔存在→驗 SHA-256→不符合則刪除重建
  - [ ] `ShellExecuteW` + `runas` verb 提升權限啟動 `LRProc.exe`
  - [ ] GUID `ef3e7b42-a87c-4c07-ae3e-eeebeef12762`（與 WPF 相同）
- [ ] 單元測試：SHA-256 驗證邏輯（用測試 fixture DLL，故意改一 byte 必須被拒）
- [ ] 整合測試：釋出流程（用 `tempfile` 當目標目錄）
- **驗收**：SHA-256 拒絕被竄改 DLL、5 檔釋出與 WPF 行為等價

### P9 — Rust `services/process` + `services/registry`

- [ ] `services/registry/game_path.rs`：對齊 WPF `ModifyRegistry` + `HKCU/HKLM` 讀取 `dir_value_name`
- [ ] `services/process/find.rs`：WMI `Select * from Win32_Process where ProcessId = ?` 比對 `executablepath`
- [ ] `services/process/kill.rs`：kill by pid（`TerminateProcess`）
- [ ] `services/process/patcher.rs`：輪詢關 Patcher.exe（對齊 WPF `checkPatcher` 100ms interval）
- [ ] `services/process/play_page.rs`：輪詢關 PlayNowPage 視窗（對齊 WPF `checkPlayPage`）
- [ ] `services/process/post_string.rs`：`FindWindowW` + `PostMessageW(WM_CHAR)` 自動貼帳密
- [ ] Integration tests：spawn 假進程（`cmd /c timeout`）測試 find + kill
- **驗收**：功能對齊 WPF

### P10 — Tauri commands + IPC 型別

- [ ] `commands/auth.rs`：`login_regular` / `login_qr_start` / `login_qr_check` / `login_totp` / `login_gamepass_complete` / `logout` / `submit_verify` / `get_verify_captcha`
- [ ] `commands/account.rs`：`get_accounts` / `add_account` / `change_display_name` / `get_contract` / `get_email` / `get_remain_point` / `refresh`
- [ ] `commands/otp.rs`：`get_otp`
- [ ] `commands/launcher.rs`：`launch_game` / `set_game_path` / `detect_game_path` / `kill_game_processes` / `auto_paste`
- [ ] `commands/storage.rs`：`load_accounts` / `save_account` / `remove_account` / `import_records` / `export_records`
- [ ] `commands/config.rs`：`get_config` / `set_config`
- [ ] `commands/update.rs`：`check_update` / `open_url`
- [ ] `commands/system.rs`：`show_message` / `open_external` / `set_theme_color`
- [ ] 用 `specta` / `tauri-specta` 自動產 `bindings.d.ts`
- [ ] 單元測試：每個 command 至少一個 happy-path
- **驗收**：前端 `invoke("login_regular", {...})` 有型別提示、錯誤以 DTO 回傳

### P11 — Vue 前端：i18n / Pinia / 主題

- [ ] `scripts/convert-lang.mjs`：讀 `Beanfun/Lang/*.xaml` → 產生 `src/locales/*.json`（key 對齊 WPF 資源 key）
- [ ] 加入 `src/locales/zh-TW.json` / `zh-CN.json` / `en-US.json`
- [ ] vue-i18n 設定、設定頁切語系即時更新
- [ ] Element Plus 主題色：runtime 設定 `--el-color-primary`（配合 Settings 頁可換色）
- [ ] Pinia stores：
  - [ ] `auth`：login state / webtoken / region / method
  - [ ] `account`：service accounts / selected game / remain point / email
  - [ ] `config`：所有 Config.xml 對應設定
  - [ ] `ui`：theme color / minimize_to_tray / sw-render
- [ ] `services/invoke.ts`：型別安全的 `invoke` 薄包裝，統一錯誤處理
- [ ] `router/index.ts`：Pages 間導航
- [ ] 單元測試：每個 store 3+ cases
- **驗收**：主題 / 語系 / 設定存檔 / 重啟保留

### P12 — Vue 前端：所有 Pages + Windows 1:1

**Pages（11 個）**：
- [ ] `pages/LoginPage.vue`
- [ ] `pages/IdPassForm.vue`
- [ ] `pages/QrForm.vue`
- [ ] `pages/GamepassForm.vue`（用 Tauri `WebviewWindow` 開 GamePass 登入分頁）
- [ ] `pages/LoginTotp.vue`
- [ ] `pages/LoginWait.vue`
- [ ] `pages/VerifyPage.vue`（Captcha 圖 + 輸入）
- [ ] `pages/AccountList.vue`（含拖曳排序 / 右鍵選單）
- [ ] `pages/ManageAccount.vue`
- [ ] `pages/Settings.vue`
- [ ] `pages/About.vue`

**Windows / Dialogs（16 個）**：
- [ ] `windows/WebBrowser.vue`（會員中心 / 商城 / 客服用 Tauri Webview）
- [ ] `windows/AddAccount.vue` / `ChangeAccount.vue` / `AddServiceAccount.vue` / `ChangeServiceAccountDisplayName.vue`
- [ ] `windows/GameList.vue` / `LoginRegionSelection.vue` / `CaptchaWnd.vue` / `CopyBox.vue`
- [ ] `windows/Contract.vue` / `ServiceAccountInfo.vue` / `AccRecovery.vue`
- [ ] `windows/UnconnectedGame_AddAccount.vue` / `UnconnectedGame_ChangePassword.vue`
- [ ] `windows/MapleTools.vue` / `CoreCalculator.vue` / `EquipCalculator.vue`
- [ ] `windows/KartTools.vue`

**每個 Page/Window 驗收**：
- [ ] WPF XAML → Vue template 對應（結構 + 樣式）
- [ ] WPF code-behind → Pinia action + composable
- [ ] Vitest component test 3+ cases（render / prop / emit / store 整合）

- **驗收**：所有 11 + 16 = 27 個視圖跟 WPF 視覺 + 互動行為對齊；component tests 全綠

### P13 — E2E + Release

- [ ] 設定 `tauri-driver` + WebdriverIO
- [ ] `tests/e2e/` 測試案例：
  - [ ] Login (Regular TW) → 取 OTP → 啟動遊戲（mock beanfun + mock LR）
  - [ ] Login (Regular HK)
  - [ ] Login (QR)
  - [ ] Login (TOTP)
  - [ ] AdvanceCheck 驗證碼走完
  - [ ] 切換語系即時變化
  - [ ] 切換主題色即時變化
  - [ ] 帳號拖曳排序保存
  - [ ] 設定存檔 → 重啟保留
  - [ ] 更新檢查
  - [ ] 不連線遊戲新增帳號 / 改密
- [ ] `tauri build` 產 `.msi` + `.exe` installer
- [ ] `.github/workflows/beanfun-next-release.yml`：沿用 WPF 版 tag 格式 `v5.9.0.YYMMDDHHMM`，build 改為 `tauri build`
- [ ] Release notes 自動產生（沿用 WPF 版雙語腳本）
- **驗收**：CI 一鍵產 installer、E2E 全綠、installer 在乾淨 Win10/11 VM 安裝執行正常

---

## 風險與注意事項

- **DPAPI Entropy**：Entropy 字串必須用與 WPF 完全相同的「8 字隨機大寫+數字」格式，否則無法互讀
- **DES key**：`Encoding.ASCII` 對應 Rust `as_bytes()` 而非 `to_string().into_bytes()`；key 長度必為 8
- **LR SHA-256**：更新 LR 檔時必須同步更新 build-time 常數（寫在 `build.rs` 讓編譯時自動計算）
- **BinaryFormatter**：`.NET` BinaryFormatter 格式複雜，**若 fixture 解析失敗立即停下討論**（禁止 workaround，對應使用者規則 7）
- **Tauri v2 穩定性**：Tauri v2 於 2024 正式版後仍快速迭代，鎖定小版本
- **WebView2 相依**：Win10 初版需額外安裝 WebView2 Runtime，installer 要偵測並引導下載
- **Config.xml 相容**：quick-xml 寫入時必須保留 .NET `ExeConfigurationFileMap` 格式（含 `<configuration>` 根節點 + `<appSettings>`）
- **WMI 查詢**：`Win32_Process` 查詢需要 COM 初始化，Rust `wmi` crate 預設會處理但要確認 tokio runtime 相容

## 總體完成宣告條件

- [ ] 13 Phases 子任務全打勾
- [ ] `cargo test --workspace` 全綠
- [ ] `npm run test` 全綠（component）
- [ ] `npm run test:e2e` 全綠（tauri-driver）
- [ ] `tauri build` 產出的 installer 在乾淨 Win 10/11 可安裝並登入
- [ ] WPF 版已有的 `Users.dat` + `Config.xml` 能被新版直接讀取並繼續使用
- [ ] 與 WPF 版並排驗證：登入 → 取 OTP → 啟動楓之谷全流程等價

## 實作節奏約定（與使用者規則一致）

- 每個 Phase 開始前先 sync：列出該 Phase 內子任務清單，使用者 OK 再動手
- 每個 Phase 完成後：跑測試 + 回報 diff + 請使用者驗收才 commit
- 任何解不開的問題：停下討論，不擅自 workaround
- 實作以 SRP / DRY 為基本原則
- Commit message 遵循 Conventional Commits，**嚴禁 `Co-authored-by: cursor`**

---

## P-1 — UI Mockups 切版（Stitch 同風格）

> 檔案統一放 `beanfun-next/mockups/`。共用 glassmorphism + Fluent + MD3 token。
> 8 個主題色 runtime 可換：Orange（預設）/ Green / LightBlue / Pink / Gold / Silver / Black / White + 自訂 hex。

### 共用資源
- [x] `_design-system.html` — 8 色完整 palette + glass-panel / fluent-input / btn-gradient / reveal-highlight utility preview

### Pages（7 未切，5 已由 Stitch 完成）
**已由 Stitch 完成（視覺由 Stitch 提供）**
- [x] `IdPassForm` / `AccountList` / `QrForm` / `GameList`（以 dialog 切法）/ `Settings`

**本輪自行補齊**
- [x] `LoginRegionSelection.html`
- [x] `LoginWait.html`
- [x] `LoginTotp.html`
- [x] `GamepassForm.html`
- [x] `VerifyPage.html`
- [x] `About.html`
- [x] `ManageAccount.html`

### Dialogs / Windows（17 檔）
- [x] `AddAccount.html`
- [x] `ChangeAccount.html`
- [x] `AddServiceAccount.html`
- [x] `ChangeServiceAccountDisplayName.html`
- [x] `CopyBox.html`
- [x] `Contract.html`
- [x] `ServiceAccountInfo.html`
- [x] `CaptchaWnd.html`
- [x] `AccRecovery.html`
- [x] `WebBrowser.html`
- [x] `UnconnectedGame_AddAccount.html`
- [x] `UnconnectedGame_ChangePassword.html`
- [x] `MapleTools.html`
- [x] `KartTools.html`
- [x] `CoreCalculator.html`
- [x] `EquipCalculator.html`
- [x] `GameList.html`（獨立檔，跟 Stitch 的 dialog 版並存）

**驗收**：所有 mockup 檔在瀏覽器開啟能呈現、字型/glass-panel/gradient button 齊備、8 色切換 preview 正常。
