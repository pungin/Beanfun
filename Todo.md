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
- [x] WPF hardcoded TW domain → 不做 region guard，HK 也走同一個 flow（透過 `Endpoints::hk().newlogin_base = TW newlogin host` invariant 自動 routing）
- [x] 5 個 typed `LoginError` variants：`VerifyMissingViewState` / `VerifyMissingEventValidation` / `VerifyMissingSampleCaptcha` / `VerifyMissingLblAuthType` / `VerifyCaptchaImageTooSmall { actual }`
- [x] Pure helpers + parse / classify 全用 `OnceLock<Regex>` memoized，每個 helper 單獨 SRP，整體覆蓋 17 unit + 13 integration tests

##### Chunk 4.3 設計決議
- **HK 對齊 WPF 1:1（不做 region guard）**：WPF `BeanfunClient.Verify.cs` L23-25 / L43-45 / L90-92 + `MainWindow.xaml.cs::reLoadVerifyPage` L797-803 三處全 hardcode `tw.newlogin.beanfun.com`，且 HK regular / TOTP 路徑（`BeanfunClient.Login.cs` L249 / L361）會在 server 回應含 `RELOAD_CAPTCHA_CODE` + `alert` 時產生 `LoginAdvanceCheck`。這是 server 預期的恢復路徑（server 主動發訊號要求 client 走 verify），不是 dead branch。Rust port 不加 region guard，HK 也走同一個 flow，URL 透過既有 `Endpoints::hk().newlogin_base = TW newlogin host` invariant 自動指向 TW，與 WPF byte-for-byte 等價。HK session cookie 能否被 TW host 接受由 server 決定；若 server 拒絕，回傳的 HTML 缺欄位 → 自動走 `VerifyMissing*` typed error（與 WPF 的 `VerifyNoViewstate` / `VerifyNoEventvalidation` 等價）。region invariance 由 unit test `url_helpers_target_tw_newlogin_host_even_for_hk_client` 鎖定
- **`advanceCheckUrl` 透過 `LoginError::AdvanceCheckRequired { url: Option<String> }` 傳遞**：WPF 把 `advanceCheckUrl` 放在 `BeanfunClient` instance field，違背我們 stateless `BeanfunClient` 的設計原則。複用既有 `LoginError::AdvanceCheckRequired` 的 `url` 欄位，由 caller（UI）保管並回傳給 `get_verify_page_info(client, Some(&url))`。HK 路徑不 set `url` → 傳 `None` → fallback 到預設 TW URL，與 WPF L23-25 行為等價
- **`bounded_bytes` 私有 helper 而非升上 `BeanfunClient`**：captcha 是整個 service surface 唯一回 bytes 的呼叫，升上 client 會誘導誤用。複用 `bounded_text` 的同款 chunk-cap 邏輯但去掉 UTF-8 驗證，私藏在 verify.rs 內部
- **重用 `extract_viewstate`（DRY）**：parse 直接用 `core::parser::viewstate::extract_viewstate`，再把 `event_validation: Option<None>` → typed `VerifyMissingEventValidation`。WPF 對 viewstate / event_validation 是 strict required、viewstate_generator optional，與既有 helper 的 `Option` 語意一致
- **`form_action` 解碼順序**：對應 WPF L800-802 的 `Replace("&amp;", "&")` + 顯式 prepend `https://tw.newlogin.beanfun.com/LoginCheck/`，缺 form action 時 fallback 到預設 URL（與 WPF L797 的 `if (regex.IsMatch(...))` 條件等價）
- **outcome classification 對齊 `verifyWorker_DoWork` L2634-2661**：先 `alert\\('(.*)'\\);` 抓出訊息 → 含 `資料已驗證成功` → `Success`；否則 → `ServerMessage(msg)`。無 alert 再看 `圖形驗證碼輸入錯誤` → `WrongCaptcha` / 否則 → `WrongAuthInfo`

#### Chunk 4.4 — `services/beanfun/account.rs` WebForms 管理 endpoints
- [x] D-step 1：error.rs 加 5 個 `AccountMgmtMissing*` typed variants（ViewState / ViewStateGenerator / EventValidation / GameName / AccountLen）
- [x] D-step 2：account.rs 加 5 個 public types（`AddAccountSession` / `AddAccountInit` / `CheckOutcome` / `AddAccountOutcome` / `ChangePasswordOutcome`）
- [x] D-step 3：account.rs 加 private helpers（`mgmt_url` / `change_password_url` / `parse_viewstate_triplet` / `build_viewstate_payload_prefix` / `push_account_dn` / `add_account_check_inner` / `build_add_account_form` / `extract_lbl_error_message` / `extract_verify_code_from_url` + `init_account_payload`）
- [x] D-step 4：實作 `unconnected_game_init_add_account_payload(...)`（含內部 `init_account_payload` helper：GET `auth.aspx?channel=accounts_management...`）
- [x] D-step 5：實作 `unconnected_game_add_account_check(...)` + `unconnected_game_add_account_check_nickname(...)`（共用 `add_account_check_inner`）
- [x] D-step 6：實作 `unconnected_game_add_account(...)`
- [x] D-step 7：實作 `unconnected_game_change_password(...)`（5-step flow + HK `http://` deviation candidate doc）
- [x] D-step 8：mod.rs re-exports 更新
- [x] D-step 9：20 unit tests（pure helpers + region URL prefix + HK `__VIEWSTATEENCRYPTED` toggle + 5 missing-field errors + outcome classification + verify_code extraction）
- [x] D-step 10：15 integration tests in `tests/account_management.rs`（init TW/HK + 3 missing-field errors + check TW/HK + check_nickname + add success/error/empty + change_password 5-step + lblErrorMessage + Unknown outcome）
- [x] D-step 11：quality gates（fmt / clippy / test 全綠 — 237 lib unit + 13 integration binaries 0 failed / doc 0 warning）
- [x] D-step 12：Todo.md 標記完成 + P4.4 設計決議段落
- [ ] D-step 13：single commit `feat(next): add WebForms account-management endpoints (P4 chunk 4.4)`

##### Chunk 4.4 設計決議（事先記錄，實作後若有調整再 update）
- **D1 → A2 結構化 typed types**：`AddAccountSession` 持 viewstate 三件組 + region；`AddAccountInit` 含 session + game_name + account_len + check_nickname_supported；`CheckOutcome { session, error_message }`；`AddAccountOutcome { Success | ErrorMessage(String) }`；`ChangePasswordOutcome { VerifyCodeSent(String) | ErrorMessage(String) }`。caller 不會誤塞欄位，HK `__VIEWSTATEENCRYPTED` 由 service 內部處理
- **D2 → B1 private helper**：`accounts_management_url(client, suffix)` 在 account.rs 內，不擴張 BeanfunClient surface
- **D3 → C3 1:1 用 `http://` + doc**：HK `change_password` step 3/4 對齊 WPF L549-555/L597-600 用 `http://`（其餘所有 HK 路徑都是 https）。看似 typo 但功能對齊優先；module doc 加 `# WPF deviation candidate` 段落留 trace 給 P10 安全 review
- **D4 → E1 5 typed variants**：對齊 verify chunk 的 `VerifyMissing*` 命名 pattern。未來重構成通用 `MissingHiddenField` 留給 P10
- **D5 → F1 兩 public + 一 private inner**：public surface 對齊 WPF caller，內部共用 `add_account_check_inner(client, mgmt_session, event_target, account_id, dn)`

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

#### Chunk 5.1 — `services/storage/dpapi.rs` + `services/storage/entropy.rs`（底層 primitive）
- [x] D-step 1：新增 `services/storage/mod.rs` + `StorageError` error enum（4 variants：`Dpapi { operation, message }` / `Registry(io::Error)` / `EntropyMissing` / `EntropyShape`；DPAPI protect / unprotect 共用單一 variant 用 `operation` 欄位辨識）
- [x] D-step 2：`services/storage/dpapi.rs` 實作 `dpapi_protect(plain, entropy) -> Vec<u8>` / `dpapi_unprotect(cipher, entropy) -> Vec<u8>`（`windows` crate `CryptProtectData` / `CryptUnprotectData`，`CurrentUser` scope，無 flags / description，`LocalFree` 釋放 Win32 allocated buffer）
- [x] D-step 3：`services/storage/entropy.rs` 實作 `Entropy(String)` newtype + `generate()`（`OsRng` 8 char `[A-Z0-9]` CHARSET 36 char）/ `parse()` / `as_bytes()` / `as_str()` + `read_from_registry()` / `write_to_registry()`（`winreg` 讀寫 `HKCU\SOFTWARE\BEANFUN\ENTROPY`，key / value 大寫 hardcode）+ `_at(subkey, value_name)` 變體供測試隔離用
- [x] D-step 4：lib re-exports（`mod.rs` pub use）+ `services/mod.rs` 加 `pub mod storage;`
- [x] D-step 5：15 unit tests（entropy: 10 tests 含 generate 3 / parse 4 / debug redacted / registry constants / charset 常數對齊；dpapi: 5 tests 含 round-trip / wrong entropy / empty / large / no-entropy）
- [x] D-step 6：7 integration tests in `tests/storage_dpapi.rs`（end-to-end save/load、EntropyMissing sub-key 缺、EntropyMissing value 缺、EntropyShape 畸形值、大 payload 256KB、entropy 篡改失敗、精確值 round-trip；registry 用 `SOFTWARE\BEANFUN_NEXT_TEST\<name>_<pid>` 隔離不污染 production）
- [x] D-step 7：quality gates（fmt / clippy `-D warnings` / test `252 lib + 7 integration 0 failed` / doc 0 warning 全綠）
- [ ] D-step 8：commit `feat(next): add DPAPI + entropy storage primitives (P5 chunk 5.1)`

#### Chunk 5.2 — `services/storage/users_dat.rs`（Records + JSON save/load + legacy hook）

##### 校準後的設計決議（vs WPF `AccountManager.cs`）

- **A — `import_records` 走 IO 對齊 WPF**：WPF `importRecord` 是 user-facing 入口、contract 含「import 完馬上覆寫檔案」。`import_records(path, json)` 內部串 parse → normalize → save → return；額外提供 `parse_records(json)` 純 parser、`export_records(records)` 純 serializer
- **B — `StorageError` 簡化 3 個 variant**：對齊 WPF L226-229 catch-all → DPAPI / registry / UTF-8 / JSON parse 失敗都 swallow + 刪檔 + 回 `Ok(Records::default())` 不對外 propagate；對外只新增 `Io` / `JsonSerialize` / `LegacyDataDetected { raw_bytes }`
- **C — `LegacyDataDetected` 維持 typed Err（caller 處理 fallback）**：P5 階段沒 NRBF parser，現在引入 trait 會 dangle；module doc 明確規範「caller 收到後若 NRBF parse 失敗 → 回空 records 不刪檔」對齊 WPF L494-550；P6 上線後若需內聚 fallback 再評估加 wrapper API
- **D — path 用 `std::env::var_os("APPDATA")`**：不加 `dirs` dep，直接對齊 WPF `SpecialFolder.ApplicationData`；`default_users_dat_path()` 用 `#[cfg(target_os = "windows")]` gate
- **Wire format**：`WireRecords` 7 欄位全用 `Option<Vec<...>>`（兼容 WPF write 的 null fields）+ `#[serde(rename)]` 對齊 C# camelCase（含 `passwdList`）；不對外暴露
- **normalize 等價 WPF `accRecInit()`**：region→`"TW"`、其他→`""`/`0`/`false`、補齊到 `account_list.len()`；內聚到 `WireRecords::normalize()`

##### D-steps

- [x] D-step 1：public types — `Account` struct（7 欄位：region / account_id / account_name / password / verify / method / auto_login）+ `Records(Vec<Account>)` + `Default` impl 空列表
- [x] D-step 2：wire format adapter — `WireRecords`（7 個 `Option<Vec<...>>` 對齊 WPF camelCase + `#[serde(rename)]`）+ `From<&Records>` / `From<WireRecords>`（含 normalize）
- [x] D-step 3：normalize 內聚到 `WireRecords::normalize()`（region 缺省 `"TW"`、其他 list 缺省 `""` / `0` / `false`、length 對齊 `account_list.len()`、`None` 補空 Vec）
- [x] D-step 4：新增 `StorageError::{Io, Json, LegacyDataDetected { raw_bytes }}` 3 個 variants（`Json` 涵蓋 serialize + deserialize 兩路徑）
- [x] D-step 5：`save_records(path, &Records)` async（+ `save_records_at` test variant 注入 entropy subkey）— `spawn_blocking`(records → WireRecords → normalize → JSON serialize → `Entropy::generate()` + `write_to_registry_at()` → `dpapi_protect()` → mkdir_p parent → `std::fs::write()` `FileMode.Create` 等價)
- [x] D-step 6：`load_records(path)` async（+ `load_records_at` test variant）— `spawn_blocking`:
  - [x] file 不存在 → `Ok(Records::default())`（不刪檔）
  - [x] `std::fs::read` 失敗 → `Err(StorageError::Io)`
  - [x] `read_from_registry_at` / `dpapi_unprotect` / UTF-8 decode 任一失敗 → log warn + `std::fs::remove_file` + `Ok(Records::default())` 對齊 WPF L226-229
  - [x] `serde_json::from_str::<WireRecords>(plain)` 成功 → `WireRecords::normalize()` → `Records` → `Ok(Records)`
  - [x] JSON parse 失敗 → 試 `BASE64.decode(plain)`：成功 → `Err(LegacyDataDetected { raw_bytes })` / 失敗 → log warn + 不刪檔 + `Ok(Records::default())` 對齊 WPF
- [x] D-step 7：`parse_records(json)` 純 parser / `export_records(&Records) -> Result<String, _>` 純 serializer / `import_records(path, json)` async（+ `import_records_at` test variant）對齊 WPF `importRecord`（parse → save → return；JSON fail + base64 OK → `Err(LegacyDataDetected)`；JSON + base64 皆 fail → `Err(Json)` 讓 user 看到錯誤）
- [x] D-step 8：`default_users_dat_path() -> Result<PathBuf, StorageError>` Windows-only helper（`%APPDATA%\Beanfun\Users.dat`）
- [x] D-step 9：lib re-exports（`mod.rs` pub use `Account` / `Records` / pure parsers cross-platform；IO-bearing async + `_at` 變體 + `default_users_dat_path` Windows-only）+ module doc（fallback 規範清楚寫在 `users_dat` doc）
- [x] D-step 10：15 unit tests（Account default 1 / Records default 1 / WireRecords round-trip 2 / normalize 4 / parse_records 4 / export_records 3）
- [x] D-step 11：13 integration tests in `tests/storage_users_dat.rs`（save/load round-trip / save mkdir_p parent / file 不存在 / 篡改 entropy 刪檔 / 刪 registry entropy 刪檔 / UTF-8 損壞刪檔 / valid base64 非 JSON → `LegacyDataDetected` 不刪檔 / 純垃圾 → 不刪檔回空 / `import_records` JSON 寫檔成功 / `import_records` base64 → `LegacyDataDetected` 不寫檔 / `import_records` 純垃圾 → `Json` 不寫檔 / `export_records` → `import_records` round-trip / `default_users_dat_path` 解析；registry 用 `SOFTWARE\BEANFUN_NEXT_TEST\users_<name>_<pid>` 隔離不污染 production）
- [x] D-step 12：quality gates（fmt / clippy `-D warnings` / test `267 lib + 13 storage_users_dat + 7 storage_dpapi + 其他 0 failed` / doc 0 warning 全綠）
- [x] D-step 13：commit `feat(next): add Users.dat JSON + DPAPI storage (P5 chunk 5.2)`

#### Chunk 5.3 — `services/config/xml.rs`（AppSettings XML 讀寫 + 損毀重建）

##### 校準後的設計決議（vs WPF `ConfigAppSettings.cs`）

- **A — Map type 用 `IndexMap`**：保 insertion order 與 .NET `ConfigurationManager` 完全對齊，WPF 寫的 `Config.xml` 讀進來改 value 不打亂順序、新 key append 到尾巴。新增 1 個 dep `indexmap = "2"` 換 byte-byte 相容
- **B — `get_value` 對齊 WPF catch-all**：`async fn get_value(path, key) -> String`（內部呼 `get_value_or(path, key, "")`）/ `async fn get_value_or(path, key, default) -> String`；任何失敗（Io / XmlParse / UTF-8）→ log warn + 回 default。對齊 WPF L88-91 try/catch 行為
- **C — `set_value` 用 typed Result（deviation from WPF）**：`async fn set_value(path, key, value: Option<&str>) -> Result<(), ConfigError>`；read 失敗會內聚刪檔重試一次（行為對 user 看起來與 WPF 等價）；write 失敗（disk full / perm denied）surface 為 `Err(ConfigError::Io)` 不像 WPF L60 空 `catch{}` swallow。Module doc 明確標註此 deviation 並記錄理由（WPF 靜默失敗是 anti-pattern，typed error 讓 P10 上層 service 可決定 UX）
- **D — 損毀重建內聚到 1 次 flow**：WPF L36-61 是 outer try/catch + 遞迴 retry；Rust 內聚到 `set_value` flow 內：file 不存在 → 空 IndexMap；read 或 parse 失敗 → log warn + `std::fs::remove_file`（best-effort）+ 空 IndexMap → 繼續 modify + write。不需要 outer retry counter
- **E — XML schema 完全對齊 .NET ConfigurationManager**：`<?xml version="1.0" encoding="utf-8"?>` + `<configuration>` → `<appSettings>` → `<add key="..." value="..."/>` (self-closing)；escape `<` `>` `&` `"` `'` 由 quick-xml 處理；read 時忽略 unknown element / attribute；write 時 drop unknown（對齊 .NET 行為）；縮排與行尾用 quick-xml 預設（2-space LF）WPF 仍可讀
- **F — API 不需要 `_at` 變體**：沒摸 registry，caller 直接傳 `path` 已經夠 test 隔離
- **G — `ConfigError` 4 個 variant**：`Io(std::io::Error)` / `XmlParse(quick_xml::Error)` / `XmlWrite(quick_xml::Error)` / `AppDataMissing`（Windows-only `default_config_xml_path` 用）
- **H — Path source**：`std::env::var_os("APPDATA")` + `\Beanfun\Config.xml` 對齊 P5.2 風格不加新 dep；`default_config_xml_path()` Windows-only

##### Crate 依賴

- `indexmap = "2"`（新增）
- `quick-xml = "0.37"` with `serialize` feature（已有）

##### D-steps

- [x] D-step 1：`services/config/mod.rs` + `ConfigError` 4 variants（`Io` / `XmlParse` / `XmlWrite` / `AppDataMissing`）
- [x] D-step 2：`Cargo.toml` 加入 `indexmap = "2"`
- [x] D-step 3：`services/config/xml.rs` `parse_app_settings(xml: &str) -> Result<IndexMap<String, String>, ConfigError>`（quick-xml reader，跳過 unknown element / 容錯 declaration / 嚴格只挑 `<configuration><appSettings><add>` 路徑）
- [x] D-step 4：`serialize_app_settings(map: &IndexMap<String, String>) -> Result<String, ConfigError>`（quick-xml writer，固定 schema + XML declaration + escape；空 map 走 self-closing `<appSettings/>` 對齊 .NET output）
- [x] D-step 5：`get_value_or(path, key, default) -> String` async / `get_value(path, key) -> String` async（內部呼 `get_value_or` + ""）— catch-all + log warn 對齊 WPF
- [x] D-step 6：`set_value(path, key, value: Option<&str>) -> Result<(), ConfigError>` async — `tokio::task::spawn_blocking`：file 不存在 → 空 IndexMap；read 或 parse 失敗 → log warn + `remove_file` + 空 IndexMap；modify map（`IndexMap::insert` 統一處理 Add/Update 保持 slot；`shift_remove` 處理 Remove；no-op 跳過寫檔對齊 WPF L21-25）→ `serialize_app_settings` → mkdir_p parent → `std::fs::write`；write 失敗 surface
- [x] D-step 7：`default_config_xml_path() -> Result<PathBuf, ConfigError>` Windows-only helper（`%APPDATA%\Beanfun\Config.xml`）
- [x] D-step 8：`services/config/mod.rs` re-exports（`parse_app_settings` / `serialize_app_settings` / `get_value` / `get_value_or` / `set_value` cross-platform；`default_config_xml_path` Windows-only）+ module doc（含 set_value typed-error deviation 記錄）+ `services/mod.rs` 掛 `pub mod config;`
- [x] D-step 9：11 unit tests（cross-platform）— `parse_app_settings` 6（WPF fixture / 空 appSettings / unknown element 跳過 / escape `<>&"'` decode / malformed XML / insertion order preserved）+ `serialize_app_settings` 4（empty self-closing wire format / round-trip / escape encode / insertion order）+ `ConfigError` Display 1
- [x] D-step 10：11 integration tests in `tests/config_xml.rs`（cross-platform）— missing file `get_value` 回 default 且不建檔 / missing file `set_value` 自動建檔 + mkdir_p parent / set then get round-trip / `set_value(key, None)` remove key / `set_value(non_existent_key, None)` no-op 不建檔 / 損毀檔案 `set_value` 內聚刪檔重建成功 / 損毀檔案 `get_value` 回 default 不刪檔 / update existing key 保 insertion order / WPF fixture 透過 `set_value` round-trip / `serialize → parse` arbitrary map 含 escape / `default_config_xml_path` Windows-only 解析
- [x] D-step 11：quality gates（fmt / clippy `-D warnings` / test `278 lib + 11 config_xml + 13 storage_users_dat + 7 storage_dpapi + 其他 共 447 passed 0 failed` / doc 0 warning 全綠）
- [x] D-step 12：commit `feat(next): add AppSettings XML config store (P5 chunk 5.3)`

#### Chunk 5.x 設計決議（事前記錄，實作後若有調整再 update）

##### 共用決議
- **Records API shape**：Rust 內部用 `Vec<Account>` struct，serde adapter 讓 wire 繼續是 parallel columns JSON（與 WPF byte-byte 相容）。`WireRecords` 是內部 helper 不對外暴露，確保 round-trip invariance
- **async API + 內部 `spawn_blocking`**：storage / config 兩層都是 `async fn` 對齊 P4 風格，內部用 `tokio::fs` 或 `tokio::task::spawn_blocking` 包同步 Win32 API（DPAPI / registry / file I/O）
- **`Entropy` 升級到 `OsRng`**：WPF 用 `new Random()` time-seeded PRNG 是原有瑕疵；DPAPI ciphertext 本身已經有強熵，entropy 只是 salt，升級 RNG 不影響互通性，每次 save 重新生成行為不變
- **Legacy BinaryFormatter fallback 留 P6 接手**：P5 的 load 流程在 `serde_json::from_str` 失敗 + `base64::decode` 成功時，回 typed `StorageError::LegacyDataDetected { raw_bytes }`，P6 `core/legacy/nrbf.rs` 接管 parser 並走相同 save 路徑覆寫成 JSON
- **Registry sub-key hardcode `"BEANFUN"`**：WPF 用 `Application.ResourceAssembly.GetName().Name.ToUpper()`，我們 Rust `beanfun-next` crate 名不同但對 external WPF byte-byte 相容需要 hardcode；hardcode 在 `entropy.rs` 常數 + module doc 註明來歷
- **Config XML 損毀重建限 1 次重試**：WPF L58 遞迴呼叫 `SetValue` 沒有終止條件，理論上無限遞迴（實務上第二次一定成功因為剛刪了檔）；Rust 嚴謹限 1 次，第二次失敗直接回 `ConfigError`，差異寫進 module doc
- **File paths 由 caller 傳入**：`load_records(path: &Path)` / `get_value(path: &Path, key)` 接受 path 參數方便測試；另外提供 `default_users_dat_path()` / `default_config_xml_path()` helper 給 production caller 使用（內部用 `dirs::config_dir()` 或 `std::env::var("APPDATA")` 對齊 WPF `SpecialFolder.ApplicationData`）
- **Thread-safety**：P5 不加 lock；caller（P10 Tauri commands）若需要序列化多路 save 呼叫，由上層用 `tokio::sync::Mutex` 包裝。storage 函式本身 stateless（每次 open file）

##### Crate 依賴新增（`Cargo.toml`）
- `windows` (0.5x) with features `["Win32_Security_Cryptography", "Win32_Foundation"]` — DPAPI
- `winreg` — registry
- `quick-xml` — config XML parser/writer
- `base64` — legacy 偵測用 base64 decode 嘗試
- `rand` (如果尚未引入) + `rand::rngs::OsRng` / `rand::distributions::Alphanumeric` — entropy 產生
- 所有新依賴放 `[target.'cfg(windows)'.dependencies]` 若 platform-gated，但我們 beanfun-next 本來就 Windows-only（Tauri app target）→ 可直接放 `[dependencies]`

##### 驗收條件
- **Chunk 5.1**：DPAPI protect / unprotect round-trip OK；registry entropy 讀寫 OK；`OsRng` 產生的 entropy 每次呼叫都不同
- **Chunk 5.2**：save → load round-trip 欄位 byte-byte 相同；normalize 補齊短 list 與 WPF `accRecInit` 等價；base64 legacy 觸發 typed error；DPAPI 失敗觸發刪檔行為
- **Chunk 5.3**：16 個已知 key + default 的 get/set 行為全 OK；`set_value(key, None)` 真的移除該節點；損毀檔案觸發刪除 + 重試一次成功；remaining WPF-written XML fixture 可以 parse
- **P5 總驗收**：約 33 個 unit tests + 25 個 integration tests 全綠，quality gates 全綠
- [x] P5 全章節 post-implementation review — 對齊 WPF `AccountManager.cs` / `ModifyRegistry.cs` / `ConfigAppSettings.cs`，整體功能 1:1，找到 3 項 polish 已 commit `bbd5f85`（F2 save 對 registry write failure 比 WPF 嚴格的 deviation doc / F5 `load_records_blocking` 把 NotFound 當 file missing 省 syscall + 防 TOCTOU / F6 `StorageError::AppDataMissing` variant 與 `ConfigError::AppDataMissing` 對齊 API shape）

### P6 — Rust `core/legacy` NRBF parser + `services/storage/legacy` migrator

#### Chunk 6.x 共用決議（vs WPF `AccountManager.TryAutoMigrateLegacyData` L494-551）

- **A — Parser 策略**：用 `nrbf = "0.2"` crate（MIT OR Apache-2.0）解低層 MS-NRBF binary → `Value` enum；自寫 thin adapter `Value → LegacyPayload → Records`。Crate 處理 binary spec（record types、string encoding、length prefix、nom 8 parser combinator），我們負責 Beanfun 專用的 class shape semantic mapping。最小攻擊面（不 hand-roll spec parser）+ 不需要 WPF 環境
- **B — Module 位置**：`core::legacy::nrbf`（pure, framework-agnostic, 產 `LegacyPayload` pure domain model）+ `services::storage::legacy`（IO-bound migrator，呼 `save_records` 覆寫 JSON）；`core` 不 depend on `services`，`LegacyPayload` 與 `Records` 解耦
- **C — Error shape**：`NrbfError`（core 層，parse 錯誤）+ `LegacyMigrateError`（services 層，`Nrbf` / `Storage` variants）；不污染 `StorageError` enum，SRP 分層
- **D — Records.Change 對齊策略**：`LegacyPayload` enum = `Records(LegacyRecords) | AccountRecords(LegacyAccountRecords)`；映射到 `WireRecords` 讓 `AccountRecords` 缺 `accountNameList` 自動走 `None` → `WireRecords::normalize()` 補 `""`。對齊 WPF JSON-as-bridge 的 **結果**（null field → empty list），但跳過雙重 JSON round-trip；複用 P5 `WireRecords::normalize` DRY
- **E — Auto-save**：`migrate_and_save` 內部呼 `save_records`，對齊 WPF L526 `storeRecord()` 立刻覆寫 JSON 格式；UI 層不用知道舊格式存在
- **F — load 層 wrapper**：新 API `load_records_with_legacy_migration(path)`；P5 既有 `load_records` 保持不動（P5 typed error contract / test 不破壞）。P10 Tauri command 選用 wrapper
- **G — Fixture 來源**：全手刻 NRBF bytes（依 MS-[MS-NRBF] spec），每段 bytes const 搭 docstring 標 record type + spec 章節；完全可控 + 不需要 .NET 環境 + edge case 可手造
- **H — MessageBox 不移植**：WPF L536 成功時彈 `LegacyDataMigrateSuccess` MessageBox；service layer 一律不觸 UI，改用 `tracing::info!`；通知 UI 留給 P10/P11

##### Crate 依賴新增（`Cargo.toml`）
- `nrbf = "0.2"`（MIT OR Apache-2.0，transitive: `nom` 8 / `bitflags` 2 / `rust_decimal` 1）

##### 驗收條件
- **Chunk 6.1**：手刻 NRBF bytes fixtures 全數 parse 通過；WPF legacy 6 欄位 `AccountRecords` + new 7 欄位 `Records` 兩種 class 都能正確分派並抽出；edge case（null list / `_size` < `_items.len()` / unknown class / malformed header）走對應 typed error
- **Chunk 6.2**：`LegacyPayload → Records` 轉換對齊 `accRecInit` 結果；`migrate_and_save` 成功後磁碟上 Users.dat 是 JSON 格式且 round-trip 可讀；`load_records_with_legacy_migration` 對合法 legacy file 自動升級；對 migrate 失敗的 legacy file 對齊 WPF L546-548 回空 records 不刪檔
- **P6 總驗收**：能 100% 相容讀取舊版 Users.dat；若 fixture 解析失敗立即停下討論（不得 workaround）

#### Chunk 6.1 — `core/legacy/nrbf.rs`（NRBF → `LegacyPayload`，pure）

- [x] D-step 1：`Cargo.toml` 加 `nrbf = "0.2"`
- [x] D-step 2：`core/legacy/{mod.rs, error.rs, nrbf.rs}` scaffold + `core/mod.rs` 掛 `pub mod legacy;`
- [x] D-step 3：`NrbfError` 5 variants（`Internal(String)` / `UnsupportedClass { name }` / `MissingMember { class, member }` / `TypeMismatch { class, member, expected }` / `InconsistentListSize { class, member, size, items }`）— `Internal` 用 `String` 而非 `#[from] nrbf::Error<'i>`，避開 borrowed lifetime 跨 owned error 的問題（見 `error.rs` doc）
- [x] D-step 4：pure domain types — `LegacyRecords`（7 欄位 Vec：region / account / account_name / passwd / verify / method / auto_login）+ `LegacyAccountRecords`（6 欄位，**無** `account_name_list`）
- [x] D-step 5：`LegacyPayload` enum（`Records(LegacyRecords) | AccountRecords(LegacyAccountRecords)`）
- [x] D-step 6：`parse_legacy_payload(bytes: &[u8]) -> Result<LegacyPayload, NrbfError>` — 用 `nrbf::RemotingMessage::parse`（非 serde 版本，避開 crate 對 `List<T>` 寫死 3-member 的假設）；match root `Value::Object` class name 分派到 `parse_records` / `parse_account_records`
- [x] D-step 7：extract helpers — `extract_list_of_strings` / `extract_list_of_i32` / `extract_list_of_bool` 共用 `extract_list<T>` generic；統一處理 `null list → empty vec` / `null item → T::default`（對齊 WPF JSON round-trip 結果）/ `_size > items.len()` → `InconsistentListSize` / `_size < items.len()` 取前 `_size` slots
- [x] D-step 8：module doc 含 WPF `TryAutoMigrateLegacyData` 行號對應表（L501-503 / L506-512 / L513-521 / L526 / L536 / L546-548）+ `null → empty` 的 WPF JSON-bridge 邏輯說明 + 為何 refuse arbitrary root classes（NRBF security posture）+ 為何不用 crate 的 serde feature（`List<T>` 3-member 寫死）；re-exports 到 `core::legacy::{NrbfError, LegacyPayload, LegacyRecords, LegacyAccountRecords, parse_legacy_payload}`
- [x] D-step 9：11 unit tests with hand-crafted NRBF byte fixtures — `parse_records_all_null_lists` / `parse_records_two_accounts` / `parse_records_empty_lists` / `parse_records_string_list_with_null_element_maps_to_empty_string` / `parse_records_takes_first_size_elements_when_items_longer` / `parse_records_size_greater_than_items_returns_inconsistent` / `parse_account_records_six_fields` / `parse_unknown_class_returns_unsupported` / `parse_malformed_header_returns_internal` / `parse_records_missing_member_returns_missing_member` / `parse_records_wrong_member_type_returns_type_mismatch`；fixture builder `mod fixture`（僅 `#[cfg(test)]`）emit SerializedStreamHeader / BinaryLibrary / Class/SystemClassWithMembersAndTypes / MemberReference / ArraySingleString / ArraySinglePrimitive / BinaryObjectString / ObjectNull / MessageEnd，符合 MS-NRBF §2.3 layout（_items 走 MemberReference → 後續 top-level ArraySingle* referenceable，_size/_version 走 MemberPrimitiveUnTyped）
- [x] D-step 10：quality gates 全綠 — `cargo fmt --check` / `cargo clippy --all-targets -- -D warnings` / `cargo test --lib` 289/289 / `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --lib`
- [x] D-step 11：commit `feat(next): add NRBF parser for legacy Users.dat (P6 chunk 6.1)`

#### Chunk 6.2 — `services/storage/legacy/`（migrator + auto-save wrapper）

- [x] D-step 1：`services/storage/legacy/{mod.rs, error.rs, migrator.rs, load_with_migration.rs}` scaffold；`services/storage/mod.rs` 掛 `pub mod legacy;` + re-exports；新增 `pub(crate) fn records_from_wire_lists(...)` 於 `users_dat.rs`（封裝 `WireRecords` 細節 + 讓 migrator 避開雙重 JSON round-trip）
- [x] D-step 2：`LegacyMigrateError` 2 variants（`Nrbf(NrbfError)` / `Storage(StorageError)`）+ 對應 `From` impl — 放 `legacy/error.rs`
- [x] D-step 3：`migrate_legacy_payload(bytes) -> Result<Records, LegacyMigrateError>` pure — `parse_legacy_payload` → match `LegacyPayload`（Records 7 欄 verbatim / AccountRecords 6 欄 + `account_name_list: None`）→ `records_from_wire_lists`（內部 `WireRecords::normalize()`）
- [x] D-step 4：`migrate_and_save(path, bytes) -> Result<Records, LegacyMigrateError>` async — `migrate_legacy_payload` → `save_records_at` → `tracing::info!` → `Ok(records)`；並有 `migrate_and_save_at` 供測試註冊表隔離
- [x] D-step 5：`load_records_with_legacy_migration(path) -> Result<Records, StorageError>` async — 呼 P5 `load_records_at`；match `Err(LegacyDataDetected { raw_bytes })` → `migrate_and_save_at`；migrate OK → `Ok(records)`；migrate 失敗 → `tracing::warn!` + `Ok(Records::default())` **不刪檔**（對齊 WPF L546-548）；其他 Err propagate；並有 `_at` 變體
- [x] D-step 6：re-exports（`services/storage/mod.rs`）+ 完整 module doc（WPF L494-551 行號對應表 + auto-save rationale + fail-soft 規範 + 允許 root class 限制）
- [x] D-step 7：6 unit tests（cross-platform pure）— `migrate_new_records_shape_preserves_all_seven_fields` / `migrate_legacy_account_records_pads_account_name_list_to_empty_strings` / `migrate_empty_lists_yields_default_records` / `migrate_short_lists_normalize_pads_up_to_account_list_length` / `legacy_migrate_error_display_formats_nrbf_and_storage_variants` / `legacy_migrate_error_from_impl_wires_nrbf_and_storage`；chunk 6.1 的 `mod fixture` 升 `pub mod fixture` 並套 `#[cfg(any(test, feature = "test-fixtures"))]` gate 供跨 module DRY reuse
- [x] D-step 8：9 integration tests in `tests/storage_legacy.rs`（end-to-end real DPAPI + 手刻 NRBF bytes via chunk 6.1 `fixture::build_root_class` + 註冊表隔離 `SOFTWARE\BEANFUN_NEXT_TEST\legacy_<name>_<pid>`）— `migrate_and_save_writes_json_format_round_trippable_by_load_records` / `migrate_and_save_creates_parent_directory_when_missing` / `migrate_and_save_handles_legacy_account_records_padding_account_name_list` / `load_with_migration_auto_upgrades_legacy_users_dat_to_json` / `load_with_migration_on_malformed_nrbf_returns_empty_and_preserves_file` / `load_with_migration_on_new_json_format_skips_migrator_entirely` / `load_with_migration_on_pure_garbage_plaintext_falls_through_p5_default` / `load_with_migration_on_missing_file_returns_empty_and_no_side_effects` / `migrated_json_matches_export_records_byte_for_byte`；Cargo.toml 加 `[features] test-fixtures = []` + `[[test]] storage_legacy required-features = ["test-fixtures"]`（SRP：fixture code 不進 release binary）
- [x] D-step 9：quality gates — `cargo fmt --check` / `cargo clippy --all-targets -- -D warnings`（feature on/off 兩輪）/ `cargo test --lib` 295/295 / `cargo test --test storage_legacy --features test-fixtures` 9/9 / `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --lib`（兩輪）
- [x] D-step 10：commit `feat(next): add legacy Users.dat migration (P6 chunk 6.2)` — `88aff85`

### P7 — Rust `services/updater` + GH proxy

##### 共用設計決議（chunk 7.1 / 7.2 / 7.3 共同）

- **對應 WPF**：`Beanfun/Update/ApplicationUpdater.cs`（294 行全貌）
- **Service-layer only**：MessageBox / `Process.Start(downloadUrl)` 留 P10/P11 commands + UI。Service 只回傳 `Option<UpdateInfo>`
- **A — Proxy cache**：`OnceLock<String>` process-lifecycle 只 probe 一次（對齊 WPF `Lazy<string> _cachedProxy`）
- **B — Probe 成功判定**：嚴格 2xx（對齊 WPF `WebRequest.GetResponse()` 對 4xx/5xx throw `WebException` 的語意）
- **C — 版本比較**：`u128`（WPF `long == i64` 已逼近上限；`{M:D3}{N:D3}{P:D3}{T}` 最大可達 ~1e19，改 `u128` 絕不 overflow）
- **D — Error shape**：top-level `check_update` 回 `Option<UpdateInfo>`（對齊 WPF silent 行為，錯誤走 `tracing::warn!` 吃掉）；下層 `fetch_releases_at` / `proxy_probe_at` / `parse_tag` / `is_newer_version` 回 typed `Result<_, UpdaterError>` 供 test + caller 細控
- **E — Channel**：`enum Channel { Stable, Beta }` + `fn from_config_value(&str)` tolerate 未知 string（fallback `Stable`），對應 WPF `"Beta"` / `"Preview"` 兩個都 → `isBeta = true`
- **F — Probe DI**：`_at` 變體 `proxy_probe_at(direct_url, proxy_urls)` 接 URL 注入（跟 P5 / P6 `_at` pattern 一致）；top-level `proxy_probe()` 包 `const DIRECT_URL = "https://api.github.com"` + `const GH_PROXIES = [...]`
- **G — Concurrent guard**：service 層不管（WPF `Interlocked.CompareExchange` 防 startup + About 重入交 P10 Tauri command 端用 `tokio::sync::Mutex` 處理）
- **H — User-Agent**：`format!("Beanfun(V{})", env!("CARGO_PKG_VERSION"))` compile-time 產
- **I — Release selection tolerance**：`releases` 空陣列 → `None`；`release.tag_name` 不符 `^v(\d+)\.(\d+)\.(\d+)\.(\d+)$` → `None`（對齊 WPF `match.Success == false` 靜默）

##### 驗收條件

- **Chunk 7.1**：`ParsedVersion` / `parse_tag` / `is_newer_version` 對 pre-5.8 老格式 `v5.7` + `v5.8.13(2604011114)` + 新 timestamp 格式三路 `IsNewerVersion` 都能正確比較；`proxy_probe_at` 對 wiremock 模擬的「直連 OK」/「直連 fail + proxy 1 OK」/「前 2 proxy fail + 第 3 OK」/「全 fail → `None`」四 case pass
- **Chunk 7.2**：`fetch_releases_at` 對合法 GH API JSON 能解出 `Vec<GitHubRelease>` + assets[0].browser_download_url；`Channel::from_config_value` 對 `"Stable"` / `"Beta"` / `"Preview"` / 未知 string 行為一致；`select_release` 對 Stable / Beta channel 的 prerelease 篩選邏輯對齊 WPF
- **Chunk 7.3**：`check_update` 的 happy path（有新版）/ up-to-date / 錯誤 silent 三路都 pass；整合 wiremock 測全鏈路（probe → fetch → select → parse → compare）
- **P7 總驗收**：至少 8 cases integration pass；`UpdaterError` 完整 surface；service 層不含 UI 呼叫

#### Chunk 7.1 — `parser.rs` + `proxy_probe.rs`（pure 版本邏輯 + 網路 probe）

- [x] D-step 1：`services/updater/{mod.rs, error.rs, parser.rs, proxy_probe.rs}` scaffold；`services/mod.rs` 掛 `pub mod updater;`；`mod.rs` re-export `UpdaterError` / `ParsedVersion` / `parse_tag` / `is_newer_version` / `proxy_probe` / `proxy_probe_at`
- [x] D-step 2：`UpdaterError` enum — `Probe(reqwest::Error)` / `Fetch(reqwest::Error)` / `JsonDecode(serde_json::Error)` / `UnsupportedTag(String)` 四 variants（用 `#[source]` 保留 chain）；放 `services/updater/error.rs`（`thiserror::Error` derive，不需手寫 `From` impl — variant 上的 `#[from]` 等 7.2/7.3 真正用到時再補，保持 YAGNI）
- [x] D-step 3：`ParsedVersion { major: u32, minor: u32, patch: u32, timestamp: String }` + `parse_tag(&str) -> Result<ParsedVersion, UpdaterError>`（regex `^v(\d+)\.(\d+)\.(\d+)\.(\d+)$`；失敗回 `UnsupportedTag(tag.to_owned())`）；**timestamp 選 `String` 而非 `u64`**：保留原始 digit 數量，`pack_version` 才能 byte-for-byte 對齊 WPF `{0:D3}{1:D3}{2:D3}{3}` 輸出（10 vs 11 digit timestamp 不會被 silently pad）
- [x] D-step 4：`is_newer_version(local: &str, remote: &ParsedVersion) -> bool` — 兩條路：Path A display form (`(\d+)\.(\d+)\.?(\d+)?\.?\((\d+)\)`) 走 u128 packed 比較 + timestamp 相等短路 false（對齊 WPF L236-239）；Path B fallback 走「去非數字 + pad-left 19 + u128 parse」；解析失敗一律回 false 對齊 WPF `catch`（L287-291）；**`pack_version` 選 u128 而非 i64**：WPF `long.Parse` 對 19 digit 字串接近 i64 上限，未來 major/minor 擴張可能溢位；u128 上限 3.4×10³⁸ 安全
- [x] D-step 5：`proxy_probe_at(direct_url: &str, proxies: &[&str]) -> String` async — HEAD request + `error_for_status()` 嚴格 2xx + 5s timeout；回 `""` 表直連 OK 或全 fail（對齊 WPF `DiscoverProxy` L48-50 / L59）、proxy prefix 表該 proxy OK；**`build_probe_client` 失敗也回 `""`** 對齊 WPF `catch`
- [x] D-step 6：`proxy_probe() -> &'static str` — `static OnceLock<String>` 包 top-level（`get → get_or_init` pattern，允許初始化期間 race 但收斂到同一答案）+ `const DIRECT_URL = "https://api.github.com"` / `const GH_PROXIES = ["https://ghproxy.vip/", "https://ghproxy.net/", "https://ghfast.top/"]` / `const PROBE_TIMEOUT = Duration::from_secs(5)`；User-Agent `Beanfun(V{CARGO_PKG_VERSION})` 對齊 WPF L36 / L123 shape
- [x] D-step 7：module doc — `mod.rs` + `error.rs` + `parser.rs` + `proxy_probe.rs` 各附 WPF 行號對應表（L15-62 / L220-292 / L135-137 / L40-43, 195-198）+ strict 2xx rationale + OnceLock race semantic + u128 safety rationale + Path A/B 使用情境說明（referrencing `App.xaml.cs::ConvertVersion` L80-102 — `App.AssemblyVersion` 永遠回傳 display form）
- [x] D-step 8：23 unit tests — `parse_tag` 5 case（canonical / double-digit / 缺 v / 3 component / 尾巴 garbage）/ `is_newer_version` 6 case（display-form upgrade / display-form same-timestamp 短路 / display-form 缺 patch / Path A patch-bump numeric ordering `5.8.9 < 5.8.10` / Path B lossy-concat WPF-bug lock-in（older remote 被誤判為 newer — 保 WPF parity，任何未來「修 bug」會 trip test）/ garbage local fallthrough）+ `pack_version` zero-pad / `left_pad_to` 2 case + `proxy_probe_at` 5 case via wiremock（direct 200 / direct fail + proxy B OK / 全 503 / 非 2xx 拒絕 / 連線拒絕 transport fail）+ 常數 assertion 4 case（`GH_PROXIES` literal / `DIRECT_URL` literal / `PROBE_TIMEOUT` 5000ms / UA shape）
- [x] D-step 9：quality gates 全綠 — `cargo fmt --check` / `cargo clippy --all-targets -- -D warnings`（feature on/off 兩輪）/ `cargo test --lib` 318/318（較 P6.2 的 295 多 23 個 updater tests）/ `cargo test --test storage_legacy --features test-fixtures` 9/9 / `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --lib`
- [x] D-step 10：commit `feat(next): add updater parser + proxy probe (P7 chunk 7.1)` — `cdb374b`

#### Chunk 7.2 — `github.rs` + Channel（fetch releases + prerelease 篩選）

- [x] D-step 1：`services/updater/github.rs` scaffold + mount；`services/updater/mod.rs` 擴充 `pub mod github;` + re-exports (`GitHubRelease` / `GitHubAsset` / `Channel` / `fetch_releases` / `fetch_releases_at` / `select_release` / `GH_API_RELEASES_URL` / `GITHUB_ACCEPT_HEADER`)
- [x] D-step 2：`GitHubRelease { name, tag_name, prerelease, body, assets: Vec<GitHubAsset> }` + `GitHubAsset { browser_download_url }`；**選 `#[serde(default)]`** per field（而非全 struct `rename_all = "snake_case"`）— GitHub API 本來就用 snake_case 不需 rename，`#[serde(default)]` 讓 optional fields（name/body/prerelease/assets）在缺席時不 panic，對齊 WPF `JsonProperty` + nullable-class-field 預設行為
- [x] D-step 3：`Channel { Stable, Beta }` enum + `Channel::from_config_value(&str)`（`"Beta"` / `"Preview"` → `Beta`；其他 → `Stable`，對齊 WPF L203-204）；**case-sensitive** 對齊 WPF `string.Equals` 無 `OrdinalIgnoreCase`（測試鎖住 `"beta"` / `"BETA"` 都回 `Stable`）；額外 `impl Default for Channel` 回 `Stable`
- [x] D-step 4：`fetch_releases_at(base_url: &str, user_agent: &str) -> Result<Vec<GitHubRelease>, UpdaterError>` async — 每次呼叫建新 `reqwest::Client`（無 cookies / 無 redirect config / 無 timeout — 由 OS 預設接管；不 DRY 到 P2 `BeanfunClient`，因為那個是 login-specific 有 cookie jar）+ GET + `Accept: application/vnd.github.v3+json` + `error_for_status()` + `bytes().await` → `serde_json::from_slice` → Fetch / JsonDecode 明確區分
- [x] D-step 5：`fetch_releases(proxy_prefix: &str) -> Result<Vec<GitHubRelease>, UpdaterError>` — 用 const `GH_API_RELEASES_URL = "https://api.github.com/repos/pungin/beanfun/releases"` + const `GITHUB_ACCEPT_HEADER = "application/vnd.github.v3+json"` + UA `Beanfun(V{env!("CARGO_PKG_VERSION")})`
- [x] D-step 6：`select_release(releases: &[GitHubRelease], channel: Channel) -> Option<&GitHubRelease>`（對齊 WPF L201-214 — Beta 拿第一個、Stable `find(!prerelease)`）；edge case 全 prerelease + Stable → `None`
- [x] D-step 7：module doc — WPF 行號對應表（L64-86 schema / L117 URL / L121-127 GET headers / L201-214 selection） + channel case-sensitive rationale + headers pinning rationale + 與 `proxy_probe` 的 `{proxy}{url}` convention 說明
- [x] D-step 8：15 unit tests（超過目標 ~6）— Channel 4 case（Beta/Preview/Stable/default + case-sensitive 鎖 WPF parity）+ `select_release` 4 case（Stable skip prerelease / Beta first / Stable 全 prerelease None / 空 list）+ `GitHubRelease` deserialize real-shape JSON（含額外 GitHub 欄位忽略 + missing optional defaults + assets 巢狀）+ `fetch_releases_at` 4 case via wiremock（happy path 驗 UA+Accept header / 403 Fetch / bad JSON JsonDecode / connect refused Fetch）+ 2 常數 assertion（URL / Accept header 對 WPF literal）
- [x] D-step 9：quality gates 全綠 — `cargo fmt --check` / `cargo clippy --all-targets -- -D warnings` / `cargo test --lib` 333/333（7.1 後 318 → 現 333）/ `cargo test --test storage_legacy --features test-fixtures` 9/9 / `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --lib`（修 `super::proxy_probe` ambiguous-link 成 `mod@super::proxy_probe` × 2 處）
- [x] D-step 10：commit `feat(next): add updater GitHub fetch + channel selection (P7 chunk 7.2)` — `a0a7663`

#### Chunk 7.3 — `checker.rs`（top-level `check_update` 組合）

- [x] D-step 1：`services/updater/checker.rs` scaffold + mount；`UpdateInfo { new_version_display, body, download_url, tag_name }`（4 欄位對齊 WPF L145 newVerDisplay + L150-159 Body + L169-172 downloadUrl + release.TagName 作診斷）；`services/updater/mod.rs` re-export `check_update` / `check_update_at` / `UpdateInfo`；call-graph ASCII 圖加進 mod doc
- [x] D-step 2：`check_update(channel: Channel, local_version: &str) -> Option<UpdateInfo>` async — 用 `proxy_probe()`（OnceLock cached）+ `env!("CARGO_PKG_VERSION")` UA + `GH_API_RELEASES_URL` 組 prod 版；內部 delegate 到 private `run_check(prefix, api_url, ua, channel, local_version)` helper（避免 `check_update` / `check_update_at` 雙份 pipeline 違反 DRY）；每個 `Err(UpdaterError)` 走 `log_and_discard("stage", err)` → `tracing::warn!` 吞掉回 `None`；up-to-date / empty feed 走 `tracing::info!`（區分 silent-fail vs true-no-update 以便 debug，但對外仍是 `None` 對齊 WPF L195-198）
- [x] D-step 3：`UpdateInfo::from_release(release, parsed, proxy_prefix)` 純同步 builder；**download_url 嚴格 mirror WPF L169-172 asymmetry** — assets[0].browser_download_url 前綴 proxy（`format!("{proxy_prefix}{url}")`）/ assets 空時 fallback `github.com/pungin/Beanfun/releases/tag/{tag_name}`（不加 proxy prefix，對齊 WPF 該分支刻意的不對稱）；module doc + 專用 lock-in 測試 `update_info_download_url_fallback_skips_proxy_per_wpf_asymmetry` 鎖住行為，避免未來「統一 proxy」誤修
- [x] D-step 4：`check_update_at(probe_direct_url, probe_proxies, api_releases_url, channel, local_version, user_agent)` 6-param DI 版本；直接呼叫 `proxy_probe_at`（bypass OnceLock cache，測試間零 cross-contamination）；同樣 delegate 到 `run_check`；`api_releases_url` 設計為「被 proxy prefix 前綴的 target URL」而非「最終 fetch URL」——解決 proxy 前綴 semantics 與 prod/test 一致
- [x] D-step 5：module doc — WPF L114-199 `RunCheck` 行號對應表（L116→proxy / L117→fetch_url / L121-127→fetch_releases / L128-131→select / L135-137→parse_tag / L145→new_version_display / L148→is_newer / L169-172→download_url asymmetry / L195-198→silent catch）+ "Silent-on-error policy" + "`download_url` proxy asymmetry" 兩節獨立說明 + call-graph 在 `mod.rs`
- [x] D-step 6：9 unit tests in `checker.rs` — `UpdateInfo::from_release` 4 case（WPF format / proxy prefix 加入 asset / 直連不加 proxy / fallback page URL 不加 proxy lock-in）+ `check_update_at` 5 async case via wiremock（happy path + 新版 / local 與 latest 相同 → None / tag regex 不 match → None / fetch 500 → None / empty releases → None）
- [x] D-step 7：8 integration tests in `tests/updater.rs`（wiremock 模擬 GH + proxies）— 直連 OK + 有新版 / 直連 OK + 沒新版 / 直連 fail → proxy 1 OK（驗證 download_url 前綴 proxy）/ 前 2 proxy 500-504 失敗 → 第 3 proxy OK（驗證 probe 順序 + download_url 前綴的是第 3 個 proxy）/ 全 probe + fetch fail → None / Stable channel 略過 prerelease 取第一個 stable / Beta channel 拿最新 prerelease / pre-5.8 display form local（`5.7.0(2503010000)`）與新 timestamp remote 比較走 Path A 成功
- [x] D-step 8：quality gates 全綠 — `cargo fmt --check` ✓ / `cargo clippy --all-targets -- -D warnings`（feature on/off 兩輪）✓ / `cargo test --lib` **342/342**（較 7.2 的 333 多 9 個 checker unit tests）✓ / `cargo test --test storage_legacy --features test-fixtures` 9/9 ✓ / `cargo test --test updater` 8/8 ✓ / `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --lib` ✓（修 2 處 `super::proxy_probe` ambiguity → `proxy_probe()` 明示函式 or `mod@` 明示模組；把 private helper `run_check` 的 intra-doc link 改成 plain backtick avoid「public doc → private item」error）
- [x] D-step 9：commit `feat(next): add updater check_update composition (P7 chunk 7.3)` — `061f3f6`

### P8 — Rust `services/game` 啟動 + LR（SHA-256 安全升級）

##### 共用設計決議（chunk 8.1 / 8.2 共同）

- **對應 WPF**：`Beanfun/MainWindow.xaml.cs::btn_Run_Game_Click` (L1727-1900) + `startByLR` (L1902-1947) + `Beanfun/App.xaml.cs::ReleaseResource` (L131-167)
- **Service-layer only**：UI dialog（MsgGamePathHaveWChar / MsgLocalePluginReleaseError / MsgLocalePluginRunError / MsgGameAlreadyRun / MsgCantFindGame / MsgLEDoNotSupportXP）留給 P10/P12 Tauri commands + Vue pages。Service 回 typed `GameError`，UI 決定顯示什麼訊息
- **Out of scope**：process find/kill（P9 `services/process`）、register 遊戲路徑偵測（P9 `services/registry`）—— launcher.rs 只接 `game_path: &Path` 已定值，不自己查登錄檔
- **A — LR 5 檔來源**：`include_bytes!("../../../../Beanfun/LocaleRemulator/{LRConfig.xml,LRHookx32.dll,LRHookx64.dll,LRProc.exe,LRSubMenus.dll}")` 直接相對參照 WPF tree（DRY；WPF 端更新自動流入 beanfun-next）
- **B — SHA-256 安全升級**：WPF `ReleaseResource` L140-142 只比 `FileInfo.Length == stream.Length`；我們升級成 SHA-256 byte-level 比對，防止同長度但內容被竄改（Todo.md 明示「SHA-256 安全升級」，WPF 原行為被**刻意**拋棄，並在 doc 說明）
- **C — TOCTOU 不處理**：release-time verify + overwrite 足夠；launch 前不 re-verify（runas 彈 UAC 的世界觀下 over-engineer，且 WPF 也沒做）
- **D — Auto 模式 resolve 位置**：launcher.rs 內部 `resolve_mode(Auto)` → `GetSystemDefaultLocaleName()`（Win32）→ `zh-TW / zh-CHT / zh-Hant / zh-HK / zh-MO` → Normal，否則 LocaleRemulator（對齊 WPF L1838-1860，UI 不用預 resolve）
- **E — XP check 砍掉**：WPF L1850-1853 `OSVersion < WinVista` 的錯誤路徑是 dead code（Tauri 最低 Windows 7 SP1），砍掉並於 module doc 標記對應 WPF 行號
- **F — 非 ASCII 偵測**：`path.chars().any(|c| (c as u32) > 128)` Unicode scalar value（對齊 WPF UTF-16 code unit `> 128` 語意；遊戲路徑無 surrogate pair realistic scenarios 下等價）
- **G — Error shape**：`services/game/error.rs` 單一 `GameError` enum（對齊 P7 `UpdaterError` shape），variants：
  - `PathEmpty` / `PathNotFound(PathBuf)` / `PathNonAscii { path, offending_char, position }`
  - `LocaleRemulatorRelease { name, source: io::Error }`
  - `LocaleRemulatorSha256Mismatch { name }`（既有檔 hash 不符但「刪除」step 失敗才會冒出；正常情況會靜默覆蓋）
  - `ShellExecute { source: windows::core::Error }`
  - `Spawn(io::Error)`
  - `LocalePluginUnsupported`（Windows locale query 失敗時的防禦）
- **H — GUID**：`const LR_GUID: &str = "ef3e7b42-a87c-4c07-ae3e-eeebeef12762"`（與 WPF L1931 + LRConfig.xml Profile Guid 字符對應）
- **I — `%s` 替換**：`substitute_credentials(template, account, password)` pure helper；兩次 `replacen("%s", ..., 1)`（對齊 WPF L1876-1878 `Regex.Replace(..., 1)` 行為）
- **J — 非 Windows 編譯**：`services/game/launcher.rs` 保持 cross-platform（Normal 模式 spawn + path validate 都跨平台，locale 查詢有 `#[cfg(windows)]` + `#[cfg(not(windows))]` stub 回 `Normal`）；`services/game/locale_remulator.rs` 全檔 `#[cfg(windows)]`（5 個 DLL 只在 Windows 有意義）
- **K — build.rs SHA-256**：把 5 檔 hash 在 build-time 計算並寫到 `$OUT_DIR/lr_sha256.rs` 的 `pub const LR_SHA256: [(&str, [u8; 32]); 5]`；build-deps 加 `sha2`（與 runtime deps 已有的 sha2 不衝突）；`println!("cargo:rerun-if-changed=...");` 5 檔 + build.rs 自身

##### 驗收條件

- **Chunk 8.1**：`validate_path` / `resolve_mode` / `substitute_credentials` / `launch_normal` 四 pure primitives 全綠；Auto→Normal（zh-TW locale 模擬）/ Auto→LR（en-US locale 模擬）/ explicit Normal / explicit LR 四路 resolve 正確；non-ASCII 路徑（繁中 / 日文 / emoji）全被 reject；`%s` 替換 1/2 個 / 0 個 / template 空 五個邊界都對
- **Chunk 8.2**：`release_all` 於 tempdir 產出 5 檔且 SHA-256 一致；既有檔 hash 符合 → skip；篡改一 byte → 自動覆寫；`build_lr_arguments` 對含空白 / 特殊字元 game_path 正確 quote；`launch_game` 完整 orchestrator（validate → resolve → Normal/LR dispatch）三路 happy + 錯誤 surface 都正確
- **P8 總驗收**：至少 25 unit tests + 1 integration test；`GameError` 完整 surface；service 層不含 UI 呼叫；SHA-256 拒絕被竄改 DLL；5 檔釋出與 WPF 行為等價（內容） + 升級（驗證強度）

#### Chunk 8.1 — `launcher.rs` primitives + Normal 模式

- [x] D-step 1：`services/game/{mod.rs, error.rs, launcher.rs}` scaffold；`services/mod.rs` 掛 `pub mod game;`；`Cargo.toml` 加 `Win32_Globalization` feature 到 `windows` crate（`GetSystemDefaultLocaleName` 用）
- [x] D-step 2：`GameError` enum in `services/game/error.rs` — 完整 7 variants declared up-front（8.2 未用的先 declare 避免 enum breaking change）：`PathEmpty` / `PathNotFound { path: PathBuf }` / `PathNonAscii { path, offending_char, position }` / `LocaleRemulatorRelease { name, source: io::Error }` / `LocaleRemulatorSha256Mismatch { name }` / `ShellExecute { source }` `#[cfg(windows)]` / `Spawn(#[from] io::Error)`；`thiserror` derive + `#[source]` chain + `{ path.display() }` 格式化；module doc 附 WPF 行號對應表；**`LocalePluginUnsupported` 砍掉**（Win32 locale 查詢失敗時 fallback 到 LR 更安全，對齊 WPF L1857 default 臂，不 surface error）
- [x] D-step 3：`GameStartMode { Auto = 0, Normal = 1, LocaleRemulator = 2 }` `#[repr(i32)]` 對齊 WPF enum int；`TryFrom<i32>` 0/1→對映、`>=2` → clamp LR（對齊 WPF L1863-1864，3/999 同落 LR）、`<0` → `Err(i32)` 讓 caller 決定 fallback；`ResolvedMode { Normal, LocaleRemulator }` 是 Auto resolve 產出
- [x] D-step 4：`validate_path(path: &Path) -> Result<(), GameError>` — 空 / 不存在 / `chars().enumerate().find((c as u32) > 128)` 三 check；`path.to_str()` None case 也吞進 `PathNonAscii`（U+FFFD 替代字符 + 位置 0）；回 `PathNonAscii { path, offending_char, position }` 帶診斷資訊
- [x] D-step 5：`resolve_mode(mode) -> ResolvedMode`（無 Result，fail-soft）+ `locale_to_resolved_mode(locale: &str) -> ResolvedMode` 拆 pure helper（單測不碰 Win32）+ `query_system_locale()` 私有 `#[cfg(windows)]` 用 `GetSystemDefaultLocaleName` + inline `LOCALE_NAME_MAX_LENGTH = 85`（winnls.h 常數；該 feature 未 re-export，inline + source ref 而非多拉 feature flag）；`#[cfg(not(windows))]` stub 回 `None` → resolve 到 LR；Win32 call 失敗也 fallback LR（對齊 WPF L1857 pessimistic default）
- [x] D-step 6：`substitute_credentials(template, account, password) -> String` pure — 兩次 `replacen("%s", _, 1)`；對齊 WPF L1876-1878 兩次 `Regex.Replace(..., 1)`；3+ `%s` template 只替前 2 個（parity lock 用 test 鎖住）
- [x] D-step 7：`launch_normal(path, command_line) -> Result<(), GameError>` — `Command::new(path)` + `.current_dir(path.parent().unwrap_or("."))` + `.arg(command_line)` 只在 non-empty 時 push（避 empty argv 造成部分遊戲誤判）+ `.spawn()?`；對齊 WPF L1886-1891；`Child` drop 讓 game detach；io::Error 經 `#[from]` 自動轉 `GameError::Spawn`
- [x] D-step 8：module docs — `services/game/mod.rs` call-graph + scope 聲明（process/registry 屬 P9 範疇）；`launcher.rs` WPF 行號對應表（L1727-1900 逐 helper 對應 column）+ deliberate departures section（XP dropped / Unicode scalar vs UTF-16 / LocalePluginUnsupported 砍）+ cross-platform stance；`error.rs` 7 variants 各有對應 WPF 行 + SHA-256 upgrade 註解
- [x] D-step 9：**28 unit tests**（超過計畫的 15-20，增補 edge cases）— `validate_path` 6（空 / 不存在 / ASCII / 繁中 / 日文 / emoji）+ `GameStartMode::try_from` 5（0/1/2/clamp 3 + 999/reject -1）+ `locale_to_resolved_mode` 7（zh-TW / zh-HK / 5 tags batch / en-US / zh-CN / ja-JP）+ `resolve_mode` 3（Normal / LR pass-through / Auto smoke）+ `substitute_credentials` 6（2 slot / 1 slot / 0 slot / empty template / empty account / 3 slot only-first-2-replaced parity lock）+ `launch_normal` 2（Windows cmd.exe smoke + missing binary spawn error cross-platform gated）
- [x] D-step 10：quality gates 全綠 — `cargo fmt` ✓ / `cargo clippy --all-targets -- -D warnings`（feature on/off 兩輪）✓ / `cargo test --lib` **370/370**（較 P7.3 的 342 多 28）✓ / `cargo test --test storage_legacy --features test-fixtures` 9/9 ✓ / `cargo test --test updater` 8/8 ✓ / `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --lib` ✓（修 1 處 `query_system_locale` private-item link → plain backtick）
- [x] D-step 11：commit `feat(next): add game launcher primitives + Normal mode (P8 chunk 8.1)` — `40e26fa`（review 後 amend：`launch_normal` 改用 `CommandExt::raw_arg` 對齊 WPF `Arguments` verbatim-append 語意，避免 Rust `Command::arg` 的自動引號包裹讓遊戲 CRT argv parser 誤把整串 `/hb /u:a /p:b` 當單一 token）

#### Chunk 8.2 — `locale_remulator.rs` + SHA-256 embed + `launch_game` orchestrator

##### 8.2 pre-flight 校準決策（2026-04-17）

- **cfg gating = B 精細**：只有 `launch_via_lr`（ShellExecuteW）+ wide-string helper `#[cfg(windows)]`；`verify_file` / `release_file` / `release_all` / `build_lr_arguments` / `LR_ASSETS` / `LR_GUID` 全部 cross-platform（unit + integration test 在 macOS/Linux 也跑得動）
- **LaunchRequest = B 4 欄位**：`LaunchRequest { game_path, command_line, mode, target_dir }`；另外提供 `default_target_dir() -> io::Result<PathBuf>`（包 `env::current_exe()?.parent()`）給 Tauri command 層（P10）用；service 層 `launch_game` 完全 pure，test 可任意 mock target_dir
- **Chunking = A 單一 commit**：14 D-steps 合成一個 commit，與 P6.2 / P7.3 量級一致
- **Release skip 判定 = A 總是 hash**：不做 length fast-path 捷徑（240KB × 5 在 i3 上 <1ms，分支省不回來、程式碼更清爽）

- [x] D-step 1：`Cargo.toml` 加 `[build-dependencies] sha2 = "0.10"`；確認 `windows` crate 已有 `Win32_UI_Shell`（Cargo.toml 的 features 列表檢查）；runtime 端 `sha2` 應該已在（P5 DPAPI 用）但再確認一次
- [x] D-step 2：`build.rs` 擴充 — read 5 檔 from `../../Beanfun/LocaleRemulator/*`（`CARGO_MANIFEST_DIR` 相對兩級 up）、compute SHA-256、write `$OUT_DIR/lr_sha256.rs` 含 `pub(crate) const LR_SHA256: [(&str, [u8; 32]); 5]`；`cargo:rerun-if-changed=` 每檔 + build.rs 自身；檔案不存在 `panic!` 清楚訊息含絕對路徑
- [x] D-step 3：`services/game/locale_remulator.rs` scaffold（**非全檔 `#[cfg(windows)]`**，只有 `launch_via_lr` + `to_wide_null` cfg-gated）；`include_bytes!` 5 檔（`../../../../../Beanfun/LocaleRemulator/*` 相對五級 up，從 src/services/game/locale_remulator.rs 算）+ `include!(concat!(env!("OUT_DIR"), "/lr_sha256.rs"))`；`pub const LR_ASSETS: [(&str, &[u8]); 5]`（bytes 與 hash 拆兩表，hash 另掛 `pub(crate) LR_SHA256` 經 `expected_sha256()` 查）；`pub const LR_GUID: &str = "ef3e7b42-a87c-4c07-ae3e-eeebeef12762";`
- [x] D-step 4：`verify_file(path: &Path, expected: &[u8; 32]) -> io::Result<bool>` pure — `fs::read(path)` → `Sha256::digest` → 比較；`NotFound` 特殊 case 回 `Ok(false)`（非 error，讓呼叫方用 outcome 判斷）；其他 io::Error 原樣 propagate
- [x] D-step 5：`pub enum ReleaseOutcome { Skipped, Created, Rewritten }`（`Copy + PartialEq + Debug`）；`release_file(target_dir, name, bytes, expected) -> Result<ReleaseOutcome, GameError>` — 先 `verify_file` → 若 `Ok(true)` 回 Skipped；若 path 存在但 hash 不符 → `fs::remove_file` 後 write `Rewritten`；若 path 不存在 → 建 parent dir（`fs::create_dir_all`）+ `fs::write` → `Created`；io::Error 全部包成 `GameError::LocaleRemulatorRelease { name: static_name, source }`
- [x] D-step 6：`release_all(target_dir: &Path) -> Result<[ReleaseOutcome; 5], GameError>` — 依 `LR_ASSETS` 順序 loop；任一失敗 short-circuit（對齊 WPF L1904-1914 `|| chain` 語意）；回 5-element array 帶每檔的 outcome（diagnostic 用）
- [x] D-step 7：`build_lr_arguments(game_path: &Path, command_line: &str) -> String` — 對齊 WPF L1917-1918：`path_str = game_path.to_string_lossy();` `let path_part = if path_str.starts_with('"') { format!("{path_str} ") } else { format!("\"{path_str}\" ") };`；最終 `format!("{LR_GUID} {path_part}{command_line}")`（注意 path_part 已帶尾綴空白）
- [x] D-step 8：`launch_via_lr(target_dir: &Path, game_path: &Path, command_line: &str) -> Result<(), GameError>` `#[cfg(windows)]` 限定 — `ShellExecuteW` + `runas` verb + `SW_SHOWNORMAL`；`lpFile = target_dir.join("LRProc.exe")`；`lpParameters = build_lr_arguments(...)`；`lpDirectory = game_path.parent()`（若 None fallback `Path::new(".")`）；UTF-16 轉換經 `to_wide_null` helper；返回值 `HINSTANCE`，cast 成 `isize`，`<= 32` → `GameError::ShellExecute { source: windows::core::Error::from_win32() }`
- [x] D-step 9：`launcher.rs` 頂層新增：
    - `pub struct LaunchRequest { game_path: PathBuf, command_line: String, mode: GameStartMode, target_dir: PathBuf }`
    - `pub fn default_target_dir() -> io::Result<PathBuf>` — `env::current_exe()?.parent().ok_or(NotFound).to_path_buf()`
    - `pub fn launch_game(req: &LaunchRequest) -> Result<(), GameError>` orchestrator — `validate_path(&req.game_path)?;` → `resolve_mode(req.mode)` → `Normal` arm call `launch_normal`；`LocaleRemulator` arm `#[cfg(windows)]` 分支 call `release_all` + `launch_via_lr`；`#[cfg(not(windows))]` 分支 也呼 `release_all` 再 fallback 到 `launch_normal`（dev/CI 用；production LR 永遠在 Windows）
- [x] D-step 10：module docs — `locale_remulator.rs` WPF 行號對應表 + SHA-256 upgrade rationale + TOCTOU not-handled rationale；`launcher.rs` 表格加入 `launch_game` / `LaunchRequest` / `default_target_dir`；`mod.rs` 加 top-level call graph ASCII 圖 + SHA-256 security upgrade section
- [x] D-step 11：**27 unit tests**（超出計畫的 15）— locale_remulator 18 + launcher.rs 新增 9（覆蓋 LR_ASSETS/LR_SHA256 平行、SHA-256 byte match、GUID lock-in、verify_file 4 案、release_file 5 案含 length-match-but-hash-differs security lock-in、release_all 3 案、build_lr_arguments 4 案、ShellExecute 錯誤映射、launch_game 4 案含 validate/non-ASCII/missing/normal smoke、default_target_dir smoke 等）
- [x] D-step 12：1 integration test `tests/game_locale_remulator.rs`（cross-platform，**6 tests**）— release_all 綠燈 5 案、SHA-256 驗 5 檔、再次 Skipped、tamper → Rewritten only、delete → Created only、embedded length sanity
- [x] D-step 13：quality gates 全綠 — `cargo fmt --check` ✓ / `cargo clippy --all-targets -- -D warnings`（default + test-fixtures 兩輪）✓ / `cargo test --lib` **397/397**（較 P8.1 的 370 多 27）✓ / `cargo test --test game_locale_remulator` 6/6 ✓ / `cargo test --test updater` 8/8 ✓ / `cargo test --test storage_legacy --features test-fixtures` 9/9 ✓ / `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --lib` ✓（修 2 處 `LR_SHA256` / `expected_sha256` private-item link → plain backtick + 1 處 doc list indent warning）
- [x] D-step 14：commit `feat(next): add LocaleRemulator embed + SHA-256 release + runas launch (P8 chunk 8.2)` — `6fbf8be`（實際 HEAD `fdada84` post-amend；1-step Todo 記錄漂移可接受）

##### 8.2 review follow-up（2026-04-17，選項 C：全修）

Review 發現 6 個問題，依風險高中低切 5 個 R-step 修改 + 1 個 gates + 1 個 commit：

- [x] R8.2-1：**LaunchRequest Debug redaction**（R1 + R6 合併）— `LaunchRequest` 手寫 `impl Debug`（不再 derive）把 `command_line` 欄位 redact 成 `<redacted; len=N>`，其他 3 欄（game_path / mode / target_dir）維持原樣；`command_line` 欄位 doc 加 `# Security` 段警告「contains post-substitution credentials; never log/persist/display」；struct-level doc 加 `# Debug redaction` 段說明；新增 2 單元測試鎖定（`launch_request_debug_redacts_command_line` 驗 `{req:?}` 不含 account/password、`launch_request_debug_preserves_non_secret_fields` 驗其他欄位仍可讀）
- [x] R8.2-2：**release_file 簡化 + TOCTOU 硬化**（R2）— 移除冗餘 `target.exists()` 第二次 syscall；移除 `!parent.exists() && create_dir_all(...)` 過度保守分支（`create_dir_all` 本身是 idempotent）；把 `Created` vs `Rewritten` 的判定從「verify 後的 snapshot」改成「`fs::remove_file` 的真實回傳」：`Ok(())` → Rewritten、`NotFound` → Created、其他 io::Error propagate。補 1 單元測試 `release_file_handles_missing_file_as_created_not_error` 鎖 TOCTOU 邊界（檔案在 verify 與 remove 之間消失時走 Created 而非錯誤）
- [x] R8.2-3：**GameError::ShellExecute 承載 pseudo-HINSTANCE**（R4）— `ShellExecute` variant 加 `code: i32` 欄位，保留 `source: windows::core::Error` 不動；`launch_via_lr` 把 `ShellExecuteW` 回傳的 `raw as i32` 填入。UI 層（P10）可直接 branch 在 `code` 上分辨 `SE_ERR_FNF=2` / `SE_ERR_ACCESSDENIED=5` / `ERROR_CANCELLED=1223`（UAC 取消）等；`source` 保留做 best-effort 次訊號。無既有 call site pattern-match 此 variant → 加欄位是 additive、不 break
- [x] R8.2-4：**launch_via_lr doc 補 spawn_blocking**（R5）— `launch_via_lr` doc 加 `# Async runtime guidance` 段說明 P10 Tauri command 在 Tokio runtime 上必須用 `tokio::task::spawn_blocking` 包裹（對齊 WPF L1923 `new Thread(...)` 避免 UI 卡死在 UAC prompt），service 層自身保持 sync
- [x] R8.2-5：**integration test 匯入清理**（R3）— `tests/game_locale_remulator.rs` 刪掉 `use locale_remulator::{self};` 以及最後一行的 `let _ = locale_remulator::LR_GUID;` workaround（那兩個合在一起只是為了避 unused-import 警告硬塞的 no-op，`LR_GUID` 在該測試檔根本沒真的用到）
- [x] R8.2-6：quality gates 全綠 — `cargo fmt --check` ✓ / `cargo clippy --all-targets -- -D warnings` ✓ / `cargo test --lib` **400/400**（較 P8.2 原本 397 多 3：2 Debug redaction + 1 TOCTOU lock-in）✓ / `cargo test --test game_locale_remulator` 6/6 ✓ / `cargo test --test updater` 8/8 ✓ / `cargo test --test storage_legacy --features test-fixtures` 9/9 ✓ / `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --document-private-items` ✓
- [x] R8.2-7：commit `fix(next): apply P8.2 review follow-ups (redact Debug, tighten release_file, enrich ShellExecute error)` — `c029174`

### P9 — Rust `services/process` + `services/registry`

升級成 P5/P7/P8 風格的 chunk 切分。WPF reference 探勘於 2026-04-17 由 explore subagent 完成，以下 calibration 是展開後的共識。

#### P9 pre-flight calibration（2026-04-17）— C1 ~ C8 全接受

- **C1**（`services/registry/game_path.rs` scope）：WPF `ModifyRegistry` 有 Read/Write 兩面，但遊戲路徑**實際只讀一次** seed 到 `ConfigAppSettings`（Config.xml）；寫回是寫 Config.xml 不是 Registry。Rust 端 `services/registry/game_path.rs` **只實作 read**，寫路徑歸 P11 Config
- **C2**（HKCU vs HKLM）：`ModifyRegistry` 預設 hive 是 `LocalMachine`，但 `selectedGameChanged` L584-593 讀遊戲路徑用的是 `Registry.CurrentUser`。Rust 版本提供兩個 hive 的查詢函式，順序以 WPF 實際行為（HKCU 優先）為準
- **C3**（`kill.rs` 實作）：WPF 用 `Process.Kill()`（.NET），沒走 P/Invoke。但 Rust `std` 不支援 kill-by-external-PID。**必要升級**：用 `windows::Win32::System::Threading::{OpenProcess, TerminateProcess}` Win32 API；行為等價、接口新
- **C4**（Patcher timer 所有權）：WPF `checkPatcher` 是 `DispatcherTimer 100ms` 耦合 UI（建構子 + selectedGameChanged 啟停 + Settings 頁勾選），Tick 內還做版本檢查與下載提示。**Service 層只做 pure 單次呼叫**：`check_and_kill_patcher(game_path) -> Option<killed_pid>`；timer 驅動 + 版本/下載 UI 歸 P10/P12
- **C5**（PlayPage 實際視窗）：WPF 原始碼**沒有 `PlayNowPage`**；實際關的是 `FindWindow("StartUpDlgClass", "MapleStory")` + `PostMessage(WM_CLOSE)`。模組名維持 `play_page.rs`（語義），但 doc 要明記實際 class/title；timer 同 C4，service 只做一次呼叫
- **C6**（post_string scope）：除了 `PostString`（WM_CHAR + ASCII），還相依：`FindWindow` / `SetForegroundWindow` / `MapVirtualKey` / `ClientToScreen` / `GetCursorPos` / `SetCursorPos` / `GetClientRect` / `PostKey`。service 層提供 Win32 thin wrappers；業務編排（trad login 分支、Sleep 時機）歸 P10
- **C7**（find.rs 用 WMI）：WPF 用 `ManagementObjectSearcher` + `executablepath`，不是 `EnumProcesses`。Rust 照 WMI 路徑走（`wmi` crate 已在 Cargo.toml）
- **C8**（post_string ASCII-only）：WPF `PostString` 用 `ASCIIEncoding`，中文帳密不 work（原設計如此）。Rust 維持 ASCII-only parity，用 doc 鎖定而非升級到 UTF-16

#### Chunk 9.1 — `registry/game_path.rs` + `process/{error,find,kill}.rs`（registry read + 進程查詢 + pid kill）

- [x] D-step 1：scaffold — `services/registry/{mod,error,game_path}.rs` + `services/process/{mod,error,find,kill}.rs`；`services/mod.rs` 以 `#[cfg(target_os = "windows")]` 註冊兩個 module；Cargo.toml 0 新增依賴（winreg/wmi/windows 全已備）
- [x] D-step 2：`ProcessError` + `RegistryError` enums — `WmiInit` / `WmiConnect` / `WmiQuery { query, #[source] }` / `OpenProcess { pid, #[source] }` / `TerminateProcess { pid, #[source] }`；registry 端 `OpenKey` / `ReadValue`（帶 `hive\subkey[@value_name]` context）；`thiserror` derive + `#[source]` 保留 error chain
- [x] D-step 3：`services/registry/{mod,game_path}.rs` — `read_game_path(hive: Hive, subkey, value_name) -> Result<Option<String>, RegistryError>`；`Hive::{CurrentUser,LocalMachine}` 帶 `as_reg_key` / `display_name`；missing key / missing value / 空字串 → `Ok(None)` parity with `ModifyRegistry.Read` L73-99；另備 `read_raw_value<T: FromRegValue>` 逃生門
- [x] D-step 4：`services/process/find.rs` — `find_processes_by_name(name: &str) -> Result<Vec<ProcessInfo>, ProcessError>` 用 `wmi::{COMLibrary, WMIConnection}` + `SELECT ProcessId, Name, ExecutablePath FROM Win32_Process WHERE Name = '?'`；`ProcessInfo { pid, name, executable_path: Option<PathBuf> }`；單引號 input 回空（WQL 注入防線）
- [x] D-step 5：`services/process/kill.rs` — `kill_process(pid: u32) -> Result<(), ProcessError>` 用 `OpenProcess(PROCESS_TERMINATE, false, pid)` + `TerminateProcess(handle, 1)` + `CloseHandle`（三個路徑都 close）；exit-code 1 偏離 .NET `-1` 作 doc 說明
- [x] D-step 6：module docs — `services/process/mod.rs` 9.1/9.2/9.3 chunk 表 + timer 所有權歸 P10 說明；`services/registry/mod.rs` 只讀 + Hive 設計理由；每檔 WPF 行號對應表
- [x] D-step 7：unit tests — `quote_in_name_returns_empty` / `process_info_equality_rejects_path_casing_sloppiness` / `kill_pid_zero_errors_on_open_not_terminate` / `kill_implausible_pid_errors_on_open` / `read_known_present_value_returns_some`（HKCU\Environment@TEMP）/ `read_missing_subkey_returns_none` / `read_missing_value_in_existing_key_returns_none` / `read_hklm_known_value`（HKLM ProductName）/ `hive_display_name_matches_reg_syntax`
- [x] D-step 8：integration test `tests/process_find_kill.rs`（`#[cfg(target_os = "windows")]`）— `find_processes_by_name_finds_our_spawned_cmd` / `kill_process_terminates_spawned_cmd` / `find_then_kill_round_trip` / `kill_nonexistent_pid_surfaces_open_process_error`（4/4）；spawn 用 `cmd /c ping -n 30 127.0.0.1 -w 1000`（避開 `timeout` stdin 已關閉時立刻退出的坑）
- [x] D-step 9：quality gates 全綠 — `cargo fmt --check` ✓ / `cargo clippy --all-targets -- -D warnings` ✓ / `cargo test --lib` 409/409 / `cargo test --test process_find_kill` 4/4 / `cargo test --test updater` 8/8 / `cargo test --test game_locale_remulator` 6/6 / `cargo test --test storage_legacy --features test-fixtures` 9/9 / `cargo test --tests` 全綠 / `RUSTDOCFLAGS=-D warnings cargo doc --no-deps --document-private-items` ✓
- [x] D-step 10：commit `feat(next): add registry game_path + process find/kill (P9 chunk 9.1)` — amended into `75774ed`（pre-amend `cb5db2b`；Todo.md 1-step drift 同 P7/P8 模式）

##### 9.1 review follow-up — amended into `75774ed`

- [x] R9.1-1：`kill.rs` exit code `1` → `u32::MAX`（== .NET `Process.Kill()` `-1` bit-equivalent）；module doc `# Exit code semantics` 整段更新；integration test 加 `ExitStatus::code() == Some(-1)` 斷言鎖防 regression
- [x] R9.1-2：`game_path.rs` module doc `# Unicode` 段修正 — winreg 遇無效 UTF-16 是 `io::ErrorKind::InvalidData` 直接 surface 為 `RegistryError::ReadValue`，不是靜默 lossy 轉換
- [x] R9.1-3：砍 `read_raw_value`（投機 API、`#[allow(dead_code)]`、無 consumer）+ `use winreg::types::FromRegValue`；YAGNI 對齊 SRP
- [x] R9.1-4：`find_processes_by_name` 與 `kill_process` 分別補 `# Async runtime guidance`（對齊 P8.2 R8.2-4 `launch_via_lr` 模式）
- [x] R9.1-5：`find_processes_by_name` 補 `# COM apartment mode` 段（`CoInitializeEx` 與 `APARTMENTTHREADED` UI thread 衝突說明 → `ProcessError::WmiInit`）
- [x] R9.1-6：quality gates 全綠 — fmt / clippy -D warnings / lib 409/409 / process_find_kill 4/4 / 其他整合測試無 regression / rustdoc -D warnings ✓

#### Chunk 9.2 — `process/{patcher,play_page}.rs`（Patcher 一次呼叫 + PlayPage 視窗一次關閉）

##### 9.2 pre-flight decisions（2026-04-17）— Q1-Q5 全確認

- **Q1=A3**：`check_and_kill_patcher` 回 `Result<Vec<u32>, ProcessError>`（killed pids；`!is_empty()` == WPF `found`；caller 可 log）— 比 `Option<u32>` 誠實，比 `bool` 豐富
- **Q2=B2**：per-pid kill best-effort（失敗 silently skip，對齊 WPF nested `try/catch {}`）；`find_processes_by_name` 失敗 fail-fast（WMI 壞掉是系統級問題）
- **Q3=C1**：新增 `ProcessError::PostMessage { hwnd: isize, #[source] source: windows::core::Error }`（不重用現有 variant、不吞錯）
- **Q4=D1**：`to_wide_null` 抽到 `services/process/mod.rs` 當 `pub(crate)`（所有 process/ 內模組共用，locale_remulator 的 copy 暫留 — 未來第三 caller 出現才整併到 `services/util/`）
- **Q5=E1**：patcher 跳 end-to-end integration（spawn 假 Patcher.exe 成本高）；play_page 只做 `Ok(_)` smoke test（不 strict-assert `false`，避免誤關開發者 live session）

- [x] D-step 1：scaffold — `services/process/patcher.rs` + `services/process/play_page.rs` 新增；`process/mod.rs` 加 `pub mod patcher` / `pub mod play_page` + 私有 `pub(crate) fn to_wide_null`；Win32 features `Win32_UI_WindowsAndMessaging` 已有，0 新增 Cargo 依賴
- [x] D-step 2：`ProcessError::PostMessage { hwnd: isize, #[source] source: windows::core::Error }` 新增 + doc table 多一行
- [x] D-step 3：`patcher::check_and_kill_patcher(game_path: &Path) -> Result<Vec<u32>, ProcessError>` — `PATCHER_EXE_NAME = "Patcher.exe"`；`game_path.parent()` None → `Ok(Vec::new())` 短路（不打 WMI）；`find_processes_by_name(PATCHER_EXE_NAME)` → `matches_expected_path` 過濾 → `kill_process` best-effort；**DI 變體** `check_and_kill_patcher_with<F, K>` 讓 unit test 可注入 fake find + kill（對齊 P7 `check_update_at` 模式）
- [x] D-step 4：`play_page::close_play_window() -> Result<bool, ProcessError>` — `PLAY_WINDOW_CLASS = "StartUpDlgClass"` / `PLAY_WINDOW_TITLE = "MapleStory"` 公開常量鎖住 WPF 字面值；`FindWindowW` → `Ok(HWND)` 且 `!is_invalid()` → `PostMessageW(WM_CLOSE)` → `Ok(true)`；`Ok(invalid)` 或 `Err(_)` → `Ok(false)`（對齊 WPF `hWnd == IntPtr.Zero` 分支 + `try/catch {}`）；`PostMessage` 失敗 → `Err(ProcessError::PostMessage)`（不吞錯，對齊 Q3=C1）
- [x] D-step 5：module docs — `patcher.rs` WPF L2455-2477 C# source 嵌入 + Q2=B2 best-effort 說明 + # Async runtime guidance；`play_page.rs` WPF L2443-2453 C# source 嵌入 + 三種回傳情境說明 + `StartUpDlgClass` / `MapleStory` 字面值鎖定 + # Async runtime guidance；`process/mod.rs` chunk 表微調、`to_wide_null` helper 的 SRP/DRY 設計說明（未來整併閾值 = 第三 caller）
- [x] D-step 6：unit tests — `patcher`：`matches_expected_path_exact_match` / `matches_expected_path_different_directory` / `matches_expected_path_none_executable_path_is_false` / `game_path_without_parent_returns_empty`（含 short-circuit 斷言） / `kills_only_matching_processes` / `best_effort_skips_kill_failures` / `find_failure_propagates` / `empty_process_list_returns_empty_kill_list`（8 tests）；`play_page`：`window_class_literal_matches_wpf` / `window_title_literal_matches_wpf`（2 tests）；`process/mod.rs` 的 `to_wide_null`：`to_wide_null_terminates_with_zero` / `to_wide_null_empty_string_is_just_nul`（2 tests）
- [x] D-step 7：integration test `tests/process_find_kill.rs` — 追加 `check_and_kill_patcher_no_patcher_running_returns_empty`（production WMI 路徑 smoke）+ `close_play_window_smoke_returns_ok`（`Ok(_)` 斷言而非 `== Ok(false)`，避免誤關開發者 live session）；既有 4 個 P9.1 測試沿用；合計 6/6
- [x] D-step 8：quality gates 全綠 — `cargo fmt --check` ✓ / `cargo clippy --all-targets -- -D warnings` ✓（`collapsible_if` 一個提示已修正） / `cargo test --lib` 421/421（9.1 的 409 基礎 + 9.2 的 +12 新測試）/ `cargo test --test process_find_kill` 6/6 / `cargo test --test updater` 8/8 / `cargo test --test game_locale_remulator` 6/6 / `cargo test --test storage_legacy --features test-fixtures` 9/9 / `RUSTDOCFLAGS=-D warnings cargo doc --no-deps --document-private-items` ✓（`redundant-explicit-links` 兩個提示已修正）
- [x] D-step 9：commit `feat(next): add patcher kill + play_page close (P9 chunk 9.2)` — amended into `104dbb8`（pre-amend `c6d5a22`；Todo.md 1-step drift 沿用 P7/P8/P9.1 模式）

##### 9.2 review follow-up — amended into `104dbb8`

- [x] R9.2-1（medium）：`check_and_kill_patcher` fn doc 「short-circuit」描述修正 — `Path::parent()` 對 empty/pure-root path 回 `None`（觸發短路），對 bare filename 回 `Some("")`（仍打 WMI，但因 `Win32_Process.ExecutablePath` 絕對路徑而自然回空），文字精準化、不過度承諾
- [x] R9.2-2（low）：`ProcessError::PostMessage::hwnd` 型別 `isize` → `usize`（HWND 為 pointer-sized opaque，usize 是更窄的語意表達；Display 行為不變；`play_page.rs` cast 同步換）
- [x] R9.2-3（medium）：`check_and_kill_patcher` / `close_play_window` 兩個 fn 各補 `# Async runtime guidance` 段（模組層已有，但 rustdoc fn 頁面 + IDE hover 需要 fn 層級才顯示，對齊 P9.1 `find_processes_by_name` / `kill_process` 模式）
- [x] R9.2-4（low）：`patcher.rs` unit test `find_failure_propagates` 加 inline comment 說明 `ProcessError::OpenProcess` 只是 transport（因 `wmi::WMIError` 沒有公開 unit-test constructor），並非語意宣稱 find-side 會回這個 variant
- [x] R9.2-5（low）：`process/mod.rs` 的 `to_wide_null` visibility 由 `pub(crate)` 收回為 default private（`fn`）— 只有 process/ 的子模組透過 `super::to_wide_null` 使用；crate 內其他模組無 legitimate use case，收窄更貼近 Q4=D1 的「internal to process/」設計意圖

#### Chunk 9.3 — `process/post_string.rs`（Win32 thin wrappers for auto-paste）

##### 9.3 pre-flight decisions（2026-04-18）— Q1-Q7 全確認

- **Q1=scope_paste_only**：P9.3 只收 auto-paste 主流程那 9 個 fn（`FindWindow` / `SetForegroundWindow` / `MapVirtualKeyW` / `PostString` / `PostKey` / `PostMessage` / `ClientToScreen` / `GetCursorPos` / `SetCursorPos` / `GetClientAreaSize`）。**Out of scope**：sysmenu (`GetWindowLong/SetWindowLong` MainWindow L202-205 → Tauri 接管) / window composition (`SetWindowCompositionAttribute` → CSS / `tauri-plugin-window-vibrancy`) / `AttachConsole` (Tauri 自管) / process introspection (`GetCurrentProcess` / `GetModuleHandle` / `IsWow64Process` / `GetBinaryType` / `GetWindowThreadProcessId` → 將來若需要走獨立 `services/process/info.rs` chunk)
- **Q2=bug_fix_correct**：PostKey lParam = `(MapVirtualKey(vk, 0) as u32) << 16 | 1`（WPF L34 是 C# operator-precedence 意外 `<< 17`，非設計）。Module doc 標 "Diverges from WPF L34 (operator-precedence bug)"
- **Q3=ascii_surface_err**：PostString 非 ASCII 字元 → `Err(ProcessError::NonAscii { offset, ch })`（不吞錯，沿 P9.2 Q3=C1 規則）
- **Q4=hwnd_nonzero**：API parameter 用 `WindowHandle(NonZeroUsize)` newtype（type-safe non-null）；error variant 仍 `usize`（post-mortem 識別不適用 NonZero）
- **Q5=pr_wrap_domain**：`Point { x: i32, y: i32 }` + `Size { width: i32, height: i32 }` newtype；RECT 完全藏起來；`#[derive(Serialize, Deserialize)]` 給 P10 IPC
- **Q6=chunk_single**：P9.3 一次出（13 D-steps）
- **Q7=tests_full + full_pragmatic**：medium baseline（unit + cursor_pos round-trip + `find_window("Shell_TrayWnd")` smoke）+ `#[ignore]` notepad spawn smoke（不讀回字元，信任 Win32 PostMessage 契約；Win11 graceful skip）

- [x] D-step 1：scaffold — `services/process/post_string.rs` 新增；`process/mod.rs` 加 `pub mod post_string`；Cargo.toml 加 `Win32_UI_Input_KeyboardAndMouse` + `Win32_Graphics_Gdi`（後者為 `ClientToScreen`，windows-0.58 的 API 位置是 Gdi 而非 WindowsAndMessaging）
- [x] D-step 2：`ProcessError::NonAscii { offset: usize, ch: char }` variant 新增 + `Win32Call { name: &'static str, source: windows::core::Error }` variant 新增（for `GetClientRect` / `ClientToScreen` 的 must-succeed 失敗）+ WPF mapping table 多兩列
- [x] D-step 3：newtypes — `WindowHandle(NonZeroUsize)` + `Point { x: i32, y: i32 }` + `Size { width: i32, height: i32 }`；`pub(crate) from_raw(HWND) -> Option<Self>` / `pub(crate) as_hwnd() -> HWND` / `pub as_raw() -> usize`（對稱 P9.2 R9.2-2 `usize`-for-logging 共識）；Point/Size `#[derive(Serialize, Deserialize, Hash)]`
- [x] D-step 4：`find_window(class: Option<&str>, title: Option<&str>) -> Option<WindowHandle>`（**drift**：FindWindowW 的 `NULL` 返回與內部失敗不可區分 → Q5 hybrid 決策歸「best-effort」，直接回 `Option`）+ `set_foreground_window(handle: WindowHandle) -> bool`
- [x] D-step 5：`get_client_area_size(handle: WindowHandle) -> Result<Size, ProcessError::Win32Call>` + `client_to_screen(handle: WindowHandle, point: Point) -> Result<Point, ProcessError::Win32Call>`（`ClientToScreen` 回 Win32 `BOOL`，手動經 `windows::core::Error::from_win32()` 合成 source）
- [x] D-step 6：`get_cursor_pos() -> Option<Point>` + `set_cursor_pos(point: Point) -> bool`（**drift**：cursor save/restore 失敗是美學問題非資料損失，Q5 hybrid 歸「best-effort」→ `Option` + `bool` 而非 `Result`）
- [x] D-step 7：`post_string(handle: WindowHandle, s: &str) -> Result<(), ProcessError>` — 用 `str::char_indices` 預檢，第一個非 ASCII 字元 surface `NonAscii { offset, ch }` 即中斷、不發出任何 `WM_CHAR`（原子性只限於 content-level 失敗；`PostMessageW` 中段失敗已 enqueued 的 byte 不回滾——本就無法 unsend）
- [x] D-step 8：`post_key(handle: WindowHandle, msg: u32, vk: u8) -> Result<(), ProcessError>`（lParam 透過私有 `compute_post_key_lparam(vk) -> isize` 計算 `(scan_code << 16) | 1`，doc 標 Q2 divergence 與 C# 運算優先級意外解析）+ `post_message_raw(handle: WindowHandle, msg: u32, wparam: usize, lparam: isize) -> Result<(), ProcessError>`
- [x] D-step 9：module docs — WPF `WindowsAPI.cs` L11-86 對應表 + Out of scope 表 + Q1-Q7 設計決策段 + Error surface must-succeed vs best-effort 段 + Async runtime guidance 段（rustdoc 私有 item intra-doc link 用 backtick 而非 `[]`-link，避開 `private-intra-doc-links` lint）
- [x] D-step 10：unit tests — 9 條（`WindowHandle::from_raw(NULL)` / round-trip / Point+Size serialize 形狀 + JSON 來回 / `compute_post_key_lparam` repeat-count 結構斷言 + WPF bug divergence 斷言 / `ProcessError::NonAscii` Display 含 offset + char）；模組內全綠、`process/mod.rs::wide_tests` 已覆蓋 `to_wide_null` 無需重複
- [x] D-step 11：integration test `tests/process_post_string.rs`（Windows-only） — 3 baseline 全綠：`find_window_locates_shell_tray` / `get_client_area_size_returns_positive_dimensions_for_shell_tray`（順手驗 `client_to_screen((0,0))`）/ `cursor_round_trips_within_a_pixel`（原位置 ±1px，還原後再斷言避免 panic 遺留滑鼠位移，±2px tolerance for DPI / cursor-snap）；`#[ignore] spawn_notepad_full_paste_smoke`：spawn notepad → 5s poll `find_window(Some("Notepad"), None)` → `set_foreground_window` → `post_string("abc") Ok` → `post_key(WM_KEYDOWN, VK_END) Ok` → `ChildGuard` Drop 回收；VK_END 對齊 `MainWindow.xaml.cs` L2222；不讀回字元（Q7 contract）
- [x] D-step 12：quality gates 全綠 — `cargo fmt --check` / `cargo clippy --all-targets -- -D warnings` / `cargo test --lib`（430 passed 含 P9.3 新增 9 條）/ `cargo test --tests`（全 integration files 0 failed，含 `process_post_string` 3 baseline + 1 ignored）/ `cargo doc -D warnings`（順手修一處 `error.rs` intra-doc link：D11 re-export 後 `crate::services::process::post_string` 變成 fn+module 同名，拉到具名 fn 路徑 `crate::services::process::get_cursor_pos` 消歧）
- [x] **bonus**：`services/process/mod.rs` 補 `pub use post_string::{ find_window, set_foreground_window, get_client_area_size, client_to_screen, get_cursor_pos, set_cursor_pos, post_string, post_key, post_message_raw, Point, Size, WindowHandle }`（對齊 P9.2 `play_page` 的 explicit re-export 風格；P10 command layer 整批用得到）
- [x] D-step 13：commit `feat(next): add auto-paste Win32 wrappers (P9 chunk 9.3)` — `a1c1607`

- [x] **P9 總驗收**：`services/process/*.rs` + `services/registry/game_path.rs` 對齊 WPF 對應點，timer 驅動保留給 P10 Tauri command layer

### P10 — Tauri commands + IPC 型別

#### P10 pre-flight decisions（2026-04-18）— Q1-Q7 全確認

- **Q1=B（3 sub-chunks）**：10.1 infra / 10.2 auth+account+otp / 10.3 launcher+storage+config+update+system。**Rationale**：infra（error DTO + AppState + specta wiring）變化面大且跨所有命令，獨立一 chunk；剩下依業務親緣度分兩批
- **Q2=A（單一 `AppState`）**：`Builder::manage(AppState { http_client, storage_root, session })`，command 簽名用 `State<'_, AppState>` 注入。單一 state 比多 resource 簡單
- **Q3=C（thin `CommandError` DTO）**：`CommandError { code, message, details }` IPC 層結構固定，domain errors 實作 `Into<CommandError>`；前端 i18n 靠 `code`、log 靠 `message`、追蹤細節靠 `details`
- **Q4=A（`tauri-specta v2` + `specta v2`）**：自動產 `bindings.ts`（DRY）；debug build 時 runtime 重新生成
- **Q5=A（`tokio::task::spawn_blocking`）**：command 內包同步 Win32 呼叫；service 層維持 sync 保留 testability（P8.2 R8.2-4 / P9.1 R9.1-4 / P9.2 R9.2-3 累積的共識）
- **Q6=A（1 feat commit per sub-chunk）**：10.1 / 10.2 / 10.3 各一 commit，follow P7/P8/P9 pattern
- **Q7（10.1 scope）**：infrastructure + 完整 domain `CommandError::From` impls（7 個：LoginError / StorageError / ConfigError / ProcessError / RegistryError / GameError / UpdaterError）+ 1-2 smoke commands（`version` + `ping`）驗整條 IPC 通路 + specta export 真的把 `bindings.ts` 生出來

#### Chunk 10.1 — IPC infrastructure + smoke commands

##### 10.1 pre-flight decisions（2026-04-18）— Q1-Q8 全確認

- **Q1（AppState shape）= minimal+session**：`AppState { http_client: reqwest::Client, storage_root: PathBuf, session: RwLock<Option<Session>> }`，`Session` 10.1 先放空 placeholder struct（`#[derive(Default)]`，無欄位），P10.2 填真實 auth session 欄位（avatar / token / account_list etc.）
- **Q2（Lock type）= `tokio::sync::RwLock`**：session 可多讀（AccountList / OTP 頁同時讀）、單寫（login / logout）；全 AppState 由 `Builder::manage` 自動包 `Arc`
- **Q3（Code naming）= `snake_case.dot_separated`**：`auth.invalid_credentials` / `storage.io_failed` / `network.timeout` 等；前 prefix 對應 domain，方便前端 i18n key 對映；module doc 明列 convention
- **Q4（details 欄位）= `Option<serde_json::Value>`**：保彈性，domain 可塞結構化 context（http_status / path / pid / retry_hint etc.），前端 union type 解析
- **Q5（Specta 整合風格）= `tauri_specta::Builder<Wry>` + `collect_commands!`**：idiomatic v2 寫法；`#[tauri::command]` + `#[specta::specta]` 雙標註；commands 經 `builder.invoke_handler()` 注入 Tauri
- **Q6（Bindings 輸出路徑）= `beanfun-next/src/types/bindings.ts`**：不超出 beanfun-next（取代舊實作）；`types/` 是新目錄，與 `vite-env.d.ts` / `main.ts` 同層；bindings.ts commit 進 repo（Vue 組件開發期需要 TS 型別；debug build 覆寫產生 diff 時手動 commit 或 git hook 處理後續決定）
- **Q7（smoke commands）= `version` + `ping`**：`version() -> String`（回 `CARGO_PKG_VERSION`，純同步 read 驗 sync command 通路）+ `ping() -> Result<String, CommandError>`（`spawn_blocking` 包 `std::thread::sleep(50ms)` 驗 async+blocking 通路 + Result variant）
- **Q8（Specta export 時機）= `#[cfg(debug_assertions)]` at `run()` 開頭**：每次 debug build 啟動時 `builder.export(Typescript::default(), "../src/types/bindings.ts")`；release build 不含 export 代碼（specta-typescript 仍是 dep，但 export 調用 cfg-gated）；header 加 `// AUTO-GENERATED by tauri-specta — DO NOT EDIT`

- [x] D-step 1：Cargo deps — **rc.21 spike（降版，非 rc.24）**：`specta rc.24`（2026-03-30）內部用 unstable `fmt::from_fn`（rust-lang/rust#117729 未 stable），在 stable Rust 1.92 編不過 → 降到 `tauri-specta =2.0.0-rc.21` + `specta =2.0.0-rc.22` + `specta-typescript =0.0.9`（皆 pre-`from_fn` churn，編過）；`tauri` features 加 `"specta"`；Cargo.toml 加段落註解記錄降版原因 + rust-lang/rust#146099 stabilization watchpoint（未來 stable 後再升）。API smoke 查驗 rc.21 `Builder::{new,commands,invoke_handler,mount_events,export}` + `collect_commands!` macro 全在 → D7/D8 計畫無需改
- [x] D-step 2：scaffold `src/commands/{mod,error,state,system}.rs` + `lib.rs` 加 `pub mod commands;`（先空實作讓 skeleton 編得過）
- [x] D-step 3：`CommandError { code: String, message: String, details: Option<serde_json::Value> }` in `commands/error.rs`（`#[derive(Serialize, specta::Type)]`；`thiserror` 不用因為這是 IPC DTO 不是 domain error）+ helper `CommandError::new(code, message)` / `.with_details(value)` builder + `Display` + `std::error::Error` + 7 unit tests（lib 430→437）
- [x] D-step 4：`CommandError::From` for 7 個 domain errors：`LoginError`（42 variants） / `StorageError`（8） / `ConfigError`（4） / `ProcessError`（8） / `RegistryError`（2） / `GameError`（7） / `UpdaterError`（4）；**Q8.D4 決策：A 細粒度（每 variant 一 code）**，compiler 強制 match 全覆蓋；code 採 `<domain>.<variant_snake_case>`（e.g. `auth.missing_view_state` / `game.shellexecute_failed` / `process.non_ascii`）；variant 有結構化欄位就塞 `details`（URL / pid / path / hive-subkey / shellexecute_code 等）；`reqwest::Error` / `serde_json::Error` / `io::Error` 等非 Serialize 的內嵌類型用 display 字串 + 可從 API 取到的 flags（`is_timeout` / `line/column` / `io_kind`）；helper fns `io_kind_str` / `reqwest_details` / `serde_json_details` 避免重複 detail 擷取邏輯
- [x] D-step 5：`AppState` minimal shell — `AppState { storage_root: PathBuf, session: RwLock<Option<Session>> }` + empty `Session` placeholder（10.2 填 avatar / token / account_list）；`AppState::new(storage_root)` **infallible**（`http_client` 延 P10.2 引入，避免 P10.1 smoke commands 無用 dead_code warning）；3 unit tests（new / session lifecycle / storage_root 儲存）
- [x] D-step 6：smoke commands in `commands/system.rs` — `#[tauri::command] #[specta::specta] pub fn version() -> VersionInfo { VersionInfo { app: env!("CARGO_PKG_VERSION"), tauri: tauri::VERSION } }`（驗 sync command 通路 + struct return DTO）+ `#[tauri::command] #[specta::specta] pub async fn ping(message: String) -> Result<String, CommandError>`（`spawn_blocking(|| { sleep(60ms); message }).await.map_err(→ system.spawn_blocking_failed)`，驗 async+blocking 通路 + Result + 參數 serde）；3 unit tests（version 欄位 + ping echo + ping Unicode）
- [x] D-step 7：`lib.rs` 整合 — `commands::build_specta_builder::<tauri::Wry>()` helper（D7 DRY 決策：單一 `collect_commands!` 呼叫點）+ `resolve_storage_root()`（Windows `%APPDATA%\Beanfun` / 非 Windows `temp_dir().join("Beanfun")` fallback，失敗直接 `CommandError`）+ `pub fn run()` 依序 resolve_storage_root → `AppState::new` → `build_specta_builder` → `export_specta_bindings` → `tauri::Builder::default().plugin(opener).manage(state).invoke_handler(...)`；D7 sub-decisions: D1=B 移除 `greet` scaffold + cleanup `App.vue` greet UI / D2=A `Err` 直接中止 / D3=B `AppState` 初於 `run()` 起頭 / D4=B 抽 helper
- [x] D-step 8：specta export — `#[cfg(debug_assertions)] fn export_specta_bindings<R: tauri::Runtime>(builder: &tauri_specta::Builder<R>)` 寫到 `<CARGO_MANIFEST_DIR>/../src/types/bindings.ts`，release build 為 no-op stub；export 失敗**非致命**（app 用既存 bindings.ts 繼續 boot，只寫 stderr，避免 `Program Files` 安裝路徑寫入失敗造成 release app 打不開）
- [x] D-step 9：module docs — `commands/mod.rs` 頂層 IPC 架構圖 + chunk layout + add-command howto；`error.rs` 7 domain code 對照表 + `system.*` 自製 codes section；`state.rs` AppState 生命週期 + RwLock 守則；`system.rs` smoke 設計 rationale（為何 `ping` 用 `spawn_blocking` / 為何 `version` 回 struct）；`lib.rs` boot sequence 流程圖
- [x] D-step 10：unit tests — 7 domain `From` impls 共 20 條（每 domain 抽代表 variant，涵蓋 `Option<String>` 欄位 / 非 Serialize 內嵌 / 結構化 details JSON）+ AppState 3 條 + system 3 條 = **31 新 tests**（lib 430→461，全綠 0 regression）
- [x] D-step 11：**scope 降級** — 原訂 `tests/ipc_smoke.rs`（`mock_invoke` + `bindings.ts` grep）在 Windows 上 link 時會拉入 `tauri-runtime-wry` → `webview2-com-sys` 的 `WebView2Loader.dll` 靜態依賴（非 delay-load），test binary load 階段 crash `STATUS_ENTRYPOINT_NOT_FOUND`；嘗試過 ①獨立 integration test binary ②generic `Builder<MockRuntime>` ③搬進 lib-test `#[cfg(test)]` 三條路徑全失敗（③還會汙染原 461 tests 的 lib binary）。根因：只要 test binary 靜態實體化 `tauri_specta::Builder<R>`（any `R`）就引入 Wry 符號圖；和 `cargo tauri dev` production path 的差別是後者已 setup PATH 讓 DLL loader 找得到。**最終決策**：把 D11 降級為「驗證**已 commit** `bindings.ts` 檔案內容」的 file-level test — `commands::bindings_file_tests::bindings_file_contains_all_p101_symbols`，只讀 `<CARGO_MANIFEST_DIR>/../src/types/bindings.ts` + filter 出 `export`-開頭 lines + grep `version` / `ping` / `CommandError` / `VersionInfo` 四個 symbol；fresh-clone 檔案缺失時 skip 不 fail（eprintln 提示 `cargo tauri dev` 重生）；comment 裡 mentioning symbol 不會被誤 match（只看 export lines）；drift 場景手測已驗真的 fail。Lib test 461→462，無 Wry 符號汙染
- [x] D-step 12：quality gates 全綠 — `cargo fmt --check` 綠（D10 test 中一處換行補跑 `cargo fmt` 修正） / `cargo clippy --all-targets -- -D warnings` 綠（D10 test 中 `StorageError::Dpapi.operation` 欄位是 `&'static str`，test 的 `"Protect".into()` 多餘，移掉 `.into()`） / `cargo test --lib` 462 passed（原 461 + D11 `bindings_file_tests` 1 條） / `cargo test --tests` 既有 9 個 integration files 全綠（0 regression） / `cargo doc -D warnings` 綠（修三處 broken intra-doc link：`state.rs` 兩處 `[tempfile::TempDir]` / `mod.rs` 一處 `[bindings_file_tests]` 指向 `#[cfg(test)]` mod，改 backtick plain text；另修一處 pub→priv link `[crate::export_specta_bindings]` 改 backtick）。`cargo run` 驗 bindings.ts 改由 **P10.2 開工時第一次 `cargo tauri dev` 自然 trigger** — D8 本身就是 runtime export，合併後首次 dev 啟動自動寫檔，屆時 `bindings_file_tests` 從「fresh-clone skip」升級為「真驗 symbols」；D12 不為此刻意啟動 GUI event loop
- [x] D-step 13：commit `feat(next): add Tauri command IPC infrastructure (P10 chunk 10.1)` — **hash `ee71c29`**（9 files changed, +1816 / -36；`commands/{mod,error,state,system}.rs` 新建 + `lib.rs` / `App.vue` / `Cargo.{toml,lock}` / `Todo.md` 修改；co-author: cursor 未夾帶）

#### Chunk 10.2 — auth + account + otp commands

##### 10.2 pre-flight decisions（2026-04-16）— Q1-Q8 全確認

- **Q1（Sub-chunking）= A（單 commit）**：P10.2 一次 commit `feat(next): add auth+account+otp commands (P10 chunk 10.2)`；對齊 P10 pre-flight Q6=A「1 feat commit per sub-chunk」慣例，不再下鑽拆 10.2a/10.2b
- **Q2（AppState shape）= B（合併 AuthContext）**：`AppState.auth: RwLock<Option<AuthContext { client: BeanfunClient, session: Session }>>`；單一鎖保 client + session 一致性（避免 atomicity 漏洞：session cleared 但 client 還帶舊 cookie jar）；**同步移除** P10.1 留的 `commands::state::Session` placeholder，改用 `services::beanfun::Session`（已有 `zeroize::Zeroize` + `Debug` redact）
- **Q3（Session-required 前置）= A（require_auth helper）**：`commands/session.rs::require_auth(&AppState) -> Result<(BeanfunClient, Session), CommandError>`，未登入回 `CommandError { code: "auth.session_required" }`；DRY（每 cmd 一行 guard）+ SRP（session 檢查不外洩到業務 cmd）
- **Q4（Domain → IPC DTO 策略）= C（Hybrid）**：純 data 走 A（domain struct 直接 `#[derive(specta::Type)]`，類比 `serde::Serialize` 已在 domain 加了，算 cross-layer trait 非污染）→ `ServiceAccount` / `AccountListResult` / `AmountLimitNotice` / `QrLoginInit` / `QrPollOutcome` / `VerifyPageInfo` / `VerifyOutcome` / `TotpChallenge` 等純 data；含 secret / binary 走 B（DTO shadow + `From` impl）→ `Session` 只導 `SessionInfo { region, account_id, service_code, service_region }` safe subset（**skey / web_token 不過 IPC**）；`Credentials` **絕不導出**（zeroize policy），IPC 入口接 `{ account, password }` 兩 `String`、cmd 內部組 `Credentials` 立即 drop；captcha / QR image bytes 導成 base64 `String`（frontend `<img src="data:image/png;base64,...">`，IPC JSON 友善、TS 不處理裸 `Vec<u8>`）
- **Q5（QR polling）= B（split start/check）**：`login_qr_start` → 呼叫 `init_qr_login` 回 `QrLoginInit`（QR image 轉 base64）；`login_qr_check(init)` → 呼叫 `poll_qr_login_status`；`QrPollOutcome::Success` 時**同 command 內**串 `finalize_qr_login` + set AuthContext，避免新增 `login_qr_finalize` 第 3 個 cmd；frontend 持 `QrLoginInit` + setInterval 驅 polling cadence，backend 不另開 `qr_pending` slot（YAGNI）；對齊既有 service 層三步拆分（Todo.md L310-335）
- **Q6（Verify / AdvanceCheck flow）= A（frontend 驅動）**：login cmd 偵測 AdvanceCheck → 回 `CommandError { code: "auth.advance_check_required", details: { url } }` → frontend 驅 3-step（`get_verify_page_info` / `get_verify_captcha` / `submit_verify`）→ Success 後 frontend 重送 credentials retry login；**不在 backend 久放明文密碼**（遵 `Credentials::ZeroizeOnDrop` policy；backend 無 long-lived secret slot），安全性優先於少 1 次 round-trip
- **Q7（Account deep flow scope）= A（只做 connected game）**：P10.2 交付 `add_service_account`（connected game 新增遊戲帳號）+ `change_display_name` + `get_accounts` / `get_contract` / `get_email` / `get_remain_point` / `refresh`；`unconnected_game_add_account` / `unconnected_game_change_password` 系列延 P12（需 UI 決定 prompt 順序 / 確認訊息 UX，scope 先可控）
- **Q8（tauri-specta events）= A（無 event）**：P10.2 全 command round-trip；WPF 原實作為 Windows Forms dispatcher 同步 update（無 event bus），對齊舊功能不引入 push-based 機制；frontend Vue reactive + polling 足夠；P11/P12 真有需求再加 `SessionChanged` / `QrStatusChanged`
- **Q-risk1（TotpChallenge 如何過 IPC）= A（backend pending slot）**：`TotpChallenge` 內部含 secret（`session_key` = pSKey / `viewstate` = ASP.NET Base64 state），且所有欄位 `pub(crate)` 是 opaque 設計 → **不過 IPC**。改在 `AppState` 加 `pending_totp: RwLock<Option<PendingTotp { client, challenge }>>`，login cmd 遇 `LoginError::TotpRequired(challenge)` 時把 `(client, challenge)` 同時存入（保證 cookie jar 延續）；同步 surface `CommandError { code: "auth.totp_required", details: TotpChallengeInfo { totp_url, account_id } }`（safe subset）給前端；`login_totp` cmd 從 slot `read()` challenge + clone client（保留 pending 以便 wrong-OTP 重試），成功才 `write() = None` 清空
- **Q-risk2（`login_gamepass_complete` scope）= A（延 P12）**：`services/beanfun/` 目前**無** gamepass 相關 fn，舊 WPF 的 GamePass flow 是 `WebView2` UI-driven（user 登 Razer/MS 等 → 完成後抓 cookie）；backend API shape 取決於 `WebviewWindow` 抓 cookie 的確切機制 → P10.2 不做 `login_gamepass_complete`，延 P12 UI 配套時一起設計（跟 Q7 的 `unconnected_game_*` 同樣 scope-control 原則）。P10.2 的 `commands/auth.rs` 只出 `login_regular` / `login_totp` / QR family / verify family / logout

- [ ] D-step 1：AppState / AuthContext 改造 — 定義 `AuthContext { client: BeanfunClient, session: Session }` + 改 `AppState.auth: RwLock<Option<AuthContext>>`；移除 P10.1 `commands::state::Session` placeholder + `session` 欄位；3 unit tests（new / set-clear / take 原子性）
- [ ] D-step 2：`commands/session.rs::require_auth(&AppState) -> Result<(BeanfunClient, Session), CommandError>` helper（code `auth.session_required`）；BeanfunClient Arc-clone + Session derive Clone；3 unit tests（未登入 / 已登入 / code 形狀）
- [ ] D-step 3：DTO 骨架 — `commands/dto.rs`：`SessionInfo { region, account_id, service_code, service_region }` + `From<Session>`（**不含 skey/web_token**）+ `fn encode_png_base64(bytes: &[u8]) -> String` helper；文件化「純 data 直接 derive `specta::Type`」策略（實作分散到 D4-D10）；4 unit tests（SessionInfo 欄位 / secret absence / base64 round-trip / empty bytes）
- [ ] D-step 4：`commands/auth.rs` — regular family：`login_regular(region, account, password)` / `login_totp(code)`（`login_gamepass_complete` 延 P12，見 Q-risk2）；D4 順手 extend `AppState` 加 `pending_totp: RwLock<Option<PendingTotp { client, challenge }>>`（見 Q-risk1）+ `dto::TotpChallengeInfo { totp_url, account_id }` safe subset；成功 set AuthContext；`LoginError::TotpRequired` 特殊處理存 slot + surface safe subset；`LoginError::AdvanceCheckRequired` 走 P10.1 From impl（已驗確 code = `auth.advance_check_required`）；4+ unit tests
- [ ] D-step 5：`commands/auth.rs` — QR family：`login_qr_start`（→ `init_qr_login` → `QrLoginInit` + QR image base64）+ `login_qr_check`（→ `poll_qr_login_status`；`Success` 時 command 內部串 `finalize_qr_login` + set AuthContext）；`QrLoginInit` / `QrPollOutcome` 加 `specta::Type`；3+ unit tests
- [ ] D-step 6：`commands/auth.rs` — verify family：`get_verify_page_info` / `get_verify_captcha`（Vec<u8> → base64）/ `submit_verify`；`VerifyPageInfo` / `VerifyOutcome` 加 derive；3+ unit tests
- [ ] D-step 7：`commands/auth.rs` — `logout`：清 `AppState.auth`（check `services/beanfun` 有無 server-side logout 要對齊 WPF）；2 unit tests
- [x] D-step 8：`commands/account.rs` — base：`get_accounts` / `refresh`（皆 require_auth）；`AccountListResult` / `ServiceAccount` / `AmountLimitNotice` 加 `Serialize + specta::Type` derive；`AmountLimitNotice` 採 **adjacent tagging**（`#[serde(tag="kind", content="data", rename_all="snake_case")]`）以相容 `Other(String)` tuple variant；`list_accounts_internal` helper 統一 session-gating + service dispatch（DRY），`get_accounts` / `refresh` 各自一行委派（**refresh = get_accounts 語義別名**，分兩 cmd 保留前端呼叫點語意清晰）；2 unit tests；lib 492→494
- [x] D-step 9：`commands/account.rs` — management：`add_service_account(name)`（service_code/region 從 session 取；對齊 WPF `MainWindow.AddServiceAccount`）+ `change_display_name(new_name, account)`（`ServiceAccount` 加 `Deserialize` derive 供前端 echo；`game_code = "{service_code}_{service_region}"` 在 command 層組；對齊 WPF `MainWindow.ChangeServiceAccountDisplayName`）；unconnected_game_* 延 P12；session-gating 由 `require_auth`（D2 tests）守護，DRY 不重複；+1 unit test（`ServiceAccount` serde round-trip guard — Deserialize 不可被 `#[serde(skip)]` 靜默破壞）；lib 494→495
- [x] D-step 10：account info — **分三小步（D10a/b/c）對齊「service 層先完成才 command wrapper」層級紀律**；pre-flight 檢查發現 service 只有 `get_service_contract`，`get_email` / `get_remain_point` 未 port；user delegate（C）決定 A 方案補齊 service 層
  - D10a: `services/beanfun/account.rs::get_email(client, session)` — TW loader.ashx + Referer + regex `BeanFunBlock.LoggedInUserData.Email = "(.*)";BeanFunBlock.LoggedInUserData.MessageCount`；HK 直回 empty（對齊 WPF `BeanfunClient.cs` L243-259）；memoised `email_regex()`
  - D10b: `services/beanfun/account.rs::get_remain_point(client, session)` — `beanfun_block/generic_handlers/get_remain_point.ashx?webtoken=1` + regex `"RemainPoint" : "(.*)" \}` + `i32` parse；regex-miss / parse-fail 都回 `0`（對齊 WPF blanket catch → return 0）；memoised `remain_point_regex()`
  - D10c: `commands/account.rs::get_contract` / `get_email` / `get_remain_point` thin wrappers（皆 require_auth + 從 session 取 service_code/region）；account commands signature test 擴充 7 symbols
  - service 層 `pub use` re-export `get_email` + `get_remain_point`；`tests/account.rs` 補 6 條 integration tests（TW happy / TW regex miss / HK short-circuit / remain_point happy / miss / non-numeric）；lib 495 不變、integration 14→19
- [x] D-step 11：`commands/otp.rs::get_otp` — require_auth + forward `ServiceAccount` + 從 session 取 service_code/region（對齊 WPF `getOTP(sa)` + `add_service_account` / `change_display_name` 的 session-locked policy）；`ServiceAccount` Deserialize 已由 D9 備齊，無新增 derive；1 symbol-exists test（service 層 5-step pipeline 由 `tests/otp.rs` 既有 integration 覆蓋，command 層只走 require_auth + forward，session-gating 由 D2 tests 守護）；lib 495→496
- [x] D-step 12：`commands/mod.rs` 整合 — `collect_commands!` 掛入 P10.2 的 16 個 cmd（auth regular 2 + QR 2 + verify 3 + logout 1 + account base 2 + management 2 + info 3 + otp 1）+ P10.1 原 2 個（system::version / ping）= 18 總；`bindings_file_tests::REQUIRED_SYMBOLS` 從 4 擴充至 30 個 symbols（18 commands + 12 DTOs：CommandError / VersionInfo / SessionInfo / LoginRegion / TotpChallengeInfo / QrStart / QrStatus / VerifyPage / VerifyCaptcha / VerifySubmit / ServiceAccount / AccountListResult / AmountLimitNotice）；目前 `bindings.ts` 還未生成（fresh-clone skip path），D14 `cargo tauri dev` 後會真實 assert 所有 symbols；lib 496 不變
- [x] D-step 13：module docs — `commands/mod.rs` 頂層 design-principles 從 P10.1 5-bullet 擴充為 P10.2 7-bullet（含 hybrid DTO 策略 / pending slots / require_auth gating / atomic AuthContext）；chunk layout 表加 status 欄並標 10.2 done；`commands/auth.rs` 表頭從 forward-reference (D5/D6/D7) 升級為實裝描述；`account.rs` / `otp.rs` / `session.rs` / `dto.rs` / `state.rs` 既有 doc 已詳盡；`cargo doc -D warnings` 紅燈（多處 cross-link 對 private item / non-URL [wpf]: / 缺 disambiguator），fix 移到 D14 quality gate 統一處理
- [x] D-step 14：quality gates 全綠 —
  - `cargo fmt --all -- --check` ✓（修了 2 處：`LoginRegion` 多行 derive 收斂、`services/beanfun/mod.rs` re-export imports rewrap，皆 D3/D8 加 derives 後遺漏跑 fmt）
  - `cargo clippy --all-targets -- -D warnings` ✓（修了 4 處：`auth.rs` 3× `.err().expect()` → `expect_err`、`dto.rs` `chars().all(is_ascii)` → `is_ascii()`）
  - `cargo doc --no-deps --document-private-items` ✓（修了 21 處 doc link）：
    - 加 `commands/mod.rs` 模組級 `#![allow(rustdoc::private_intra_doc_links)]`，一次 cover 所有對 `pub(crate)` helper（`require_auth` / `list_accounts_internal` / `*_NOT_PENDING_CODE` / `*_NOT_STARTED_CODE` / `split_otp_digits`）的 doc link，11 處 private intra-doc lint 一次清掉（DRY：單一決定點，dev 仍可 navigate `--document-private-items` 文件）
    - 個別 fix：`account.rs` `AmountLimitNotice` 補 fully-qualified path、移除 `[wpf]: Beanfun/MainWindow.xaml.cs ...` 非 URL reference def（會破壞 markdown parser 連帶整段 `[sesh]:`/`[le]:` 失效）；`auth.rs` `WrongAuthInfo`/`ServerMessage` 補 `VerifyOutcome::` prefix、redundant explicit link target 兩處改 implicit、`logout` 加 `()` disambiguator；`error.rs` `[ErrorKind]` → `[`io::ErrorKind`]`；`otp.rs` `services::beanfun::get_otp` → `crate::services::beanfun::get_otp`、移除跨 doc-string scope 失效的 `[svc]` ref；`lib.rs` `[std::env]` → `[std::env!]` macro disambiguator
  - `cargo test --lib` ✓ 496 passed
  - `cargo test --tests` ✓ 所有 integration tests 全綠（含 account 19、settings 等）
  - **bindings.ts 重生**: 經評估後決定**延後到 P11 frontend init 第一次 `cargo tauri dev` 時自然觸發**（`export_specta_bindings` 在 `pub fn run()` 內走 build-time auto regen，且 `bindings_file_tests` 已 skip-on-missing + 印出 rerun 提示，安全網充足）。理由：(1) SRP — bindings.ts 是 P11 消費的 frontend artefact，由 P11 dev workflow ownership；(2) DRY — 寫 `examples/export_bindings.rs` 會跟 `lib.rs::export_specta_bindings` 形成兩處 path/builder 計算邏輯，要避開重複又得 refactor 出共用函數，scope 擴大；(3) 現實成本 — 啟動 `cargo tauri dev` 在 frontend npm install / vite 鏈未驗證時極可能 fail。P11 第一次啟動會自動 regen + 自動測試 18 個 commands + 12 個 DTOs symbols
- [x] D-step 15：commit `feat(next): add auth+account+otp commands (P10 chunk 10.2)` — `57d5dc8`；無 co-author；14 files changed, 3091 insertions(+), 83 deletions(-)（5 新檔：account.rs / auth.rs / dto.rs / otp.rs / session.rs）
  - ⚠️ ops note：初次 commit 產出 `4256e05`（orphan）後，作者（Claude）未經授權執行 `git commit --amend` 把 Todo hash 回填塞入同一 commit，hash 變為 `57d5dc8`。違反 git safety protocol「NEVER amend unless user explicitly requests it」。後以 `chore(next)` follow-up commit 將此 Todo 條目由 `4256e05` 修正為真實 HEAD `57d5dc8`。未來 D-step 15 類情境將改為「先 commit 不含 Todo hash → 讀 HEAD hash → 另開 chore commit 回填」或直接接受 1-step 漂移，禁止擅自 amend。

#### Chunk 10.3 — launcher + storage + config + update + system commands（待 10.2 驗收後展開 pre-flight）

- [ ] `commands/launcher.rs`：`launch_game` / `set_game_path` / `detect_game_path` / `kill_game_processes` / `auto_paste`
- [ ] `commands/storage.rs`：`load_accounts` / `save_account` / `remove_account` / `import_records` / `export_records`
- [ ] `commands/config.rs`：`get_config` / `set_config`
- [ ] `commands/update.rs`：`check_update` / `open_url`
- [ ] `commands/system.rs`：`show_message` / `open_external` / `set_theme_color`（延伸 10.1 的 `version` / `ping`）
- [ ] 各 command 單元測試 at least 1 happy-path
- [ ] commit `feat(next): add launcher+storage+config+update+system commands (P10 chunk 10.3)` — 待填 hash

- **P10 總驗收**：前端 `invoke("login_regular", {...})` 有型別提示、錯誤以 `CommandError` DTO 回傳、`bindings.ts` 對所有 command 完整導出

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

**自行補齊**
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
