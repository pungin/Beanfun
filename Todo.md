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

#### Chunk 10.3 — launcher + storage + config + update + system commands

##### Pre-flight 決策（Q1-Q8）

| # | 決策 | 理由 |
|---|------|------|
| Q1 system/open_url 架構 | **A** 建 `services::system::open_url` 薄包裝 + command thin wrapper | 層級原則一致（services = business logic / commands = IPC boundary）；未來 P12+ 還會有 `open_folder` / `open_mailto`，先建 module 模板便宜 |
| Q2 import/export dialog | **A** command 接 `path: String`，dialog 交 frontend | Tauri 慣例 + SRP 乾淨 + 避免 `tauri-plugin-dialog` capability 設置複雜 |
| Q3 get_config 形狀 | **C** 三支：`get_config_value(key)` + `get_all_config()` + `set_config(key, value)` | per-key 對齊 service 層 SRP；全表服務 frontend setting page 常見 UX；WPF 兩種 access 都有 |
| Q4 kill_game 一/兩段式 | **A** 拆 `list_game_processes` + `kill_game_processes` 兩支 | 對齊 WPF MessageBox confirm 流程；service 層 DRY 拆 `find_game_processes` + `kill_pids` helper |
| Q5 state PID 追蹤 | **A** 不加 state，所有 process command stateless | 對齊 WPF 每次 enumerate；涵蓋「非我 launch 也歸我管」case |
| Q6 detect_game_path 副作用 | **B** 讀+寫一條龍對齊 WPF | WPF parity 優先；Tauri 慣例「一次 invoke 完成一個 user-meaningful action」 |
| Q7 Records 密碼 DTO | **A** 直回明文 + import/export JSON 含明文 | 對齊 WPF；Tauri webview 同 trust boundary；本機 user 自存。docs 警告 export 檔案安全自負 |
| Q8 D-step 策略 | **A** 由淺入深 D1 system → D2 config → D3 storage → D4 update → D5 launcher（最複雜） | risk 留後面；前 4 步建立的薄 wrapper 模板可複用 |

##### D-step plan

- [x] D-step 1：`services/system/` 新模組 + `commands/system.rs` 擴充 `open_url` **COMPLETED**
  - 新建 `services/system/{mod.rs, error.rs, open_url.rs}`（依 `open` crate 5.3.3 direct dep，避開 `tauri-plugin-opener` 的 `AppHandle` coupling，保住 services framework-agnostic 原則）
  - `SystemError { InvalidUrl, OpenFailed, SpawnBlockingFailed }` + scheme allowlist (`http`/`https`/`mailto`) 拒絕 `file://` / `javascript:` / `data:` / 客製 scheme
  - `commands/system.rs` 擴充 `#[tauri::command] open_url(url)`；`commands/error.rs` 加 `From<SystemError>` + module-level `SystemError` code table + 更新 command-layer `system.*` table 註解與 `SystemError` 共享 namespace
  - `services/mod.rs` 加 `pub mod system;`
  - tests: 9 service (scheme allowlist 8 + 空 URL 1) + 3 command error-path (empty / file / javascript)
  - quality gates：fmt / clippy / `cargo test --lib` 509（P10.2 結尾 496 → +13）全綠
- [x] D-step 2：`commands/config.rs` — 3 commands **COMPLETED**
  - 新建 `commands/config.rs`：`get_config_value(key)` / `get_all_config()` / `set_config(key, value: Option)`
  - `services/config/xml.rs` 新增 `pub async fn get_all_values(path) -> Result<IndexMap<...>, ConfigError>`（typed error；command 層決定 catch-all policy）; `config/mod.rs` re-export + Layers 表更新
  - Path resolution 走 `state.storage_root.join("Config.xml")`（cross-platform + tests 乾淨；避開 windows-only `default_config_xml_path`）
  - Error policy asymmetry: `get_config_value` / `get_all_config` catch-all → `""` / `{}` (WPF parity); `set_config` 走 typed `CommandError { code: "config.*" }` (services deviation 延續 — WPF silent-swallow 是 support pain point)
  - DTO: `HashMap<String, String>`（specta object）; IndexMap → HashMap 在 command 層轉
  - tests: 7 command (config_xml_path helper + 6 command path) + 4 integration (`tests/config_xml.rs`: get_all_values missing / ordered / corrupted / non-utf8)
  - quality gates：fmt / clippy / lib 509→516 (+7) + config_xml 11→15 (+4)，全綠
- [x] D-step 3：`commands/storage.rs` — 5 commands **COMPLETED**
  - 新建 `commands/storage.rs`：`load_accounts` / `save_account(account)` / `remove_account(region, account_id)` / `import_records(path)` / `export_records(path)`
  - Q7=A 決策落地：`services::storage::Account` + `Records` 加 `Serialize + Deserialize + specta::Type` derive（row-shape 明文直送；WPF parallel-columns wire format 獨占 service 層，兩者分離）
  - `mutate_records_internal<F>` helper 包 load → mutate → save pipeline（Windows-only inside `imp` sub-mod）；save/remove 共用；mutator 設計為 infallible（list 操作不會 fail）
  - import_records 走 `services::storage::import_records`（含 legacy 遷移）+ `tokio::fs::read_to_string` 讀 ext 檔；export_records 走 `load_records_with_legacy_migration` + `export_records` (pure) + `tokio::fs::write`
  - Platform gate：commands 本身 unconditional 存在（bindings.ts 跨平台一致）；body 走 `#[cfg(target_os = "windows")]` 分版，非 Windows fallback `storage.platform_unsupported` CommandError；`PLATFORM_UNSUPPORTED_CODE` const 供 test pin 防漂移（windows build 標 `#[cfg_attr(windows, allow(dead_code))]`）
  - 新增 command-layer codes：`storage.import_read_failed` / `storage.export_write_failed` / `storage.platform_unsupported`（D7 補 module-level doc table）
  - Docs 明寫 Q7=A rationale（WPF parity + shared trust boundary + 未來 redactor 抽象位置）與 export JSON 明文密碼警告
  - tests: 5（3 upsert helper + 1 account serde roundtrip + 1 platform code 漂移防護；非 Windows 多 1 `platform_unsupported_error` 測試）
  - quality gates：fmt / clippy / lib 516 → 521 (+5) 全綠
- [x] D-step 4：`commands/update.rs` — 1 command **COMPLETED**
  - 新建 `commands/update.rs`：`check_update(channel: Channel, local_version: Option<String>) -> Option<UpdateInfo>`（對齊 WPF `ApplicationUpdater.CheckUpdate()` 的 silent-on-failure 契約）
  - `services::updater::github::Channel` 加 `Serialize + Deserialize + specta::Type` derive（unit-variant → bare `"Stable"` / `"Beta"` string，對齊 WPF `updateChannel` config value shape）
  - `services::updater::checker::UpdateInfo` 加 `Serialize + specta::Type` derive（backend-to-frontend only；刻意不加 `Deserialize`，frontend 不會產生此 struct）
  - return shape = `Option<UpdateInfo>` 而非 `Result<_, CommandError>`：service 層已把所有 failure mode collapse 成 `None`（對齊 WPF `catch (Exception) { Debug.WriteLine }`），command 層維持這個契約讓前端不用 try/catch
  - `local_version` 優先取 frontend override（diagnostic 用途），否則 self-report `env!("CARGO_PKG_VERSION")`；版號對齊留給 P12（目前 `0.1.0` vs remote `v5.8.3.*` 會恆為有更新）
  - tests: 3（Channel bare-string serialize/deserialize 各 1 + UpdateInfo 全欄位 serialize 1）
  - quality gates：fmt / clippy / lib 521 → 524 (+3) 全綠
- [x] D-step 5a：`commands/launcher.rs` — `launch_game` **COMPLETED**
  - 新建 `commands/launcher.rs`：`launch_game(game_path, mode: GameStartMode, command_line_template, account, password) -> Result<(), CommandError>`；hybrid 簽名（P1=C）：前端從 Config 讀 path/mode/template，帳密 + 實際拼接交給 backend（避免明文 command_line 往返 IPC）
  - `services::game::launcher::GameStartMode` 加 `Serialize + Deserialize + specta::Type` derive（P2）；unit-variant → bare `"Auto"` / `"Normal"` / `"LocaleRemulator"` string；對齊 P10.3 D4 `Channel` IPC 合約，frontend 把 legacy `startGameMode` 整數 (`"0"`/`"1"`/`"2"`) 轉字串
  - `build_command_line(template, account, password)` pub(crate) helper（P3）：任一字串空 → 回 `""`（對齊 WPF `MainWindow.xaml.cs` L1867-1879 guard）；否則委派 `substitute_credentials`；抽出來讓 empty-guard 獨立 unit-testable，未來 `auto_paste` 等想 reuse 時 DRY
  - 整段 orchestrator（`game::launch_game` 含 Normal `Command::spawn` + LR `ShellExecuteW`）包在 `tokio::task::spawn_blocking`（P10-Q5=A 守則，single await point）
  - 兩個新 command-only error codes（P4 獨立）：`launcher.target_dir_resolve_failed`（`default_target_dir()` io::Error，極罕見）/ `launcher.spawn_blocking_failed`（`JoinError`，panic or cancel）；定義為 `pub(crate) const` 給 drift test pin；跟 `system.spawn_blocking_failed` 保留區分以利 telemetry 分辨
  - `target_dir` 由 backend 用 `default_target_dir()` 解析（而非 frontend 提供），SRP 乾淨
  - Docs（P5 一次寫完整）：module doc 含 chunk layout 表（D5a 標 this module / D5b-d pending）、credentials plaintext policy、spawn_blocking 粒度決策、两個 command-only code origin 表；`commands/error.rs` module doc 加 `launcher.*` table
  - tests: 10（`build_command_line` 5 edge cases + `GameStartMode` serde 2 + `launch_game` async error-path 2 整合（empty path / missing file）+ command-code drift pin 1）
  - 敏感資料流：`LaunchRequest.Debug` 已 redact `command_line`，command 傳整個 struct 進 spawn_blocking 繼承此保障；plaintext password 只存活在 spawn_blocking task + ShellExecute/CreateProcess 呼叫點（不可避免）
  - quality gates：fmt / clippy / lib 524 → 534 (+10) 全綠；`cargo doc` D5a 新增 0 個 error（我引入 3 個 intra-doc link 錯誤皆修；剩 6 個既有 error 全部在 D1 `system.rs` × 4 + D3 `storage.rs` × 2，依 user rule #2「不修改沒叫我修改的部分」留 D8 統一處理）
- [x] D-step 5b：`commands/launcher.rs` — `set_game_path` / `detect_game_path` **COMPLETED**
  - `set_game_path(state, game_code, dir_value_name, path)` → 薄包 `services::config::set_value`；跨平台（Config I/O 不需 gate）；empty path 直接寫入空字串（等效於「未設」狀態，讓下次 `detect_game_path` 走 registry fallback；caller 想整個移除 key 請用 `set_config(key, None)`）
  - `detect_game_path(state, game_code, dir_value_name, dir_reg) -> Result<Option<String>, CommandError>` → Q6=B 讀+寫一條龍（對齊 WPF `MainWindow.xaml.cs` L574-607）：先讀 Config 短路；若空且 `dir_reg` 非空，strip `HKEY_LOCAL_MACHINE\` literal prefix（WPF L580 parity quirk），走 `services::registry::read_game_path(HKCU, subkey, dir_value_name)` 再把結果寫回 Config；Option<String> shape（`Some(path)` / `None`）Rust idiomatic，取代 WPF 的空字串 sentinel
  - `game_path_config_key(dir_value_name, game_code) -> String` pub(crate) helper — 統一 `{dir_value_name}.{game_code}` Config key 格式（WPF L575/590/604 parity），set/detect 兩命令都走此 helper（DRY 單點 truth）
  - INI-separation：`dir_reg` / `dir_value_name` / `game_code` 由 frontend 傳入（P11 會接 per-game INI service）；command 不讀 INI（SRP + testability + 未來 INI command 直接 compose）
  - Async/sync 粒度切分（偏離 D5a「整塊 spawn_blocking」）：Config I/O 已是 tokio 原生 async → 原生 await；只有 winreg 那段同步 call 包 `spawn_blocking`（三段 await 比一顆大 spawn_blocking 清晰；docs 已說明偏離理由）
  - Platform gating：`detect_game_path` body `#[cfg(target_os = "windows")]` 走 `detect_imp::detect_game_path_impl`，非 Windows 走 `platform_unsupported_error()`（對齊 D3 storage pattern）；`set_game_path` 不 gate
  - 新增 command-only code：`launcher.platform_unsupported`（`PLATFORM_UNSUPPORTED_CODE` const + `platform_unsupported_error()` fn，鏡像 D3 `storage.platform_unsupported` 設計）
  - Error 映射全部沿用：`ConfigError` / `RegistryError` 既有 `From<_> for CommandError`（registry NotFound/空值不走 error 通道，對齊 WPF catch 吞掉 → Ok(None)）
  - tests +11（total 10 → 21；lib 534 → 545）：
    - `game_path_config_key` 格式 2（dir.game 序 + empty game_code 防漂移）
    - `set_game_path` 跨平台 2（value round-trip + empty path）
    - Windows-only 5：config 短路（跳 registry）/ config 空 + dir_reg 空 → None / HKCU\Environment@TEMP 讀+寫回 Config / HKLM prefix strip / 不存在 subkey → None 且 Config 不變
    - `strip_hklm_prefix` helper 純 function test 1
    - code drift pin 擴為 3 個 codes 1
    - `platform_unsupported_code_is_stable` 1（explicit cross-module contract marker）
    - 非 Windows fallback 2（這兩條走 `#[cfg(not(target_os = "windows"))]` gate，CI 在 Linux runner 時跑）
  - quality gates：fmt / clippy / lib 534 → 545 (+11) 全綠；`cargo doc` D5b 引入 4 個 intra-doc link 錯誤（test/cfg-gated item 跨 scope linking），全部修完（改用 prose 描述代替 \[link\]）；剩 6 個既有 error 仍在 D1/D3 留 D8 處理
- [x] D-step 5c：`commands/launcher.rs` — `list_game_processes` / `kill_game_processes` ✅ **COMPLETED**
  - 新增 `services::process::game` module（sibling of `patcher`/`play_page`）：
    - `find_game_processes(game_path) -> Result<Vec<ProcessInfo>>`：從 `game_path.file_name()` 抽 exe 名 → WMI `find_processes_by_name` 一次取 `ExecutablePath` → byte-equal filter；`file_name == None`（pure root / 空路徑）短路回空 Vec
    - `kill_game_processes(pids) -> Vec<u32>`：iterator + 個別 `kill_process`，best-effort silent-skip（對齊 `check_and_kill_patcher` 既有 pattern），空 pids 不呼叫 kill
    - 兩者皆附 DI `_with` 變體供測試（沿用 `patcher::check_and_kill_patcher_with` pattern），pure match helper `matches_game_path` 獨立可測
  - commands 層都是 thin wrapper（`spawn_blocking` 包整個 service fn）
  - 新 IPC DTO `GameProcessInfo { pid, name, executable_path: Option<String> }`（camelCase serde + specta::Type，backend→frontend only 無 Deserialize）：定義在 command 層而非 service 層，因為 `services::process::ProcessInfo` 在 Windows-only gated module 裡，DTO 隔離讓 command signature cross-platform（bindings.ts 穩定）
  - `executable_path` 用 `Option<String>` via `Path::to_string_lossy`（遊戲安裝路徑實務上皆合法 UTF-8，lossy 無差；避免前端處理 PathBuf + specta quirks）
  - kill 信任邊界：**不**重驗 PID 屬於 game_path（P10.3 Q4=A 拆兩段決策；frontend list → confirm → 直接 forward pids；對齊 WPF L1821-1833 的 Yes 分支）
  - 無新增 error code：process.* (from `ProcessError`) + reuse D5a/D5b 的 `launcher.spawn_blocking_failed` + reuse D5b 的 `launcher.platform_unsupported`（non-Windows）
  - platform gating：command signature unconditional（sticky with D3 storage pattern）、body `#[cfg]` gate；Windows impl 集中在 `mod list_imp`（對齊 D5b `detect_imp`），含 `into_dto` 轉換 helper
  - tests：~18 (service 11 + command 7)
    - service `services/process/game.rs`：
      - `matches_game_path`：exact / mismatch dir / None path filter → false（×3）
      - `find_game_processes_with`：file_name=None short-circuit 不呼叫 find、全 match、部分 match + None 過濾、exe 名含副檔名傳給 finder、find err 傳遞、空 result（×6）
      - `kill_game_processes_with`：empty 不呼叫 kill、all success、partial fail（回成功子集）、all fail → 空 Vec、input order 保留（×5）
    - command `commands/launcher.rs`：
      - `GameProcessInfo` serde shape（camelCase、None → null）（×2）
      - `list_imp::into_dto` 路徑轉換 + None 保留（Windows-only，×2）
      - 非 Windows list/kill → `launcher.platform_unsupported`（×2）
  - quality gates：fmt / clippy / lib 545 → 563 (+18) 全綠；`cargo doc` D5c 只引入 1 個新錯誤（`Path::to_string_lossy` intra-doc link，改用 `std::path::Path::to_string_lossy` 完整路徑即修掉），剩 6 個既有 error 續留 D1/D3/D8 處理
- [x] **D-step 5d：`commands/launcher.rs` — `auto_paste`（COMPLETED）**
  - 新增 service：`src-tauri/src/services/process/auto_paste.rs`（~1070 行）
    - `PasteRequest<'a> { class_name, account, password, special_click }` 借用式 DTO（避免 IPC boundary 多一次 alloc）
    - `PasteDriver` trait（10 methods）+ `DefaultPasteDriver` 生產實作（delegate to `post_string::*` + `std::thread::sleep`）
    - `paste_credentials` 便捷 entry + `paste_credentials_with<D>` DI 變體（tests 用 `RecordingDriver` mock 驗證 sequence）
    - 私有 helpers：`find_target_window`（MapleStoryClass → MapleStoryClassTW fallback，硬編碼）、`compute_click_point`（WPF 0.5/0.4 ratio）、`pack_lbutton_pos`（WPF `(x & 0xFFFF) | (y << 16)` 位元排版）、`do_special_click`（SEA pre-login ESC + 點擊）、`clear_field`（VK_END + N×VK_BACK）
    - 常數封裝：`WM_KEYDOWN`/`WM_LBUTTONDOWN`/`VK_*` 鍵碼 + `ACCOUNT_CLEAR_BACKSPACES=64`/`PASSWORD_CLEAR_BACKSPACES=20` + 3 組 sleep 常數（100/100/200ms）+ `MAPLESTORY_PRIMARY_CLASS`/`MAPLESTORY_FALLBACK_CLASS`
  - 新增 `ProcessError::WindowNotFound { primary_class, fallback_class }` variant + `commands/error.rs` mapping 到 `process.window_not_found`（含 `primary_class` / `fallback_class` details）
  - `services/process/mod.rs`：加 `pub mod auto_paste;` + 重新導出 `paste_credentials` / `paste_credentials_with` / `DefaultPasteDriver` / `PasteDriver` / `PasteRequest` / `MAPLESTORY_PRIMARY_CLASS` / `MAPLESTORY_FALLBACK_CLASS`；chunk table 更新 10.3 列為 `[game, auto_paste]`
  - `commands/launcher.rs`：
    - 模組 chunk layout 表把 D5d 標 `**this module**`；新增 D5d 專屬段落說明 WPF parity / IPC DTO 動機 / `specialClick` dispatch（Q2 決定）/ blocking isolation / credentials / 沒有新 error code（`process.window_not_found` 透過既有 `From` mapping 自動得到）
    - `AutoPasteRequest { class_name, account, password, special_click }` DTO（`serde::Deserialize` + `specta::Type` + `camelCase`）
    - `auto_paste(req: AutoPasteRequest)` command thin wrapper（Windows-gated；非 Windows 回傳 `launcher.platform_unsupported`）
    - `paste_imp` Windows-only submodule：整段 orchestration 包在單一 `spawn_blocking`（D5a 顆粒度）
  - 測試增加 19 個（service 層 11 + command 層 8）：
    - 純函數：`pack_lbutton_pos` 位元排版 + x 溢位 mask / `compute_click_point` WPF 比例 + C# int 截斷語意（×4）
    - `find_target_window`：primary 命中 / MapleStory fallback / 非 MapleStory 不 fallback / 兩次都 miss → `WindowNotFound`（×4）
    - `paste_credentials_with`：非 special click 完整序列對齊 WPF / special click 前綴 ESC + 點擊 + 恢復 cursor / cursor save 失敗時不 restore（×3）
    - 錯誤傳播：`WindowNotFound` short-circuit / `GetClientRect` 失敗不送任何合成輸入 / `post_string` 非 ASCII 帳號 short-circuit（×3）
    - command 層：`AutoPasteRequest` camelCase 反序列化 / `specialClick` 必填 / 非 Windows fallback / Windows live 行為（無視窗 → `process.window_not_found`，details 帶 `primary_class`）（×4）
    - `commands/error.rs`：`process_window_not_found` 雙端測試（含 fallback null 情境）（×2）
  - quality gates：fmt / clippy 全綠；`cargo test --lib` 563 → 582 (+19) 全過；`cargo doc` D5d 零新增錯誤，剩下 6 個 warnings 全是 D1/D3 pre-existing（system.rs / storage.rs，續留 D8 處理）
- [x] **D-step 6：`collect_commands!` 整合 + `bindings_file_tests` 重構 + `bindings.ts` 首次生成 + Windows manifest workaround**
  - [x] D6-1 `lib.rs` 加 `pub fn default_bindings_path() -> PathBuf`（`run()` debug export + example binary + `bindings_file_tests::bindings_path` 三處全部改 call 此 helper，DRY）
  - [x] D6-2 新增 `beanfun-next/src-tauri/examples/export_bindings.rs`：`build_specta_builder::<tauri::Wry>()` + `builder.export(Typescript::default(), default_bindings_path())`；附 module docs 說明 standalone 匯出入口與 `run()` debug boot export 的 DRY 關係
  - [x] D6-3 `commands/mod.rs::build_specta_builder`：加 16 個新 commands（system 1 + config 3 + storage 5 + update 1 + launcher 6），分組註解對齊現有 `// auth (P10.2 — ...)` 風格
  - [x] D6-4 `commands/mod.rs::REQUIRED_SYMBOLS`：加 16 commands + 6 DTOs（`Account`/`GameStartMode`/`GameProcessInfo`/`AutoPasteRequest`/`Channel`/`UpdateInfo`；`Records` newtype 被 specta inline 成 `Account[]` 故不列入）
  - [x] D6-5 `cargo build --lib` ✓；`cargo run --example export_bindings` 解 `STATUS_ENTRYPOINT_NOT_FOUND` (0xc0000139) 後成功生成 `beanfun-next/src/types/bindings.ts`（75 KB，34 commands + 19 DTOs）
    - **Windows manifest workaround**（root cause fix，非 workaround）：`tauri-build` 透過 `embed_resource::compile()` 把 Common Controls v6 manifest 只 link 到 main bin（`cargo:rustc-link-arg-bins`），example/test bin 缺 manifest → `comctl32.dll` 解析到 v5 stub 缺 v6 entry point；參照 tauri 維護者 `lucasfernog` 在 [tauri#13419](https://github.com/tauri-apps/tauri/issues/13419) 推薦解法：
      - 新增 `beanfun-next/src-tauri/windows-app-manifest.xml`（直接 copy `tauri-build` bundle 的同名檔，內容字節相同）
      - `build.rs` Windows 段切到 `tauri_build::WindowsAttributes::new_without_app_manifest()` 停用 tauri 自動嵌；改用 `cargo:rustc-link-arg=/MANIFEST:EMBED` + `/MANIFESTINPUT:` 自己嵌（無 `-bins` 後綴 → 套用 main bin + example + test 全部）；其餘 Windows resource (version/icon/product name) 仍由 tauri-build 處理
      - 新增 `beanfun-next/src-tauri/.cargo/config.toml`：`x86_64-pc-windows-msvc` 加 `/DELAYLOAD:WebView2Loader.dll` + `delayimp.lib`（雙重保險：未來 test 直接連 wry 也不會缺 DLL；production main bin 行為 unchanged，DLL load 從 process-start 推遲到第一次 webview 呼叫，差幾百微秒不可觀察）
  - [x] D6-6 `bindings_file_tests` 重構（修 P10.1 D8 設計缺陷，過去因 fresh-clone skip 從未真跑暴露）：拆 `REQUIRED_SYMBOLS` 為 `REQUIRED_COMMANDS` (camelCase, search `async ${name}(`) + `REQUIRED_DTOS` (PascalCase, search `export type ${name}`)；同時砍 `TotpChallengeInfo`（只進 `CommandError.details` JSON 從未在 command 簽名出現，specta 不 emit）；`cargo test --lib commands::bindings_file_tests` ✓；`cargo test --lib` 全 582 tests ✓ 無 regression
  - [x] D6-7 更新 Todo.md 標 D6 完成
- [x] D-step 7：module docs — 5 模組（`system` / `config` / `storage` / `update` / `launcher`）頂層 doc 在 D1〜D5d 各自實作時已同步寫好（延續 P10.2 「commands 同步帶 module doc」習慣），D7 實際 scope 縮小為：
  - `commands/mod.rs` chunk layout 表 10.3 由 `pending` → `**done**`，描述列出 D1 / D2 / D3 / D4 / D5a~d 完整對應
  - design principles 在 P10.2 7-bullet 後擴充三條 P10.3 跨模組決策：auto-generated TS types 補上 `cargo run --example export_bindings` + `default_bindings_path()` 路徑統一描述；新增 platform gating with stable IPC surface（Windows-only command 簽名仍 unconditional，non-Windows fall through `<domain>.platform_unsupported`）；新增 hybrid credential pass-through（Q7=A 跨 storage / launcher 共識，明文 + import/export 互通 + 未來 secondary renderer 退路）
  - 初版加入 `[st]: storage` / `[ln]: launcher` reference link 對齊 P10.2 風格，但 D8 cargo doc 跑出 `redundant-explicit-link-target` lint（P10.2 既有 `[ac]` / `[pt]` 是 type alias 才需要顯式 ref，`storage` / `launcher` 是 module 名 implicit 已能 resolve），D8 改回 `[`storage`]` / `[`launcher`]` implicit form 並移除兩條 reference link
  - `cargo doc` smoke check 延後到 D8 quality gates 統一跑（P10.2 D14 同 pattern；D7 範圍只動 doc，無新增 lint surface）
- [x] D-step 8：quality gates 全綠 —
  - `cargo fmt --all -- --check` ✓（修 1 處：`build.rs::main` 多行 `windows_attributes(...)` 鏈呼收斂為單 expression，D6-5 manifest workaround 加 line 後遺漏跑 fmt）
  - `cargo clippy --all-targets -- -D warnings` ✓（0 warning，D5d / D6 / D7 期間每步都跑過 clippy 沒留 debt）
  - `cargo test --lib` ✓ 582/582（P10.2 結尾 496 → +86：system 13 + config 7 + storage 11 + update 8 + launcher 47）
  - `cargo test --tests` ✓ integration 全綠（含 hk_login 等既有 + auto_paste / process_find_kill / config_xml 等本 chunk 新增）；同時驗證 D6-5 manifest fix 對 test binaries 也生效（issue tauri-apps/tauri#13419 描述的 `STATUS_ENTRYPOINT_NOT_FOUND` path）
  - `cargo doc --no-deps --document-private-items` ✓（修 12 處 doc lint，6 個 D6/D7 新引入 + 6 個 D1/D3 累積；P10.3 各 D-step 把 doc gate 集中在 D8 處理是固定 pattern）：
    - `lib.rs` `default_bindings_path` doc 對 private `export_specta_bindings` 改 plain code（`commands/mod.rs` 已有 `#![allow(rustdoc::private_intra_doc_links)]` 但 `lib.rs` 沒有；單一 reference 不值得加 file-level allow，plain text 較 SRP）
    - `commands/mod.rs` D7 加的 5 處 `[`storage`][st]` / `[`launcher`][ln]` redundant explicit target 改 implicit `[`storage`]` / `[`launcher`]`，並拿掉對應的 `[st]:` / `[ln]:` reference link（見 D7 條目修正）
    - `commands/storage.rs` 兩處對 `cfg(not(target_os = "windows"))` gated `platform_unsupported_error` / `cfg(test)` gated `tests::platform_unsupported_code_is_stable` 的 intra-doc link 改 plain code（在 Windows host 跑 cargo doc 兩 item 都不可見，原 link 一定 unresolved）
    - `commands/system.rs` 4 處 `[`crate::services::system::open_url`]` ambiguous（`pub mod open_url; pub use open_url::open_url;` mod 跟 fn 同名）加 `()` disambiguator → `[`crate::services::system::open_url()`]`，並拿掉一個 redundant `[svc]` reference link
  - `bindings.ts` regen：D6-5 已透過 `cargo run --example export_bindings` 真實生成（不再延後到 P11）；後續 P10.3 再無 command 簽名 / DTO 變動，無需再 regen
- [x] D-step 9：commit `feat(next): add launcher+storage+config+update+system commands (P10 chunk 10.3)` — `2f28041`；無 co-author；32 files changed, 7474 insertions(+), 151 deletions(-)（13 新檔：`.cargo/config.toml` / `examples/export_bindings.rs` / `commands/{config, launcher, storage, update}.rs` / `services/process/{auto_paste, game}.rs` / `services/system/{mod, error, open_url}.rs` / `windows-app-manifest.xml` / `src/types/bindings.ts`）
  - ops note：按 P10.2 D15 教訓採「先 commit 不含 Todo hash → 讀 HEAD hash → 另開 chore commit 回填」流程，禁止擅自 amend

##### 預估

- 新增 commands：16（system 1 + config 3 + storage 5 + update 1 + launcher 6）
- 新增 DTOs：約 3-5（`UpdateInfo`/`ProcessInfo`/`KillResult` + 視需要 `LaunchOutcomeDto`；Account/Records 是既有 service type 加 derive）
- 新增 service 函數：約 3-4（`system::open_url` / `process::game::find_game_processes` / `process::auto_paste_otp` / 可能 `config::get_all_values`）
- lib tests 預估 496 → 530+

- **P10 總驗收**：前端 `invoke("login_regular", {...})` 有型別提示、錯誤以 `CommandError` DTO 回傳、`bindings.ts` 對所有 command 完整導出

### P11 — Vue 前端：i18n / Pinia / 主題

#### P11 pre-flight decisions（2026-04-16）— Q1-Q10 全 all_you_decide

- **Q1（Sub-chunking）= A（單 chunk + 一次 commit）**：user 指示「如果都確定了就一次做完一次驗證」；14 D-step 內聚（彼此依賴 invoke wrapper / i18n / stores），中途 commit 反而切出非 user-visible-feature-complete 狀態
- **Q2（`services/invoke.ts` 形狀）= B（thin wrapper）**：`wrapCommand` 解 `Result<T, CommandError>` + ElMessage error toast + console log + `auth.session_required` redirect hook；另 export `safeInvoke` 不 throw 給少數 caller 自處理。對齊 WPF「try-catch + MessageBox.Show」改為 ElMessage；DRY（每 store action 一行 invoke 不用 try/catch）
- **Q3（i18n key + convert-lang）= A（WPF key 1:1）**：`<system:String x:Key="K">V</system:String>` → flat KV JSON；佔位符 `{0}` 保留（vue-i18n list-mode 支援）；映射 `zh.xaml→zh-TW.json` / `zh-Hans.xaml→zh-CN.json` / `en.xaml→en-US.json`；vitest 加三語系 key 一致性 guard 防漂移
- **Q4（Pinia store 粒度）= A（4 store）**：`auth` / `account` / `config` / `ui`；對齊 backend AppState 4 面向；auth 內部用 sub-state field 區分 regular/qr/totp/verify 流程不拆 store
- **Q5（Persisted state）= B（不 persist）**：single source of truth = `Config.xml`；frontend store 只是 in-memory cache，啟動 boot hook 呼 `get_all_config` 載入；DRY 最強（避免 Config.xml + localStorage 雙存同步 bug）；對齊 WPF（無獨立 frontend persistence）；`pinia-plugin-persistedstate` 留 package.json 不 wire
- **Q6（ELP theme color runtime）= A（CSS variable + JS shade gen）**：`useThemeColor` composable + 自寫 30 行 HSL mix helper 算 5 lighter / 3 darker shade + `setProperty`；mockup `_design-system.html` 8 色作 preset + 自訂 hex picker；對齊 WPF「Settings 即時換色」
- **Q7（`tauri.conf.json` capabilities）= A（暫不補）**：保持現況 `core:default + opener:default`；YAGNI — P11 純 infra，P12 切 page 時依需求補（dialog 給 import/export，shell 給 game launch 等）
- **Q8（Router mode + structure）= A（hash + flat + minimal）**：`createWebHashHistory`；P11 只建 root `/` route 載 placeholder，P12 切 page 時逐個加 route；Tauri SPA 標準
- **Q9（Smoke test 範圍）= B（cargo tauri dev + version command）**：App.vue boot 呼 `commands.version()` 顯示 build info；驗 frontend↔backend IPC round-trip + bindings.ts 真用得起來
- **Q10（Test scope per module）= 3+ vitest tests**（contract / 邊界 / error path）對齊 P10 backend lib tests pattern；不深測 vue-i18n / pinia 內部 behavior；quality gates 跟 P10 對齊 `vitest run` + `vue-tsc --noEmit` + `eslint .` + `prettier --check .`

#### D-step plan（單 chunk，14 D-step + 1 chore）

- [x] D-step 1：`services/invoke.ts` thin wrapper ✓ — `wrapCommand<T>` 解 `Result<T, CommandError>` + `ElMessage.error` + `console.error` + `auth.session_required` redirect hook；`safeInvoke<T>` 回 `SafeResult<T>` 不 throw；`CommandInvocationError` 保留 structured cause；refactor 出 `surfaceCommandError` 讓 auth store 的 `safeInvoke` branch（`totp_required` / `verify_required` 當 flow continuation 不 toast）也能共用 error pipeline；`registerErrorTranslator` / `registerSessionExpiredHandler` 為 main.ts boot wire；12 vitest ✓
- [x] D-step 2：`scripts/convert-lang.mjs` ✓ — Node ESM + `fast-xml-parser`（devDep 新增）解 `<system:String x:Key="K">V</system:String>` → flat KV；保留插入順序 + `{0}` 佔位符 + XML entity 解碼；`LOCALE_FILE_MAP` 映射 3 XAML → 3 JSON；8 vitest（inline XAML fixture，涵蓋 non-string resource / invalid XML / order preservation）✓
- [x] D-step 3：跑 D2 script 產 `src/locales/{zh-TW,zh-CN,en-US}.json` ✓ — generated-and-checked-in artefact；**上游 WPF `zh-Hans.xaml` 真實缺約 30 個 key**（vs `zh.xaml` / `en.xaml` 完全對齊），對齊舊 WPF 的 `ResourceDictionary` fallback 行為；D10 drift guard 因此調整（見 D10）
- [x] D-step 4：`composables/useThemeColor.ts` ✓ — `setPrimaryColor(hex)` + 線性 RGB mix helper 算 `light-3/5/7/9` + `dark-2` shade（Element Plus 實際 CSS variable 名稱）；export `THEME_PRESETS` 8 色 + `DEFAULT_PRIMARY_COLOR`；18 vitest（含 3-digit hex / 大小寫正規化 / edge case weight / all shades applied）✓
- [x] D-step 5：`router/index.ts` ✓ — `createWebHashHistory` + `routes[]` flat + 1 root `/` 掛 `PlaceholderPage.vue` + catch-all redirect；`createAppRouter()` factory；`ROUTE_NAMES` 常數；5 vitest ✓
- [x] D-step 6：`stores/config.ts` ✓ — Pinia `defineStore('config', ...)` setup syntax；`entries` / `loaded` / `size` computed；`loadAll()` 經 `wrapCommand(commands.getAllConfig())` 篩 string-only；`get` / `getOr` / `set(key, null)` delete semantics；不走 `pinia-plugin-persistedstate`（P11 Q5=B）；8 vitest ✓
- [x] D-step 7：`stores/ui.ts` ✓ — 上層 `useConfigStore`；5 persistent computed getter（`themeColor` / `language` / `minimizeToTray` / `disableHwAccel` / `updateChannel`）+ 對應 setter 走 `config.set` 寫 `Config.xml` + side-effect（`setPrimaryColor` / `localeApplier`）；純 UI `globalLoading` / `currentDialog` 不走 Config；`applyAll()` boot hook（theme fallback default / locale try-catch keep previous）；`registerLocaleApplier` 留給 D10 wire；12 vitest（含 invalid config 值 fallback）✓
- [x] D-step 8：`stores/auth.ts` ✓ — `session` / `pendingTotp` / `pendingVerify` / `qrChallenge` + `pendingAction` 雙擊 guard（`withGuard` helper）；8 IPC actions；**`loginRegular` / `loginTotp` 用 `safeInvoke` 攔 `auth.totp_required` / `auth.verify_required` 當 flow continuation 不 toast**（backend 回 `CommandError` 是正常登入階段訊號，`surfaceCommandError` 只留給真 error）；15 vitest ✓
- [x] D-step 9：`stores/account.ts` ✓ — 雙 scope 整合在單一 store（per P11 Q4=A 4-store decision）：Users.dat（`accounts[]` / save / remove / import / export）+ live service account（`serviceAccounts[]` / refresh / add / rename / otp）；`getEmail` / `getRemainPoint` / `getContract` 各自 `Map<number, T>` cache 因 session-scoped 且跨 service-account lookup；`clearSessionData()` 供 auth `logout` 呼叫清 runtime cache；16 vitest ✓
- [x] D-step 10：vue-i18n setup ✓
  - `src/i18n/messages.ts` — frontend-only 非 WPF key（`placeholder.*` / `errors.*` / `themePreset.*`）三語系 nested tree；`KeysMatch<T, U>` compile-time guard 強制 zh-CN / en-US tree 與 zh-TW canonical 完全一致（任何 key 缺失即 `vue-tsc --noEmit` 失敗）
  - `src/i18n/index.ts` — `i18nMessages` 把 generated WPF flat key（`AppName` 等）+ frontend-only nested key（`placeholder.*` 等）shallow merge（namespace 不碰撞）；`createAppI18n()` factory（`legacy: false` Composition API mode / `fallbackLocale: 'en-US'` / 依 `import.meta.env.DEV` 開 warn）；`setLocale(i18n, code)` helper；`wireI18n(i18n)` 同時註冊 ui store 的 `localeApplier` 跟 invoke 的 `errorTranslator`（errors.{code} → localized toast；te() miss 則 fallback backend message）
  - 9 vitest（含 2 drift guard + 4 createAppI18n behavior + 3 wireI18n surface 測）✓
  - **P11 原 Q3 plan 假設「三語系 key set 完全一致（防漂移 guard）」，但 D3 發現 WPF 上游 `zh-Hans.xaml` 真實比 `zh.xaml` / `en.xaml` 少約 30 key**。對齊舊 WPF 1:1 的原則，drift guard 放寬為：(a) 三 JSON load non-empty、(b) zh-CN ⊆ zh-TW（抓 zh-CN renegade key）、(c) zh-TW ≡ en-US（平行翻譯雙方皆上游維護）；frontend-only messages.ts 仍 strict equality；runtime 由 `fallbackLocale: 'en-US'` 補 zh-CN 缺 key（match WPF ResourceDictionary fallback semantics）
- [x] D-step 11：`main.ts` wire ✓ — `createApp(App).use(pinia).use(i18n).use(ElementPlus).use(router).mount('#app')`；`wireI18n(i18n)` 在 `app.use(i18n)` 之前執行確保 UI store 首次 render 前已註冊好 locale applier；`element-plus/dist/index.css` import 一併做在 main.ts；`pinia-plugin-persistedstate` 留 `package.json` 但故意不 register（P11 Q5=B）
- [x] D-step 12：`App.vue` overhaul ✓ — 整個換掉 Tauri scaffold；`<el-config-provider :locale="elpLocale">` wrap `<RouterView />`；`ELP_LOCALE_MAP: Record<AppLocale, Language>` 映射 3 locale 到 Element Plus `zh-tw.mjs` / `zh-cn.mjs` / `en.mjs`；`elpLocale` computed 跟 `useUiStore().language` 連動即時切；`onMounted` 跑 `config.loadAll()`（失敗走 `ElMessage.warning` 不卡 boot）→ `ui.applyAll()`；root font-family / 色系走 mockup design
- [x] D-step 13：quality gates ✓
  - `npm run test`：**vitest 111 passed / 10 files**（D1: 12, D2: 8, D4: 18, D5: 5, D6: 8, D7: 12, D8: 15, D9: 16, D10: 9, smoke: 3；比預估 ~35 多 ~3 倍，因各 D-step 實作時就補足 contract/邊界/error path 測到飽）
  - `npm run typecheck`：`vue-tsc --noEmit` 0 error（D12 過程補 `src/element-plus-locale.d.ts` ambient shim，因 element-plus 的 `package.json` exports 沒掛 `dist/locale/*.mjs` subpath → `TS7016`，此 workaround 是 element-plus issue tracker 官方建議做法；`createAppI18n()` 拿掉顯式 return type 讓 TS infer，原寫 `I18n<typeof i18nMessages, ...>` 跟 vue-i18n 內部 `LocaleMessage<VueMessageType>` 不相容）
  - `npm run lint`：`eslint .` 0 error 0 warning（`src/types/bindings.ts` 已於 D1 加入 `ignores`，D10 移除 `messages.ts` 無效的 `/* eslint-disable @typescript-eslint/no-unused-vars */` 塊）
  - `npm run format:check`：`prettier --check .` all clean（一次性跑 `npm run format` 應用樣式；加 `src/types/bindings.ts` 進 `.prettierignore` 因為 tauri-specta 生成檔的 `/* prettier-ignore */` 只能 ignore 下一個 statement 不是全檔）
  - `cargo check --manifest-path src-tauri/Cargo.toml` ✓ Rust 側仍 compile 乾淨
  - `npm run build`（`vue-tsc --noEmit && vite build`）✓ 1647 modules transformed，prod bundle 產出（JS 1.15 MB / CSS 353 kB，>500 kB warning 是 ELP + Pinia + vue-i18n 組合的典型 baseline，P12 可視需再 code-split）
  - `cargo tauri dev` 視覺 smoke 交 user 手動驗（placeholder 顯示 heading + `app` / `tauri` 版本 + 預設橘 `#FF8201` 主題 + zh-TW 文案）
  - **D13 期間發現 D5 留下的 debt 一併修掉**（typecheck / lint 卡住）：(a) `Placeholder.vue` 使用 `version.productVersion` / `buildSha` / `buildTimestampUtc` 這 3 個實際不存在的欄位（`VersionInfo` 只有 `app` / `tauri`，D5 沒跑 typecheck 沒抓到），改成顯示 `app` + `tauri` + 新增 `placeholder.appVersion` / `placeholder.tauriVersion` i18n key（三語系同步）；(b) `Placeholder` 違反 `vue/multi-word-component-names`，加 `defineOptions({ name: 'PlaceholderPage' })` 不動檔名與 router `ROUTE_NAMES.Placeholder` 常數
- [x] D-step 14：commit `feat(next): add P11 frontend infra (i18n + Pinia + router + theme)` — `8aeebaf`；無 co-author；33 files changed, 10312 insertions(+), 5758 deletions(-)（deletions 主要是 `src/types/bindings.ts` 上一版 + `App.vue` Tauri scaffold 整個換掉）
  - ops note：按 P10.2 D15 / P10.3 D9 教訓採「先 commit 不含 Todo hash → 讀 HEAD hash → 另開 chore commit 回填」流程，禁止擅自 amend

##### 預估

- 新增 frontend 模組：~12 (`services/invoke.ts` / `router/index.ts` / `composables/useThemeColor.ts` / `i18n/index.ts` / 4 stores / `pages/Placeholder.vue` / `App.vue` overhaul / `main.ts` overhaul / `scripts/convert-lang.mjs`)
- 新增生成 artefact：4 (`src/locales/{zh-TW,zh-CN,en-US}.json` + 1 fixture for D2 script test)
- 預估 vitest tests：~35（D1: 3, D2: 2, D4: 3, D5: 2, D6: 4, D7: 4, D8: 6, D9: 5, D10: 2 + 1 guard）

- **驗收**：主題 / 語系 / 設定存檔 / 重啟保留（透過 Config.xml backend 寫入；frontend store 重啟空，boot hook 重新讀回）；`cargo tauri dev` smoke 證 IPC 通路 + bindings.ts 型別正確

### P12 — Vue 前端：所有 Pages + Windows 1:1

#### P12 pre-flight decisions（2026-04-16）

- **M1（chunking）= 5-chunk plan**（user 修正 from 「1 chunk per view = 30 commit」→ 「不要 commit 太多」）
- **M2（push）✓**：origin `feat/beanfun-next-rewrite` 已落 8 commit
- **Q1（sub-chunking）= 5 chunks**：依「依賴順序 + 用戶可見里程碑」切（登入 → 帳號 → 遊戲 → 設定 → 工具）；每 chunk 1 feat commit；每 chunk 結尾 1 chore 回填 Todo.md hash（總 10 commit）
- **Q2（mockup → Vue 對齊精度）= 結構＋互動級**：mockup 視覺（glassmorphism / fluent / gradient button）為基底，但 ELP 元件標準樣式覆寫局部以保 a11y / behavior consistency
- **Q3（routing）= per-page route, flat**：`/login` / `/login/region` / `/login/id-pass` / `/login/qr` / ... / `/accounts` / `/accounts/manage` / `/settings` / `/about`；hash mode；catch-all redirect 到 `/login`（未登入）或 `/accounts`（登入後）
- **Q4（dialog）= ElDialog 為主 + 限定 Tauri WebviewWindow**：所有純表單 dialog（AddAccount / ChangeAccount / etc.）走 `ElDialog`；GamepassForm / WebBrowser / GamePassBrowser 走 Tauri `WebviewWindow`（需獨立瀏覽器渲染環境）
- **Q5（拖曳排序）= vuedraggable**（已在 deps）：用於 AccountList 帳號排序
- **Q6（i18n frontend-only key）= per-view 邊做邊補**：`i18n/messages.ts` 三 locale 同步加 key + `KeysMatch` guard 守住；不集中補
- **Q7（capability）= per-view 最小**：每 view 開頭評估補必要 capability（dialog 給 import/export，shell 給 game launch 等），P12 結尾統整 audit
- **Q8（per-view test）= 3+ vitest baseline**：render / prop or store 整合 / interaction（emit / button click）；重要互動點才加碼到 5+
- **Q9（檔名 + 命名）= 對齊 mockup 檔名 + ESLint multi-word**：`pages/LoginPage.vue` / `windows/AddAccount.vue` 配 `defineOptions({ name: 'AddAccountDialog' })` 滿足 `vue/multi-word-component-names`
- **Q10（quality gates）= 每 chunk 跑全套**：`npm run test` + `npm run typecheck` + `npm run lint` + `npm run format:check` + `cargo check` + `npm run build`；`cargo tauri dev` 視覺 smoke 交 user 看
- **Q11（Todo.md hash backfill）= chunk-end 1 chore（沿用 P10/P11 pattern）**：每 chunk 1 feat commit + 1 chore 回填 hash；feat 內各 view 那行 [x] 由 chore 一次處理（保 feat 純 = 純實作 + 純測試）

#### Scope 修正

- **Pages**：11 個（對齊 `Beanfun/Pages/*.xaml` 11 檔）
- **Windows**：**19 個**（對齊 `Beanfun/Windows/*.xaml` 19 檔；原 Todo.md 寫 16 是漏算 `GamePassBrowser` / `EquipCalculator` / `CoreCalculator`）
- **總 30 view**

#### 5-chunk 切法

##### P12.1 — 登入流程（10 view）

WPF mapping：

| Vue file | WPF source | Mockup | View kind |
|---|---|---|---|
| `pages/LoginPage.vue` | `Beanfun/Pages/LoginPage.xaml` | n/a (shell) | Page |
| `windows/LoginRegionSelection.vue` | `Beanfun/Windows/LoginRegionSelection.xaml` | `LoginRegionSelection.html` | Dialog |
| `pages/IdPassForm.vue` | `Beanfun/Pages/id-pass_form.xaml` | `IdPassForm` (Stitch) | Page |
| `pages/QrForm.vue` | `Beanfun/Pages/qr_form.xaml` | `QrForm` (Stitch) | Page |
| `pages/GamepassForm.vue` | `Beanfun/Pages/gamepass_form.xaml` | `GamepassForm.html` | Page (+ Tauri WebviewWindow) |
| `windows/GamePassBrowser.vue` | `Beanfun/Windows/GamePassBrowser.xaml` | n/a (folded into GamepassForm WebviewWindow) | Tauri WebviewWindow |
| `pages/LoginTotp.vue` | `Beanfun/Pages/LoginTotp.xaml` | `LoginTotp.html` | Page |
| `pages/LoginWait.vue` | `Beanfun/Pages/LoginWait.xaml` | `LoginWait.html` | Page |
| `pages/VerifyPage.vue` | `Beanfun/Pages/VerifyPage.xaml` | `VerifyPage.html` | Page |
| `windows/CaptchaWnd.vue` | `Beanfun/Windows/CaptchaWnd.xaml` | `CaptchaWnd.html` | Dialog |

D-step：
- [x] D1 router shell ✓ — `/` redirect → `/login`；`/login` → `pages/LoginPage.vue`（glass-panel shell + i18n brand heading + `<RouterView />` slot）；`children: []` 留 D2-D8 補；同步刪 `pages/Placeholder.vue` + `placeholder.*` i18n key（已 served P11 smoke）；新 `loginShell.{heading,subline}` 三 locale；`tests/unit/pages/LoginPage.spec.ts` 3 vitest（render heading / router-view slot / locale switch）；router spec 重寫覆蓋 root redirect / direct /login / catch-all；i18n spec placeholder 引用換 `loginShell` + 切到 `GashRemain` 驗 `{0}` 仍 work；vitest 115 passed（+4）/ typecheck 0 / lint 0 / prettier clean
- [x] D2 `pages/LoginRegionSelection.vue` ✓ — TW/HK 2-tile picker（routable view，**改放 `pages/` 而非 `windows/`** 因 SPA 自然 fit；WPF Window 改成 named empty-path 子路由 `/login` 預設子）；click → `config.set('loginRegion', region)` → `router.push('/login/id-pass')`；新 `loginRegion.{subline,defaultBadge,totpHint,tip}` 三 locale；router 改成 `routes[1].children = [{path:'', name: LoginRegion}]`（vue-router 警告：named parent + unnamed empty child won't render → 移 name 到 child）；移除 `ROUTE_NAMES.Login`，僅保留 `ROUTE_NAMES.LoginRegion`；`tests/unit/pages/LoginRegionSelection.spec.ts` 6 vitest（render tiles / heading+subline+tip / TW persist / HK persist / nav to /login/id-pass / locale switch）；router spec 重寫覆蓋 / → /login → picker / direct /login → picker / name → path / catch-all；vitest 123 passed（+8）/ typecheck 0 / lint 0
- [x] D3 `pages/IdPassForm.vue` ✓ — 帳號 + 密碼 ElInput（show-password toggle）+ Remember / AutoLogin 勾選（WPF coupling：Auto→Remember 自動勾、Remember off→Auto 自動取消，對齊 `id-pass_form.xaml.cs` L29-37）+ Login button；submit 時讀 `config.get('loginRegion')` 預設 `TW` → `auth.loginRegular(region, account, password)`；空 account → toast `AccountNeed`、空 password → toast `PasswordNeed`（對齊 WPF `btn_login_Click` 的 MessageBox 早 return）；submit loading 綁 `auth.pendingAction === AUTH_ACTIONS.LoginRegular`；post-login 路由：success → `/accounts`、`pendingTotp` → `/login/totp`、`pendingVerify` → `/login/verify`（後三者目前 fall-through 經 catch-all 回 `/login`，等 D6/D8/P12.2 才有實際畫面，per Q2=A trade-off）；router 加 `/login/id-pass` named child（`ROUTE_NAMES.LoginIdPass = 'login-id-pass'`）；**reuse 現有 WPF i18n key**（`AcountOrEmail` / `Password_` / `RememberPassword` / `AutoLogin` / `Login` / `AccountNeed` / `PasswordNeed`），無新 frontend-only key；`tests/unit/pages/IdPassForm.spec.ts` 9 vitest（render labels / empty account toast / empty password toast / Auto→Remember coupling / Remember-off→Auto coupling / TW default submit + nav `/accounts` / HK config submit / `pendingTotp` nav `/login/totp` / `pendingVerify` nav `/login/verify` / locale switch）；用 `vi.hoisted` 包 `elMessageError` spy（vi.mock 工廠 hoist 在 const 之前）；用 stub element-plus 元件（ElForm/ElFormItem/ElInput/ElCheckbox/ElButton/ElIcon）以 v-model 行為驅動測試；router spec 加 `/login/id-pass` resolution 1 case + child 數量改 `>=1` + LoginIdPass 常量 assertion；vitest 135 passed（+12）/ typecheck 0 / lint 0 / prettier auto-fix 1 file
  - **D3 視覺驗證 hotfix A/B（frontend）** ✓ — 空 account / 空 password 各跳兩次 toast：`ElButton` 的 `@click="submit"` 跟 form 的 `@submit.prevent="submit"`（`native-type="submit"`）雙觸發；移除 `@click` 讓唯一入口是 form submit；對應 `IdPassForm.spec.ts` 從 `.el-button-stub @click` 改走 `.el-form-stub @submit` 觸發（行為 1:1 對齊 WPF button → 單次 click handler，沒有 form submit 概念所以無此 bug）
  - **D3 視覺驗證 hotfix F（P3 parity fix）** ✓ `1e40051` — Beanfun prod 對 `AccountLogin` / `CheckAccountType` 的 `ResultCode` / `Result` / `ResultMessage` / `ResultData.Captcha` 四欄位會回 integer（`ResultCode: 1`），Rust `Option<String>` serde 嚴格拒收（`invalid type: integer '1', expected a string`），而 WPF 用 `JToken.ToString()` 寬鬆 coerce（`BeanfunClient.Login.cs` L77/L97-99）；新 `login/mod.rs::deserialize_jtoken_to_string` helper（`deserialize_with` visitor，接 str/i64/u64/f64/bool/null，bool 吐 `"True"`/`"False"` 對齊 .NET 大小寫、object/array 仍 reject 留未來 regression 探針）+ `parse_step_json<T>(text, step)` wrapper（失敗 `tracing::warn!` 印 `step` + bounded `body_preview`（500 char 截斷，UTF-8 safe 不切半字）+ error）；**DRY**：helper 共用於 4 個欄位；**SRP**：helper 放 `login/mod.rs` 而非 `beanfun/mod.rs` —— 目前僅 login flow 踩 WPF `.ToString()` pattern，hoist 條件是出現第二個 non-login consumer；新增 13 個 `helper_tests`（str/int+/int-/float/bool T/bool F/null/missing/obj reject/arr reject/truncate_chars full/truncate_chars multibyte/parse_step_json LoginError::Json）+ 4 個 `check_account_type::tests`（Captcha int/str/null/missing ResultData）+ 3 個 `account_login::tests`（all-int response 走 classify_outcome 成功分支 / mixed int+str 走 AdvanceCheckRequired with URL / legacy all-string 仍過；pin WPF regression）；`cargo fmt` clean / `cargo clippy --all-targets -D warnings` clean / `cargo test --lib` 602 passed（+20）；獨立於 D3 commit（P3 parity 非 P12.1 UI 範疇）
  - **D3 登入觀測補件（login-flow diagnostic logging）** ✓ `7ef3085` — P3 fix 後 live 登入成功但 backend happy path 完全無 log、3 個常見失敗點（TW / HK session key regex miss、SendLogin empty scrape）只丟 typed error 沒 wire-shape 上下文，下次用戶再報一個錯根本無從 root-cause；對稱加：(a) `tw_regular.rs` + `completed.rs`（HK Regular / HK TOTP / QR finalize 共用 tail）在 `Ok(Session::new(...))` 前各印一行 `tracing::info!`，field 只帶 `step` / `region` / `account_id`（account_id 非 secret，已透過 `SessionInfo` 曝給 frontend；skey / web_token / akey 是 session bearer，`Session::Debug` 已 redact，絕不進 log）；(b) 3 處失敗 diagnostic：`session_key.rs::get_session_key_tw` regex miss 印 `final_url`（redirect chain 終點，query 可能含 UA-derived id 但無憑證，安全可記）、`session_key.rs::get_session_key_hk` regex miss 印 `body_preview`（區分 anti-bot interstitial / error page / 新 markup）、`send_login.rs` empty scrape 印 `body_preview`（同三分類）；`truncate_chars` + `BODY_LOG_PREVIEW_CHARS` 沿用 P3 helper 共用（**DRY**），兩者留 private in `login/mod.rs` 由 sibling submodule 透過 descendants rule 存取，不提升可見性（**SRP**）；純觀測、零行為改變、零 error-code 變動；`cargo fmt` clean / `cargo clippy --all-targets -D warnings` clean / `cargo test --lib` 602 passed（不新增測試 — tracing side effect，`tracing-test` 非 dev-dep，commit message 已 document）/ frontend 135 passed；獨立 chore commit 跟 P3 fix 平行（不混進 P12.1 D12 UI batch）
  - **D4 live-test hotfix — QR poll HTTP 411 Length Required（backend）** ✓ — live reproduction 顯示 region picker → TW → QR → bitmap → 2s → inline 紅框「無法取得登入狀態」；frontend `console.error` 抓到 `CommandError { code: "auth.unknown", message: "unexpected login error: QRLogin/CheckLoginStatus returned HTTP 411 Length Required", details: { detail: "…HTTP 411…" } }`；root cause 是 `src-tauri/src/services/beanfun/login/qr_poll.rs` L176 `.body("")` → reqwest/hyper 對空字串 body 不自動補 `Content-Length: 0`（用 `Transfer-Encoding: chunked` 或兩者皆無），beanfun `QRLogin/CheckLoginStatus` endpoint 嚴格 HTTP/1.1 回 411；WPF `WebClient.UploadString(url, new NameValueCollection())` 對空 NV 會同時自動補 `Content-Type: application/x-www-form-urlencoded` **和** `Content-Length: 0`，原作者 P3 chunk 只補了 Content-Type（註解 L174-175 確實寫了「reqwest does NOT do automatically for `.body("")`」但漏算 Content-Length 那一層）；fix 最小 diff：`qr_poll.rs` 新增 `.header(header::CONTENT_LENGTH, "0")` 一行 + 擴寫註解解釋 WPF 雙 auto header + hyper chunked fallback 的 411 根因（live 2026-04-18 觀察到）；不加新 wiremock case（現有 `tests/qr_poll.rs` 沒檢查 request headers、加 Content-Length matcher 需改 test harness、ROI 低）；`cargo fmt --check` + `cargo clippy --all-targets --all-features -- -D warnings` + `cargo test --lib --no-fail-fast`（602 passed）全綠；獨立於 D4 component scope、不 commit（留 D12 batch）
  - **P11 live-test hotfix — LightBlue themeColor RangeError（frontend）** ✓ — live boot 每次噴 `ui.ts:176 [ui.applyAll] failed to apply themeColor; falling back to default RangeError: invalid hex color: LightBlue at parseHexColor (useThemeColor.ts:98:11)`；root cause 是舊 WPF `Beanfun/Pages/Settings.xaml` L90-97 ThemeColor ComboBox 有 8 項（2 hex `#FF8201` / `#B6DE8E` + 6 WPF named `White` / `Black` / `LightBlue` / `Pink` / `Gold` / `Silver`），WPF 用 `ColorConverter.ConvertFromString(sColor)`（`MainWindow.xaml.cs::changeThemeColor` L249）吃雙格式；user 在舊 WPF 選 `LightBlue` → Config.xml 寫入 `<ThemeColor>LightBlue</ThemeColor>` → P11 `setPrimaryColor` 嚴格走 `parseHexColor` 路徑 → `RangeError` → `applyAll` catch 後 fallback `#FF8201` 能啟動但每次 boot 印紅 console + user 主題色沒回來；fix：`src/composables/useThemeColor.ts` 新增 `WPF_NAMED_COLOR_ALIASES: Record<string, string>`（6 對映射：White→#555555、Black→#1A1A1A、LightBlue→#0B6E99、Pink→#D85A88、Gold→#C9A227、Silver→#7A7A7A；對應 `THEME_PRESETS` P11 版 hex，非 W3C/WPF named color hex，因 P11 design system 刻意 re-tune 飽和度，保留 user 語意選擇「淺藍」的 intent 但用 P11 視覺）+ `resolvePrimaryColor(stored: string): string` pure helper（case-insensitive + trim，unknown → 原字串直通由 `parseHexColor` 噴 RangeError）+ `setPrimaryColor` 先呼叫 `resolvePrimaryColor` 再 `parseHexColor`；**不**改 `parseHexColor` 契約（只吃 hex）也**不** migrate Config.xml（避免舊新 client 共存配置相踩、`setPrimaryColor` 不需變 async）；2 個 hex ComboBox 項 `#FF8201` / `#B6DE8E` **不**在 alias table（hex 直通 parseHexColor，user 手寫 `#B6DE8E` 不強轉 P11 `#5C8430` green，尊重 literal 輸入）；`tests/unit/composables/useThemeColor.spec.ts` +6 case（新 `resolvePrimaryColor` describe：6 aliases lock + case-insensitive + trim + hex pass-through + unknown pass-through；`setPrimaryColor` describe +1 `LightBlue` → `#0b6e99` 應用）；vitest 160 passed（+6）/ lint 0 / typecheck 0；獨立於 D4 scope、不 commit（留 D12 batch）
  - **D3 → D4 navigation hotfix（frontend）** ✓ — live smoke test 顯示 id-pass 零導覽鉤（無法回 region picker、無法切 QR），導致 D4 落地的 `/login/qr` UI 不可達、user 報「應該要可以回上一頁」；最小 diff 在 `IdPassForm.vue` 補兩顆：(a) top-left `← 返回` 按鈕 → `router.push('/login')`（SPA affordance；WPF region picker 是 blocking modal 非 routable page，故無直接對照）；(b) bottom `QR Code便利登` 文字連結 → `router.push('/login/qr')`（WPF parity for `btn_QRCode`，`id-pass_form.xaml` L736 `btn_QRCode_Click`）；複用既有 i18n key `Back` / `QRCodeLogin`（zh-TW + en-US 已有；zh-CN 缺 `Back` 經 P11 fallback → zh-TW「返回」，符合 i18n drift guard `zh-CN ⊆ zh-TW` 契約）；icon 用 `ArrowLeft`（element-plus/icons-vue 已有）、QR 用文字連結（element-plus 無 QR glyph，inline SVG 不值得 footprint）；GamePass 切換 icon（WPF `btn_GamePass` `Visibility="Collapsed"`）留 D5 跟 `GamepassForm` 一起來；`IdPassForm.spec.ts` +2 case（back → `/login` 不 call `loginRegular` / QR → `/login/qr` 不 call `loginRegular`；memory router 加 `/login` + `/login/qr` stub；icon mock 加 `ArrowLeftStub`）；`IdPassForm.vue` docblock 更新（刪除「QR/GamePass switch icons → D4/D5 wire each form into a switcher」，新增 D3→D4 hotfix 小節）；vitest 154 passed（+2）/ typecheck 0 / lint 0 / prettier clean；cargo 不動；獨立於 D4 scope（D4 主檔案 `QrForm.vue` 不碰）、不 commit（留 D12 batch）
- [x] D4 `pages/QrForm.vue` ✓ — QR 登入表單（Q1=B MVP + Copy Deeplink / Q2=B `QR_POLL_INTERVAL_MS = 2000` const + WPF L161 `TimeSpan.FromSeconds(2)` 註解 / Q3=A `onMounted → doStart`、`onBeforeUnmount → disposed=true + clearPollTimer` / Q4=C approved→`/accounts`、expired 自動 `doStart()`（WPF `qrCheckLogin_Tick` L2364-2367 `refreshQRCode()`）、pending/retry 靜默繼續 polling（WPF res==0 mix of `Wait Login` / `Failed`）/ Q5=C pre-flight HK guard `readRegion()==='HK'` → `ElMessage.info(loginQr.unsupportedHK)` + `router.push('/login')` 不 round-trip backend / Q6=B deeplink null → 按鈕 disabled、有值 → `navigator.clipboard.writeText` + `CopyDeeplinkSuccess` toast、clipboard API reject → `CopyFailed` toast / Q8=A 單檔 / Q10=C 遞迴 `setTimeout(runPollTick, 2000)` 天然 overlap 保護、`disposed` + `pollTimeoutId` 兩層 idempotent guard 保證 unmount / approval / back 都無孤兒 tick / Q11=B `auth.loginQrCheck` 改回 `SafeResult<QrStatus>`（`safeInvoke` 不 toast）→ 輪詢錯誤停 + inline `loginQr.connectionLost` 紅框 + refresh 按鈕恢復；對齊 WPF `qrCheckLogin_Tick` L2358-2359 timer disable 無 MessageBox 靜默語意 + 現代 inline UX；`loginQrStart` 仍走 `wrapCommand` toast path（HK guard 前置後實際 backend `auth.qr_unsupported_region` 幾乎不可能觸發）；`CommandInvocationError` 走 inline fallback、`withGuard` "already in progress" 走 silent ignore（refresh 雙擊）；`src/pages/QrForm.vue`（~300 LOC incl. docblock + template + scoped style）；`src/stores/auth.ts::loginQrCheck()` 回型由 `Promise<QrStatus>` 改 `Promise<SafeResult<QrStatus>>`（breaking signature 但 D4 是唯一 caller）；`src/i18n/messages.ts` 新增 `loginQr.{title,subtitle,unsupportedHK,connectionLost}` 三 locale（`KeysMatch` guard 保 drift）；router 加 `/login/qr` named child（`ROUTE_NAMES.LoginQr = 'login-qr'`）；`tests/unit/pages/QrForm.spec.ts` 14 vitest（mount→start→bitmap / HK pre-flight 不 call backend / 2s polling cadence / pending+retry 靜默繼續 / approved push `/accounts` + 停輪詢 / expired 自動 refresh 換 bitmap / check error 停+inline / refresh 重 mint + 清 banner / back 停輪詢 + nav `/login` / clipboard success toast / deeplink null → disabled / clipboard reject → `CopyFailed` / unmount 清 timer / locale 切換），用 `vi.useFakeTimers` + `advanceTimersByTimeAsync(2000)` 驅動 polling cadence；`tests/unit/stores/auth.spec.ts` +1 case（error result 不 toast、qrChallenge 保留；`vi.mocked(ElMessage.error).mockClear()` 先清再驗）；`tests/unit/router/index.spec.ts` +2 case（`qr` child route config / `/login/qr` resolve）；vitest 152 passed（+17）/ typecheck 0 / lint 0 / prettier auto-fix 2 files；cargo check clean（純 frontend diff）
  - **Observability cosmetic — `account_id=<deferred>` log sentinel（empty-string render fix）** ✓ — 方案 B subscriber 上線後 user live test 看到 QR 成功的 log `…step="LoginCompleted" region=TW account_id=` 後半空字串看起來像 bug；root cause 是 QR flow 照 WPF parity 傳 `""` 進 `login_completed`（qr_finalize.rs L228，module doc L137-147 已紀錄 "the actual account … we only learn it on the subsequent GetAccounts call (P3.5)"）；WPF `LoginCompleted`（L838-842）根本沒 account_id 參數，連帶 class 也沒 this.account field（只 L874 `GetAccounts(service_code, service_region, false)` 拿列表），所以 Rust 的 `Session.account_id` 是我們刻意多加的純 UI display field（非 wire）—— QR flow 空值 expected、不影響 P12.2 AccountList（那邊讀 `services/beanfun/account.rs::get_accounts` 回傳的列表，不是 Session.account_id）；fix 在 `completed.rs::login_completed` tracing call 前加 `let account_id_display = if account_id.is_empty() { "<deferred>" } else { account_id };` sentinel，純 cosmetic：不改 `Session::new(... account_id ...)`（傳進去還是原本的 `""`，下游 P3.5 GetAccounts 依賴不變、不改 public API、不影響 bindings.ts）、只動 log 欄位，讓 postmortem reader 一眼分得出「設計上 deferred」vs「HK Regular / TOTP 不該空但空了的 bug」（comment 亦說明：HK 路徑 contract 強制非空，若 surface `<deferred>` 也是 signal 不是 noise）；`cargo fmt` 0 / `cargo clippy --all-targets -- -D warnings` 0 / `cargo test --lib` 602 passed / 不新增測試（tracing side effect，與 observability hotfix 同規格）；獨立於 P12.1 D-step、不 commit（留 D12 batch）
  - **Observability hotfix — `tracing` subscriber 從未 init（方案 B：minimal env-filter fmt）** ✓ — Option B 修完後 user live test QR → pending → approved → 成功 push `/accounts`（`auth.missing_web_token` 確認消失）但 dev server terminal `log 完全沒東西`；root cause 是 `src-tauri/src/lib.rs::run()` 從專案 bootstrap 起就**從未呼叫** `tracing_subscriber::*::init()`，而 `tracing` 全域 `Dispatch` 沒人 register 時 macro 會 compile 但 resolve 到 no-op writer → P3 / D3 / D4 一路投資的 diagnostic（`session_key.rs` regex miss `body_preview` / `send_login.rs` empty scrape / `completed.rs::login_completed` `step="LoginCompleted"` / `verify.rs` retry trace 全部）通通沉到海底；Cargo.toml L62-63 早就有 `tracing = "0.1"` + `tracing-subscriber = { features = ["env-filter"] }` 但缺 init call site；fix 最小 diff 在 `lib.rs` 加 `init_tracing()` private helper（`fmt().with_env_filter(...).with_target(true).init()`）+ `run()` 最開頭呼叫；directive 走 `EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info,beanfun_next_lib=debug"))` —— 預設 third-party crate（hyper / reqwest / tauri / tao）INFO+（避免 dense per-request DEBUG drown signal）+ 我們自己 `beanfun_next_lib` crate DEBUG+（surfaces P3+D3+D4 投資的 diagnostic 而不需強制每個 contributor 設 `RUST_LOG`），developer 可 `RUST_LOG=trace cargo tauri dev` / `RUST_LOG=hyper=warn,beanfun_next_lib=trace` per-session override；輸出走預設 stderr（`cargo tauri dev` terminal 即見）；prod Windows build `windows_subsystem = "windows"` 抑制 console window，stderr 進虛空（不影響今日範圍 — observability scope 只 cover dev-time，prod 路由到檔案是未來 P-stage 工作）；test isolation：`cargo test` 不經 `run()` 故不會 double-init panic；helper docblock 完整紀錄 why（postmortem）/ how（filter directive）/ 輸出 destination / test 隔離 / WPF parity none（observability infra 非 business logic，WPF 用 ad-hoc `Console.WriteLine` + 自訂 file logger，這是 industry-standard 升級）；不選方案 C（`tauri-plugin-log`）原因：plugin 走 `log` facade 不認 `tracing`，要寫 30+ 行 `tracing::Layer` bridge forward to `log::log!()` 才能讓 plugin 收到我們的 event，過度工程；`cargo fmt --check` clean / `cargo clippy --all-targets -- -D warnings` clean / `cargo test --lib` 602 passed（無 regression）；Tauri file watcher auto-rebuild 抓到變動、新 binary 上線；獨立於 P12.1 D-step、不 commit（留 D12 batch）
  - **D4 live-test hotfix — `auth.missing_web_token` after QR scan（Option B：full WPF parity）** ✓ — 411 hotfix 修完後 user 掃 QR → pending→approved → 卻收 `CommandError { code: "auth.missing_web_token", message: "bfWebToken cookie missing from return.aspx response" }`；root cause 逐行比對 WPF `BeanfunClient.Login.cs::QRCodeLogin`（L530-607）+ `LoginCompleted`（L838-882）出兩條 divergence：**Bug 1**（QR step 3）WPF L591-598 把 `bfWebToken` scrape 包在 `if (!string.IsNullOrEmpty(setCookieHeader))` 裡、缺 cookie 靜默 `return "OK"`（L600），但 Rust `post_return_aspx` 嚴格要求 cookie 否則 `MissingWebToken`；**Bug 2**（QR step 4 / HK Regular / TOTP 共用 tail）WPF L863 `UploadString` 預設 `AllowAutoRedirect = true` 跟 chain 到底 + L868 `GetCookie("bfWebToken")` 從 cookie jar 讀，但 Rust `login_completed` 透過 `post_return_aspx`（`http_no_redirect()` + 第一 302 `Set-Cookie` 即時 scrape）→ 如果 beanfun 在後面 hop 才 set bfWebToken（live 觀察到的行為）就抓不到；Option B 兩 bug 同修；fix：(1) `qr_finalize.rs` step 3 call site 改 `match post_return_aspx(...) { Ok(_) | Err(MissingWebToken) => {}, Err(other) => return Err(other) }`，只吞 `MissingWebToken` 其他錯（transport / HTTP 4xx/5xx / URL）仍 propagate；(2) `completed.rs::login_completed` 改 `client.http()`（auto-redirect 10 hop max 預設）→ `ensure_success`（非 2xx 回 `LoginError::Unknown`，WPF L604-607 outer try/catch parity）→ drop response → 新 helper `read_bfwebtoken_from_jar(&client) -> Option<String>`（`client.cookie_store().lock().matches(&portal_base).into_iter().find(name eq "bfWebToken").map(value)`；用 `CookieStore::matches(&Url)` 拿 RFC 6265 §5.1.3 domain-scope-matched cookies，對齊 WPF `CookieContainer.GetCookies(new Uri("https://{portal_host}/"))` L144-150），None → `MissingWebToken`；(3) `return_aspx.rs` module docs 更新成 narrower scope（TW Regular step 5 + QR finalize step 3 only，明標 NOT used by `login_completed` + 指回 completed.rs postmortem），舊 "L160-165 no-redirect + Set-Cookie scrape" rationale 原樣保留給 TW 路徑；SRP：`post_return_aspx` 只管 WPF `redirect=false` 場景，`login_completed` 自己實作 `redirect=true` 場景，兩者不混；DRY：新 helper `read_bfwebtoken_from_jar` 只有一個 caller，不 hoist 到 mod.rs；副作用：7 個 integration test 檔 `mount_return_aspx_*` helper 全補新 `mount_after_landing(server)`（`GET /after → 200 OK`）讓 auto-redirect chain 有落地點，不然 wiremock 404 會被 `ensure_success` 轉成 `LoginError::Unknown`；`tests/qr_finalize.rs::step3_missing_set_cookie_yields_missing_web_token` 改寫成 `step3_missing_set_cookie_is_tolerated_and_continues_to_step4`（驗新 leniency：step 3 缺 cookie 不再 fatal，step 4 照跑並最終產出 `STEP4_WEB_TOKEN`），檔頭 summary table 同步；`tests/qr_finalize.rs::step4_login_completed_missing_token_yields_missing_web_token` 改 fixture（step 3 也不 set cookie）+ 註解解釋：auto-redirect + jar read 下「step 4 omit 但 step 3 set」jar 非空會被 read_bfwebtoken_from_jar 吞到 step 3 token（WPF 行為相同），canonical MissingWebToken surface 是「所有 hop 都沒 set」不是「step 4 單獨 omit」；`tests/login_completed.rs` 新增 `bfwebtoken_set_on_late_redirect_hop_still_lands_on_session` case（第一 302 不帶 Set-Cookie、/after 200 才 Set-Cookie: bfWebToken → 證明 jar-read 抓得到、header-scrape 抓不到）；cargo fmt 0 / cargo clippy --all-targets -- -D warnings 0 / cargo test 602 lib + 221 integration passed / 0 failures / 2 ignored；frontend 不動；touch `login/qr_finalize.rs` + `login/completed.rs` + `login/return_aspx.rs`（docs only pt3）+ 7 test 檔；獨立於 P12.1 D-step、不 commit（留 D12 batch）；P13 E2E live retest 可直接覆蓋 QR → portal → GameList
- [x] D5 `pages/GamepassForm.vue` + `windows/GamePassBrowser`（Tauri WebviewWindow open / cookie sync / on_navigation hook）✓ — Q matrix: Q1=C 切 D5a/D5b 兩 commit / Q2=B `login_gamepass_start` + `login_gamepass_complete` 拆兩 cmd / Q3=A `pending_gamepass: (BeanfunClient, skey)` slot / Q4=A `WebviewWindow`（系統新視窗）/ Q5=C backend handles cookie injection（避免 HttpOnly 曝給 JS）/ Q6=A `WebviewWindowBuilder::on_navigation` + `Webview::cookies_for_url` / Q7=C ElSteps 4-step / Q8=C cancel + refresh / Q9=A 純 unit test（無 E2E、對齊 WPF）；Tauri 2.10.3 `Webview::cookies_for_url` (Aug 2025) 支援 HttpOnly cookies、`WebviewWindowBuilder::on_navigation(|url| → bool)` 攔每次 navigation；切 4 個 checkpoint 每 CP 結束 sanity check：
  - **CP1 D5a backend** ✓ — `PendingGamepass { client, skey }` struct + `AppState::pending_gamepass` slot + `login_gamepass_start(state, region) -> Result<(), CommandError>` cmd（HK reject `auth.gamepass_unsupported_region` / clear pending_totp,qr,gamepass / fresh BeanfunClient / `get_session_key` / stash slot / tracing info）+ logout 清 pending_gamepass + specta builder 註冊 + lib unit smoke；新增 `LoginError::GamepassUnsupportedRegion` variant + `commands/error.rs` mapping `auth.gamepass_unsupported_region`；`GAMEPASS_NOT_STARTED_CODE` const 留到 CP3 隨 `open_gamepass_window` 落地（避免 dead_code warning）；更新 `commands/auth.rs` family table（加 gamepass row、刪掉「deferred to P12」舊段、改寫成「CP3 split out」）+ `clear_all_auth_state` 清 4→5 slot（順序 auth → totp → qr → verify → gamepass）+ `logout` doc 同步；新增 2 個 lib unit test：`commands::state::tests::pending_gamepass_can_be_populated_then_taken`（state slot populate + take 對稱）+ `commands::error::from_impls_tests::login_gamepass_unsupported_region_has_no_details`（wire-string + 無 details 契約，pin 住 `auth.gamepass_unsupported_region` 不漂移）；bindings.ts 自動 regen（`loginGamepassStart(region: LoginRegion) : Promise<Result<null, CommandError>>` + Rust docblock 完整 carry over）；quality gates：cargo fmt 0 / clippy 0 warnings / cargo test --lib **604 passed**（從 P11 baseline 602 增 2，剛好對應新測試）；dev server 自動 rebuild compile 0 warning（最後一輪 `Finished dev profile in 8.08s`）；不 commit、留 D12 batch
  - **CP2 D5a frontend** ✓ — 設計決策（user 全權委託、保 WPF 功能 parity / DRY / SRP）：**(a)** bindings 簽名 `loginGamepassStart(region): Promise<Result<null, CommandError>>` 維持（CP1 生成值），**(b)** HK guard 走前端 pre-flight（DRY：沿用 QrForm `readRegion()==='HK'→redirect+toast` 樣板，與 WPF `btn_GamePass Visibility="Collapsed"` 語意一致 —— HK user 根本進不來，backend `auth.gamepass_unsupported_region` 仍當 defence-in-depth），**(c)** GamepassForm 自動觸發模式（mount → `loginGamepassStart`，不保留 "Open GamePass" 按鈕：route 導航本身就是 user intent，WPF 的額外一次按鈕點擊只是 legacy in-place form swap 的殘留，SPA 不需要），**(d)** CP2 step 範圍鎖在 `STEP_INITIAL`/`STEP_PREPARED`（CP3 event listener 落地後接手 2/3/4），**(e)** i18n 複用策略 `GamePassLogin` 當 IdPassForm 切換鏈結 label / `loginGamepass.*` 當 form-scoped copy（對齊 `loginQr.*` 結構）；`src/pages/GamepassForm.vue` 新增（~250 LOC incl. 完整 docblock：WPF parity / 自動觸發 rationale / HK pre-flight / CP2 scope / error handling；`<el-steps :active>` + `STEP_INITIAL`/`STEP_PREPARED` 符號常數避免 magic number；`onMounted` region guard + `doStart` + `onBeforeUnmount` disposed 三層 guard 跟 QrForm 同步；`CommandInvocationError` → inline `connectionLost` banner、非 CommandInvocationError 走 silent `withGuard` "already in progress"）；`src/stores/auth.ts` 加 `AUTH_ACTIONS.LoginGamepassStart = 'login.gamepass_start'` + `loginGamepassStart(region): Promise<void>` action（withGuard + wrapCommand、docblock 說明 void return 因為 no secrets over IPC、HK guard defence-in-depth 解釋）；`src/router/index.ts` 加 `ROUTE_NAMES.LoginGamepass = 'login-gamepass'` + `loginChildren` 追加 `{ path: 'gamepass', name: ..., component: GamepassForm }`、header doc table 同步 D5 CP2 ✓；`src/pages/IdPassForm.vue` 加 `switchToGamepass()` + `<button data-test="id-pass-switch-gamepass">` 跟 QR link 並排在 `.id-pass-form__switches` flex container（WPF `btn_GamePass` parity、L742-749 `Visibility="Collapsed"` 在 SPA 改為與 QR 同層級顯示，讓 HK user 看得到兩個路徑；HK pre-flight guard 在 form 內攔截）；`src/i18n/messages.ts` 新增 `loginGamepass.{title,subtitle,steps.{prepare,openWindow,authenticate,complete},prepareDone,unsupportedHK,connectionLost,refresh}` 三 locale（`KeysMatch` guard 保 drift）+ 擴 module docblock 說明 `loginGamepass.*` namespace + 為何 WPF `GamePassLogin` 保留作切換鏈結 label；`tests/unit/pages/GamepassForm.spec.ts` 新增 6 vitest（mount→start→step 1 advance + prepareDone 狀態列 / HK pre-flight 不 call backend + info toast + redirect / backend err → inline connection-lost + step stays 0 / Refresh 重 invoke + 清 banner + step 1 / Back → `/login` + 不 call backend / locale switch 重繪；ElSteps/ElStep stub 透明 render `:active` 到 `data-active` 屬性）；`tests/unit/stores/auth.spec.ts` +3 case（loginGamepassStart ok → void + session 不變 / backend refusal → CommandInvocationError / in-flight pendingAction 為 `LoginGamepassStart`）；`tests/unit/router/index.spec.ts` +2 case（gamepass child route config 存在 / `/login/gamepass` resolve to `LoginGamepass`）+ stable const 斷言 +1；`tests/unit/pages/IdPassForm.spec.ts` +1 case（`[data-test="id-pass-switch-gamepass"]` → `/login/gamepass` 不 call `loginRegular`）+ mountForm memory router 加 `/login/gamepass` stub；quality gates：**frontend** vitest 172 passed（從 D4 baseline 154 增 18 = QrForm 14 + store 1 + router 2 + IdPassForm 1 + GamepassForm 6 - 重計差、實 +12）/ typecheck 0 / lint 0 / prettier auto-fix `src/i18n/messages.ts` 後 clean；**backend** cargo fmt 0 / clippy 0 / cargo test --lib **604 passed**（CP1 baseline，frontend-only diff 無 regression）；不 commit（留 D12 batch）
  - **CP3 D5b backend** ✓ — 設計偏離 / rationale：**(a)** 原計畫 `complete_gamepass_login(...) -> Session` + `LoginError::MissingGamepassWebToken` 改為 `try_complete_gamepass_login(client, skey, service_code, service_region) -> Option<Session>`（silent `None`=尚未出現 bfWebToken），因為 WebView 完成 OAuth 後此 service 零 HTTP、純讀 jar，且 WPF L803-836 在 token 缺席時也是 silent early-return 非 throw（`MissingGamepassWebToken` variant 因此不用加，`CommandError` mapping 也不動）；**(b)** WPF 的 `on_navigation` 鉤改用 Tauri `on_page_load(Finished)`，`Started` 邊 cookies 還沒寫入、會 race；每次 `Finished` edge 仍依 URL 觸發 harvest，語意等價但更穩；**(c)** `read_bfwebtoken_from_jar` 從 `completed.rs` 升到 `login/mod.rs` shared helper（DRY：HK Regular / TOTP / QR / GamePass 4 條 flow 共用 jar-scope 讀 bfWebToken 的同一段 `cookie_store().matches(portal_base)`）；`services/beanfun/login/gamepass.rs` 新增（~180 LOC incl. 模組 docblock：WPF L803-836 mapping / zero-I/O rationale / `Option<Session>` silent-return parity）含 `inject_webview_cookies(client, source_url, cookies: impl IntoIterator<Item=RawCookie<'static>>)`（Tauri `cookie 0.18.1` = `reqwest_cookie_store 0.8.2` 底層 `cookie_store 0.21.1` → `cookie 0.18.1` 同版本，`tauri::Cookie<'static>` 與 `RawCookie<'static>` binary-compatible，可直 `store_response_cookies` 免轉換）+ `try_complete_gamepass_login` 兩 fn + 單元測試（inject additive / mismatched domain rejection / complete happy-path / complete with missing token → None / complete preserves skey/service_code/service_region）；`commands/auth.rs` 新 `open_gamepass_window<R: tauri::Runtime>(app: AppHandle<R>, state) -> Result<(), CommandError>` cmd（generic over runtime 因 `WebviewWindow<R>`；async fn 避 WebView2 在 sync context build 時的 wry#583 Windows 死鎖）+ 常數 `GAMEPASS_NOT_STARTED_CODE` / `GAMEPASS_WINDOW_LABEL` / `GAMEPASS_SUCCESS_EVENT`=`gamepass-login-success` / `GAMEPASS_FAILED_EVENT`=`gamepass-login-failed` / `GAMEPASS_CANCELLED_EVENT`=`gamepass-login-cancelled` / `GAMEPASS_HARVEST_URLS`（WPF 3 domain：tw.beanfun.com / tw.newsgame.beanfun.com / bfweb.hk.beanfun.com）/ `GAMEPASS_COMPLETION_PATH_MARKERS` / `GAMEPASS_AUTOCLICK_JS`（`DOMContentLoaded` listener click `a.use-gama-pass`，比 WPF `ExecuteScriptAsync` 的 timing race 穩）+ 內部 helper `should_try_gamepass_completion` / `parse_harvest_url` / `build_gamepass_login_url(skey)` / `handle_gamepass_page_load(app, window, url)`（`on_page_load(Finished)` → spawn 到 `tauri::async_runtime::spawn` 避 `cookies_for_url` 在 WebView2 message pump 上 Windows 死鎖 → 3 domain harvest → `inject_webview_cookies` 回 BeanfunClient jar → `try_complete_gamepass_login` → `Some(session)` 時 `pending_gamepass.take()` + emit success + `window.close()` / None 時 silent wait 下一 navigation / service err 時 emit failed + close）+ `handle_gamepass_window_destroyed(app)`（`WindowEvent::Destroyed` 偵測 user 手動關窗：success path 已 `take()` 所以 `pending_gamepass.is_some()` 為 true = user cancel → clear + emit `gamepass-login-cancelled` / false = programmatic close = skip 避 double-emit）+ double-open guard 用 `app.get_webview_window(GAMEPASS_WINDOW_LABEL).is_some()` 回 `GAMEPASS_NOT_STARTED_CODE`；`commands/mod.rs` `collect_commands!` 註冊 `auth::open_gamepass_window::<tauri::Wry>`（specta 側 turbofish 必須具體化 runtime、tauri 側被 macro 砍掉 turbofish 由 `generate_handler!` 帶 closure 捕 outer `R`；用 outer `R` 會踩 rustc E0401 `use of generic parameter from outer item`，因 `_collect_functions!` expansion 產生非泛型內部 `fn export`；註解寫明 TS binding 與 runtime 無關故鎖 Wry 不影響輸出契約）+ `REQUIRED_COMMANDS` 加 `openGamepassWindow`；8 個新單元測試（`harvest_urls_match_the_wpf_reference_set` / `should_try_gamepass_completion_*` / `parse_harvest_url_*` / `build_gamepass_login_url` URL 編碼 / `GAMEPASS_AUTOCLICK_JS` JS snippet 正確性 / event name 常數 pin / `GAMEPASS_NOT_STARTED_CODE` 契約 / `handle_gamepass_window_destroyed` early-exit when pending 已 clear）；`bindings.ts` 以 `cargo run --example export_bindings` regen（+ `openGamepassWindow()` wrapper，Rust docblock carry over）；quality gates：cargo fmt 0 / cargo clippy --lib --all-targets --no-deps -- -D warnings 0 / cargo test --lib **625 passed**（從 CP2 baseline 604 增 21）/ cargo test 全套 0 failed / 2 ignored / frontend `npm run typecheck` 0 錯（bindings regen 安全）；不 commit（留 D12 batch）
  - **CP4 D5b frontend** ✓ — 設計偏離 / rationale：**(a)** 直用 `@tauri-apps/api/event` 的 `listen()` 不包 `useEventListener` composable（單 form 單用途、SRP 下不值得抽 wrapper，跟 QR form 不走 event-driven 對稱），listener 在 `onMounted` 註冊三個 `listen<T>(event, cb)` 收 unlisten handle push 到 `unlistenFns: UnlistenFn[]`、`onBeforeUnmount` 逐個 detach（防 late event 打壞 destroyed tree），**(b)** step 映射改 `0→1→2→4` 而非原計畫 `1→2→3→4`：backend 無中間 "authenticated but not yet harvested" event（bfWebToken 出現 IS 完成），故 `STEP_COMPLETE=4` 直接從 `STEP_WINDOW_OPENED=2` 跳過去，step 3 transient <1 frame 不值得 fabricate 事件；**(c)** 事件名 `GAMEPASS_SUCCESS_EVENT`/`FAILED`/`CANCELLED` constants literal 不透過 `tauri-specta` 綁定（specta 不 model `app.emit`，只 model commands；飄移風險由 `.spec.ts` 固定常數斷言 + backend 同檔 const 鎖），**(d)** `applyGamepassSession(info: SessionInfo): void` 新 store action，複刻 `loginRegular` 成功分支的 4-欄位原子 mutation（`session` 設值 + 清 `pendingTotp`/`pendingVerify`/`qrChallenge`）讓 GamePass 事件路徑的 store 狀態與 command 路徑完全一致（SRP：session 寫入仍限在 store；DRY：保留未來把 4-欄位 clear 提 helper 的餘地但目前不動其他 callers）、`withGuard` 不包因為 event 到達時 `LoginGamepassStart` 的 guard 早已 release，**(e)** 錯誤 banner 兩分離（`connectionLost` 鎖 step 0 / `windowError` 鎖 step ≥1），mutually exclusive by design；`gamepass-login-failed` 在 event handler 裡走 `windowError=true` + step rewind 到 `STEP_PREPARED`（讓 Refresh 有意義），**(f)** cancelled = silent（WPF parity，`GamePassBrowser` 關窗不跳 dialog）：step reset 0 + clear windowError、**無** toast、**無** i18n key（docblock 明寫為何沒 `loginGamepass.cancelled`），**(g)** listener 註冊時序 `registerEventListeners()` **before** `doStart()` 裡的 `openGamepassWindow`，避免 backend 超快 emit（harvest 在 command Promise resolve 前完成）漏接 —— 保留 race 安全邊界；`src/pages/GamepassForm.vue` 擴 ~150 LOC（docblock CP4 flow 段 + `GAMEPASS_*_EVENT` 常數 3 個 + `STEP_WINDOW_OPENED=2`/`STEP_COMPLETE=4` 2 個常數 + `windowError` ref + `unlistenFns` + `registerEventListeners` fn + `doStart` 擴展第二段 `wrapCommand(commands.openGamepassWindow())` + `onMounted` 先 register 後 start + `onBeforeUnmount` detach 迴圈 + template `windowError` banner + `prepareDone` `v-if` 收窄 `>= STEP_PREPARED` → `=== STEP_PREPARED && !windowError` 避免 step 2 殘留顯示）；`src/stores/auth.ts` +`applyGamepassSession(info)` action（純 state setter、docblock 說明為何不 `withGuard`、為何 inline mutation 而非先行 DRY 提 helper）+ export；`src/i18n/messages.ts` +3 locale 各一 `loginGamepass.windowError` key + module docblock 擴 `loginGamepass.*` 段說明 `windowError` 的 "固定 UX 字串而非 CommandError 透傳" 設計（對齊 `connectionLost` 既有做法）+ cancelled silent rationale；`src-tauri/src/commands/auth.rs::handle_gamepass_page_load` 補 6 行 `tracing::info!` structured log（`GamepassPageLoad.Finished` / `.NoPending` / `.SkipUrl` / `GamepassHarvest.Summary` / `GamepassCompletion.PendingToken` / `.RaceLost` / `.Success`）+ fn docblock 加 "Tracing schema" 小節 pin `step =` 值集合（live-test fault isolation 前置，user 確認 CP3+CP4 一起 live test 故 debt 現在補）；`tests/unit/pages/GamepassForm.spec.ts` 全改寫 **11 tests**（CP2 原 6 test migrated：step 1 → step 2 assertion update、HK guard 無 call 兩 cmd、connection-lost 不 call openGamepassWindow、Refresh 兩 cmd 都重跑、Back 不 call 兩 cmd、locale 切換；**+5 new**：openGamepassWindow fail → windowError banner step 1、success event → applyGamepassSession 被呼叫 + step=4 + nav `/accounts`、failed event → windowError + step rewind 1、cancelled event → 靜默 reset 0 + 無任何 ElMessage、unmount → 3 unlisten spy 各被 call 1 次），用 `vi.hoisted` mock `@tauri-apps/api/event::listen` 捕 `eventListeners`/`eventUnlistenSpies` registry + `fireEvent<T>` helper 觸發事件；quality gates：**frontend** vitest **177 passed**（從 CP2 baseline 172 增 5，15 test files 全 pass）/ typecheck 0 / lint 0 / prettier clean；**backend** cargo fmt 0 / clippy `--lib --all-targets --no-deps -- -D warnings` 0 / cargo test --lib **625 passed**（CP3 baseline，tracing-only diff 無 regression）/ cargo test 全套 0 failed / 3 ignored；`bindings.ts` 無 drift（CP4 未動 Rust command 簽名）；D5 所有 CP sanity check ✓ 等 user live test CP3+CP4 合測；不 commit（留 D12 batch）
  - **CP4 hotfix — gamepass error code split（debt fix before live test）** ✓ — CP4 結束 user 要求 `先把 debt 修正再一起測`，rationale：CP3 `open_gamepass_window` 兩條 precondition reject 路徑（`pending_gamepass` 為 `None` / `GAMEPASS_WINDOW_LABEL` window 已開）共用 `GAMEPASS_NOT_STARTED_CODE` (`auth.gamepass_not_started`)，frontend UX 因為 `wrapCommand` 一律 toast + GamepassForm `windowError` banner 視覺一致並無問題，但 backend log line `code=auth.gamepass_not_started` 在「window 已開」場景說謊，未來 postmortem 不分得出 root cause；fix 走 SRP（不同 precondition violation = 不同 wire-string）：**(a)** `commands/auth.rs` +`GAMEPASS_WINDOW_ALREADY_OPEN_CODE: &str = "auth.gamepass_window_already_open"` `pub(crate)` 常數（docblock 解釋為何跟 `GAMEPASS_NOT_STARTED_CODE` 拆開：操作員 log 歸因 + 將來 i18n toast 文案分流；前者 remediation = 重新 `login_gamepass_start`、後者 = user 先關 window），同步擴 `GAMEPASS_NOT_STARTED_CODE` docblock 加「scope 鎖在 empty `pending_gamepass`」段 + 反向 link，把舊版「double-open 也 surface 同 code」的舊話刪掉；double-open 分支從 `GAMEPASS_NOT_STARTED_CODE` 切到 `GAMEPASS_WINDOW_ALREADY_OPEN_CODE`、訊息改 `"GamePass login window is already open; close it before retrying."`（多帶 actionable 提示比舊 `"…is already open."` 對 frontend toast 拼接友善）；`open_gamepass_window` `# Preconditions` docblock 完全改寫成兩 bullet（每條列出 code / 觸發條件 / 復原動作），把 WPF parity 的「allocate exactly one `GamePassBrowser`」rationale 從敘述體挪進 bullet body，**(b)** +1 unit test `gamepass_window_already_open_code_is_distinct_from_not_started`：assert wire string == `"auth.gamepass_window_already_open"` + `assert_ne!` 跟 `GAMEPASS_NOT_STARTED_CODE` 字串（雙重保險：以後即使有人手滑把 const literal 改錯，至少 `assert_ne` 會擋住兩 code 字面塌成同字串），**(c)** `commands/error.rs` module-level code table 新加 "## Inline-raised codes — `auth.*`" 段 + 一行 row 文件化新 code（`Where=open_gamepass_window — prior WebView still alive` / Code / details `—` / Note 連回 const docblock）；故意只列新 code 不回填 `gamepass_not_started`/`qr_not_started`/`totp_not_pending`/`verify_not_started` 4 個既有 inline-raised code（避免擴大 scope，留作獨立 chore）—— 這段是「inline 表的奠基 row」，下次有人加新 inline code 自然延用，**(d)** `src/i18n/messages.ts` `errors.auth` namespace 三 locale 各補 `gamepass_window_already_open` key（zh-TW「GamePass 登入視窗已開啟，請先關閉再重新嘗試。」/ zh-CN「GamePass 登录窗口已开启，请先关闭再重新尝试。」/ en-US「GamePass login window is already open. Please close it before trying again.」）—— `KeysMatch<typeof zhTW, typeof zhTW>` compile-time guard 強制三 locale 同步，wrapCommand 走 `t('errors.' + code, fallback)` 自動拿到本地化 toast；GamepassForm `windowError` banner 仍維持 generic UX 字串（特定 code 的 toast 由 wrapCommand 處理 + banner 提供 Refresh affordance，這個雙層 UX 是 CP4 既定設計，不重新討論）；scope 故意不擴：(i) 不引入 "focus existing window" 的 UX 升級（將來如要做，再加新 helper `focus_or_open_gamepass_window`，跟此 debt fix 解耦）、(ii) 不順手回填其他 inline code 的 i18n（同樣規模控制），純粹只把「兩條路徑共用一 code」這條 SRP 違規拆乾淨；quality gates：cargo fmt 0 / cargo clippy `--lib --all-targets --no-deps -- -D warnings` 0 / `cargo test --lib` **626 passed**（CP4 baseline 625 +1 = 新 contract test）/ `cargo test` 全套 0 failed / 3 ignored / vitest **177 passed** unchanged（i18n key 增量不影響任何既有測試 assertion）/ typecheck 0 / lint 0 / prettier clean；不 commit（留 D12 batch）；D5 live test 解鎖
  - **D5 live-test hotfix — WebView cookie seeding + login_gamepass_start race guard（CP3+CP4 live retest unblock）** ✓ — CP4 hotfix commit 完 user 啟動合測，第一次 run 即噴：`Get SecretCode Success(8aa72c92b63344f9eebf06763ad7b25cf3e6bbeb) but get data fail: (0) No such auth key and secret code.`，WebView 停在 `return.aspx` 不自動關窗；兩個獨立 bug 並修（user 批「都修」並要求簡短解釋「為何剛剛登入成功會有這錯誤」）：**(Bug A) 主要 fix — 漏了 WebView cookie 種子**：WPF `Beanfun/Windows/GamePassBrowser.xaml.cs::OnWebViewReady`（L66-77）在第一次 navigation 前把 `bfClient.CookieContainer` 全部 cookies 透過 `CoreWebView2.CookieManager.AddOrUpdateCookie` 塞進 WebView2，讓 WebView 帶著「從 `get_session_key` 拿到的 portal session id」去訪 `login.beanfun.com/Login/Index?pSKey=…`，beanfun 才能把 OAuth round-trip 的 pSKey 對回 server 側那個 session 的登入 attempt；CP3 只做「建 window→navigate→page_load 收集 cookies 回 client」的反方向（inject_webview_cookies），**正方向（client → WebView 預載）從沒實作**，所以 return.aspx 找不到 server-side session 契合的 cookie → "No such auth key and secret code"；用 C# 解釋給 user：「WPF 的 WebView2 開窗時就先把舊客戶端的 session cookie 塞進去，beanfun 才認得是同一次登入；Tauri 版少了這步，`pSKey` 對不回 server 那邊的 session，所以掃完就卡」；**fix 設計**：(1) `services/beanfun/login/gamepass.rs` 新 `seed_webview_cookies_from_client<F,E>(client, sink: F) -> Result<usize, E> where F: FnMut(RawCookie<'static>) -> Result<(),E>`（SRP：helper 只負責「從 `BeanfunClient` jar 抽 unexpired cookie → yield owned `'static` clone 給 sink」，caller 決定是 `set_cookie` 還是 collect 到 Vec；`sink.Err` 第一次即中止不繼續做 short-circuit，docblock 註明 production 走 infallible closure / 測試走 collect closure，**不** swallow errors 在 helper 層；回傳 `Ok(count)` — 包含失敗之前已 yield 的數量），同步從 `login/mod.rs` re-export；`gamepass.rs` 再開 tests 4 個：`seed_yields_every_unexpired_cookie_to_the_sink` / `seed_on_empty_jar_is_a_no_op` / `seed_propagates_sink_error_and_short_circuits` / `seed_yields_owned_static_cookies`（pin 住 `'static` clone 契約 — caller 會 across thread 把 cookie 傳給 `WebviewWindow::set_cookie` dispatcher），(2) `commands/auth.rs::open_gamepass_window` 改走 `about:blank → seed → navigate` 三拍：（i）`WebviewWindowBuilder::new(..., WebviewUrl::External("about:blank"))`，因 Tauri 的 `build()` 是「async 直到第一次 navigation」沒有「pre-navigation hook」讓我們像 WPF 那樣在 `InitializationCompleted` 切入，用 `about:blank` 當 no-network 殼把第一次 real HTTP 推遲到我們種完 cookie；（ii）build() 完立即呼叫 `seed_webview_cookies_from_client(&client, |c| window.set_cookie(c.clone()))`，wrap `set_cookie` 的 closure **不**讓 helper 走 short-circuit：捕 Err 後 `tracing::warn!` 加 `cookie_name` + `cookie_domain` 然後 `Ok::<_, Infallible>(())`（對齊 WPF 對 `AddOrUpdateCookie` 沒 try/catch 的 per-cookie 容忍）+ 印 `GamepassWebViewSeed.Summary` 總量 log；（iii）`window.navigate(login_url)`，navigate Err 時自己 `pending_gamepass.write().await = None` + `window.close()` 避免 destroy hook 誤判 "user cancel" 再 emit cancelled，回 `ui.gamepass_navigate_failed`（新 `ui.*` inline-raised code）；**(Bug B) 副要 race guard fix — login_gamepass_start 加 window-alive pre-flight**：debug 時看出舊邊界：若 user A 視窗還活著，再點登入 → `login_gamepass_start` 舊版是「先清 pending_gamepass → mint 新 client + skey → 塞回 pending_gamepass」，之後 `open_gamepass_window` 被 CP3 建的 double-open guard 擋回（`auth.gamepass_window_already_open`）—— 但 `pending_gamepass` 已被新的 (client, skey) 寫滿了；user 只要一手動關「A 視窗」，`handle_gamepass_window_destroyed` 會看到 `pending_gamepass.is_some()` → 誤判「user 取消本次」→ 清 slot + emit `gamepass-login-cancelled`，本次新 attempt 被誤殺、且 user 面臨 silent reset；**fix**：`login_gamepass_start` 升泛型 `<R: tauri::Runtime>` 吃 `app: AppHandle<R>`，在「清三 slot」之前先 `app.get_webview_window(GAMEPASS_WINDOW_LABEL).is_some()` 檢查，若舊窗還在直接回 `GAMEPASS_WINDOW_ALREADY_OPEN_CODE`（複用 CP4 hotfix 的 wire-string，i18n key 已有 — DRY），**不碰** pending 與 cookie jar；docblock 加 "# Preconditions" 段完整寫明 race 現象 + WPF allocate-exactly-one `GamePassBrowser` parity；**bindings 影響**：`login_gamepass_start` 從泛型展開在 `commands/mod.rs::collect_commands!` 加 `::<tauri::Wry>` turbofish（同 `open_gamepass_window` 當年踩的 E0401 原因，註解一起更新成「兩 cmd 都 generic」），`bindings.ts` 的 TS 簽名不變（`AppHandle` 不在 specta export），`bindings_file_tests` 未紅；**error.rs 表格**：`## Inline-raised codes — auth.*` 擴一行 `login_gamepass_start → auth.gamepass_window_already_open`（note 寫明 race-guard 同 code 語意統一）+ 新 `## Inline-raised codes — ui.*` 段（當前只放 `ui.gamepass_navigate_failed` 一行；既有 `ui.window_create_failed` 刻意不一次回填，scope 控制，留獨立 chore），**i18n**：不動（`auth.gamepass_window_already_open` 三 locale 由 CP4 hotfix 已有、`ui.gamepass_navigate_failed` 走 fallback 英文，將來獨立 chore 補）；scope 故意不擴：(i) 不為 race guard 新增 mock-runtime integration test（helper + WPF 參考足夠 rational、`tauri::test::mock_app` 沒 `get_webview_window` 的真 window 建構 API 會造成測試假象），(ii) 不動 `handle_gamepass_window_destroyed` logic（race 被 pre-flight 解了，destroy hook 邏輯對於「真 cancel」場景依然正確，把兩條 diff 混進一個 fix 反而讓 review 難做），(iii) 不統一 `ui.window_create_failed` vs `ui.gamepass_navigate_failed` 的中途 cleanup 策略（前者 build 失敗時 window 還沒存在所以不需 pending clear；後者 build 後才失敗所以要清 + close，兩條現況本來就不同，強行 DRY 會扭曲語意）；quality gates：`cargo fmt` 0 / `cargo clippy --lib --all-targets --no-deps -- -D warnings` 0 / `cargo test --lib` **630 passed**（CP4-hotfix baseline 626 +4 = seed helper 4 tests）/ `cargo test` 全套 0 failed / 3 ignored / vitest **177 passed** unchanged / typecheck 0 / lint 0 / prettier clean；檔案觸動：`services/beanfun/login/gamepass.rs` / `services/beanfun/login/mod.rs` / `commands/auth.rs` / `commands/mod.rs` / `commands/error.rs`；不 commit（留 D12 batch）；等 dev server 重啟後 user 繼續 CP3+CP4 合測
  - **D5 live-test hotfix 2 — host-only cookie 的 Domain attribute rehydrate（根據 Option A 證據定位）** ✓ — diagnostic 上線後第一次 live retest 的 terminal log 揭露真因：`GamepassStart.JarDump` / `GamepassWebViewSeed.JarDump` 兩個 snapshot 都顯示 **兩條 `ASP.NET_SessionId` cookie 的 `domain` 欄位皆為 `None`**（而非預期的 `HostOnly("…beanfun.com")`），但 `total=2 seeded=2 failed=0` → 種是種進去了，但送給 WebView 的 RawCookie 根本沒帶 `Domain` 屬性，WebView2 (Windows) 對無 Domain 的 cookie 會靜默 drop，server 收不到對應 session id → `return.aspx` 回 "No such auth key and secret code"；root cause 是 `cookie_store::Cookie` 的 two-level domain 模型跟 C# `System.Net.Cookie` 有 silent divergence：cookie_store **struct field** `domain: CookieDomain`（enum，`HostOnly(host)` / `Suffix(host)` / `NotPresent` / `Empty`）是「邏輯 scope」，但 Cookie 透過 `Deref<Target=RawCookie>` 暴露的 `.domain()` **method** 回傳 `Option<&str>`，對 host-only cookie（`Set-Cookie` 沒帶 `Domain=` 屬性的那種）是 `None`—— WPF 從 `CookieContainer.GetCookies(uri)` 拿出的 `System.Net.Cookie.Domain` 對應的永遠是非空字串（host-only entry 會被 UA 補上 request host），所以 `CoreWebView2.CookieManager.CreateCookie(name, value, domain, path)` 永遠有得吃；Tauri 側 helper 先前是 `sink(cookie.deref().clone())` 把 RawCookie 原封轉手給 `window.set_cookie` → 對 host-only cookie 漏掉 Domain → WebView2 drop；hotfix 1 以為 bug 在「沒種子」（真的是，但那次修完後更深層的 attribute-rehydration 沒修），diagnostic 才把「種子送出去了但 WebView 不收」這層攤開；**fix 設計**：`services/beanfun/login/gamepass.rs::seed_webview_cookies_from_client` 改成先讀 **struct field** `cookie.domain: CookieDomain`，呼叫 `CookieDomain::as_cow() -> Option<Cow<str>>`（`HostOnly(h)` / `Suffix(h)` 都回 `Some(h)`、`NotPresent` / `Empty` 回 `None`），`Some(host)` → `raw.set_domain(host.into_owned())` 把 Domain 屬性 stamp 回 cloned `RawCookie` 再 `sink`、`None` → `tracing::warn!` 附 cookie name 然後 `continue`（不種，因為沒 Domain 的 cookie 進 WebView 也是 noop），`seeded` count 只記真的 yield 到 sink 的那些；這等效於把 host-only cookie widen 成 subdomain-match —— 跟 WPF `CookieContainer.GetCookies(uri)` rehydrate 行為結果一致，而 beanfun 的 session cookie 本來就在 `beanfun.com` eTLD+1 內流動，widening 不會把 cookie 外洩到外網域；docblock 加整段「Host-only cookie rehydration」解釋 `CookieDomain` 三態、WebView2 drop rationale、WPF parity；**diagnostic log 同步升級**：`commands/auth.rs::trace_cookie_jar` 把 `domain = ?cookie.domain()` / `path = ?cookie.path()`（method via Deref，Option<&str>）改成 `domain = ?cookie.domain` / `path = ?cookie.path`（struct field access，CookieDomain / CookiePath enum），加註解解釋選 field 不選 method 的原因是「enum 變體才能分辨 HostOnly vs Suffix，這是本次 regression 的決定性屬性」；`+2` unit tests：`seed_rehydrates_domain_attribute_on_host_only_cookies`（`RawCookie::parse("ASP.NET_SessionId=…; Path=/")` 無 Domain attribute → assert fixture 本身 `.domain() == None` → inject 進 jar → seed → 拿到的 RawCookie `.domain() == Some("login.beanfun.com")`；pin 住這次 regression），`seed_preserves_explicit_domain_on_suffix_cookies`（`Domain=tw.beanfun.com` 顯式 Suffix → seed → `.domain() == Some("tw.beanfun.com")`；pin 住 no-rewrite 契約）；scope 故意不擴：(i) **不改 UA / Accept-Encoding**（當初 user 選 Option A 就是為了拿證據，證據指向 cookie attribute 而非 UA，Option B 相關改動不做），(ii) **不刪或降級 diagnostic log**（helper 便宜且未來 debug 還會用，改成 `debug!` 會被預設 env filter 吞；等 D5 live 全綠後再評估），(iii) **`NotPresent` / `Empty` 分支不寫 unit test**（實務上 `iter_unexpired()` 回傳的 cookie 經過 `try_from_raw_cookie` 的 L176-196 保證不會是這兩個變體，defensive `continue` 無法透過 inject_webview_cookies 正常路徑構造出來；prop 測試只會重複驗 upstream invariant），(iv) **sink 簽名不變**（仍 `FnMut(RawCookie<'static>) -> Result<(),E>`，只是我們 yield 的 RawCookie 內容變了 — call site 不需動）；quality gates：cargo fmt 0 / cargo clippy `--all-targets --all-features -- -D warnings` 0 / `cargo test --lib` **632 passed**（diagnostic baseline 630 +2 = 新 regression tests，gamepass module 14/14 全綠）/ vitest / typecheck / lint / prettier 全 unchanged（純 Rust diff）；檔案觸動：`services/beanfun/login/gamepass.rs`（seed fn + docblock + 2 tests）/ `commands/auth.rs`（trace_cookie_jar domain/path field access + 註解）；不 commit（留 D12 batch）；**live retest 通過（2026-04-18 21:16）**：terminal 716502 log 驗證 `GamepassStart.JarDump` / `GamepassWebViewSeed.JarDump` 兩條 `ASP.NET_SessionId` 的 `domain` 從 `None` 改成 `HostOnly("tw.newlogin.beanfun.com")` / `HostOnly("tw.beanfun.com")`，seed 後 WebView 走完 `Login/Index?pSKey=…` → `accounts.gamania.com/oauth2/authorize` → `Login/SendLogin`（bfWebToken 尚未落地、WPF 同 station）→ `tw.beanfun.com/index.aspx`（portal landing，bfWebToken in jar）→ `GamepassLoginCompleted region=TW account_id="<deferred>"` → `GamepassCompletion.Success`；session 裝進 `AppState::auth` 並 emit `gamepass-login-success`，WebView 視窗自動關、frontend nav `/accounts`；"Get SecretCode Success(…) but get data fail" 徹底消失；D5 CP3+CP4 live verification ✓
  - **D5 live-test diagnostic — cookie-jar dump logging（Option A，同 bug 再現後加證據）** ✓ — cookie seed hotfix ship 完 user live 再跑 CP3+CP4，1-3 做完依然噴 `Get SecretCode Success(…) but get data fail: (0) No such auth key and secret code.` 且窗不關，terminal log 顯示 `GamepassWebViewSeed.Summary seeded=2 failed=0` + 多次 `GamepassCompletion.PendingToken`（bfWebToken 始終沒落地），meaning seed 執行面沒問題但「種子內容是否 WPF 會帶的那包」無法從現有 summary-count-only 的 log 判斷；比對 WPF（`Beanfun\Tools\BeanfunClient.cs` 的 `userAgent = "Mozilla/5.0 (Windows NT 10.0; WOW64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/55.0.2883.87 Safari/537.36"` + 明設 `Accept-Encoding: identity` + `GetCookies()` 只把「scope 到 `tw.beanfun.com` URI」的 cookies 拿給 WebView2 `AddOrUpdateCookie`）跟 Rust（`client.rs::DEFAULT_USER_AGENT = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36"` + `reqwest` 預設 Accept-Encoding + `seed_webview_cookies_from_client` 用 `iter_unexpired()` 所有 cookies）兩側存在三個可能造成 server-side session correlation 失敗的 divergence：(a) UA 字串差很大 / (b) Accept-Encoding 差異 / (c) 種子來源是否 scope 過；user 要求三條路 Option A（加 log 先拿證據）/ B（直接改 UA）/ Bonus（A+B），選 **A** —— 先上診斷 log 拿第一手 cookie 屬性證據再決策，不盲改 production；**fix 設計**：**不動** production behavior（UA / Accept-Encoding / seed scope 都保留原狀），只**純加 structured tracing**：(1) `commands/auth.rs` 新 `fn trace_cookie_jar(step: &'static str, client: &BeanfunClient)` 私有 helper（SRP：只負責讀 `client.cookie_store().lock()` + 對每條 `iter_unexpired()` 發一條 `tracing::info!` 記 `name / domain / path / secure / http_only / same_site` + 收尾發 `total` summary；mutex poisoned 走 `warn!` 不 panic — 診斷路徑不該綁 production 穩定性；**不印** cookie value — session 憑證等級機密，log sink 會寫檔）；關鍵欄位是 `cookie.domain()` —— 這裡呼叫的是 `cookie_store::Cookie` 自己的 `domain()`（shadow `Deref<Target=RawCookie>::domain()`）回傳 `CookieDomain` enum (`HostOnly("...")` / `Suffix(".beanfun.com")` / …)，這正是「WPF `CookieContainer.GetCookies(tw.beanfun.com)` 會留下哪些 cookie」的決定性屬性（host-only vs parent-domain 的拓朴不同），同樣的 `cookie.path()` 走 `cookie_store::CookiePath` shadow 版拿到 enum 而非 Option<&str>；docblock 完整寫明「2 call sites + WPF filter 哪條 attribute 對得起來」的 triage 邏輯，(2) `login_gamepass_start` 在 `get_session_key` 成功 `tracing::info!` 之後立即 `trace_cookie_jar("GamepassStart.JarDump", &client)`（time-0 快照 — 捕捉「portal redirect chain 剛走完時 jar 真正握在手上什麼」），(3) `open_gamepass_window` 在 `seed_webview_cookies_from_client` call 之前 `trace_cookie_jar("GamepassWebViewSeed.JarDump", &client)`（time-1 快照 — 跟 time-0 理論等值因中間無 HTTP，若不等就能直接定位「誰動了 jar」），(4) `open_gamepass_window` 在 `window.navigate(login_url)` 前 `tracing::info!(step="GamepassWebViewNavigate", url=%login_url, ...)` 記 `pSKey`-composed URL —— 多次 attempt 交錯時可從 log 配對「哪次 seed 對哪次 pSKey」避免 post-mortem 誤判；scope 故意不擴：(i) **不改 UA / Accept-Encoding**（Option B 留到拿到 A 的證據再決定，避免「log 看起來對但 prod 卻已改」的雙變量陷阱），(ii) **不寫 service 層 `dump_jar_for_tracing` helper**（當前只 GamePass flow 需要、其他 flow 沒 diagnostic 需求，Tracing 是 cross-cutting concern 就讓它停留在呼叫端 command 層，將來若 QR / TOTP 也要再抽），(iii) **不上 debug level**（用 `info!` 讓 default env filter 直接捕；解完 debt 後可調降或整塊刪掉 — helper & 呼叫點都集中在 `commands/auth.rs` 單檔，未來 revert 只動一檔），(iv) **不補 unit test**（log-only 無 behavioral contract 可 pin；新增 test 只是為了測 `tracing` subscriber 捕到 log —— YAGNI）；quality gates：cargo fmt 0 / cargo clippy `--lib --all-targets --no-deps -- -D warnings` 0（log-only diff 不碰 signature、`cookie_store::Cookie` 自帶 `.domain()` / `.path()` / `.secure()` / `.http_only()` / `.same_site()` 全通過型別檢查）/ cargo test --lib **630 passed** 跟 D5 live-test hotfix baseline 持平（helper 是私有診斷 fn 無 test，行為零變）/ typecheck / lint / prettier / vitest 177 全 unchanged；檔案觸動：**單檔** `src-tauri/src/commands/auth.rs`（+helper ~78 LOC 含 docblock、+`login_gamepass_start` 1 call、+`open_gamepass_window` 1 call + navigate pre-log ~11 LOC 含註解）；不 commit（留 D12 batch）；等 dev server 重啟讓 user live retest 拿 JarDump 證據
  - quality gates 走 4 次（每 CP 結束）+ debt hotfix 1 次 + live-test hotfix 2 次 + live-test diagnostic 1 次：fmt + clippy + cargo test --lib + npm test + lint + typecheck + prettier ✓
  - 不 commit（留 D12 batch 一起）
- [x] D6 `pages/LoginTotp.vue`（TOTP 6 碼輸入 → `auth.loginTotp`）
  - [x] D6.1 `composables/useOtpInputs.ts` 共用 composable（長度可變 N 格、auto-focus next、backspace focus prev、paste 跨格 spread、non-digit filter、填滿 emit `complete`）+ `useOtpInputs.spec.ts` 23 cases（instantiation / input filter / autofill / backspace / paste spread & cap / focus / reset）
  - [x] D6.2 `pages/LoginTotp.vue`（6 ElInput 獨立格 + WPF parity：`maxlength=1`、`IsDefault=True` Enter submit、第 6 格填完 auto-submit、GotFocus `select()`；post-submit nav：success→`/accounts` / `pendingVerify`→`/login/verify` / throw→`/login/id-pass`（WPF `errexit(err, 1)` parity）；Back→`/login/id-pass`；loading 綁 `AUTH_ACTIONS.LoginTotp`）+ `LoginTotp.spec.ts` 9 cases（renders / empty submit no-op / auto-submit on fill / success nav / verify nav / error nav / back link / paste spread auto-submit / locale switch）
  - [x] D6.3 router: `/login/totp` child + `ROUTE_NAMES.LoginTotp = 'login-totp'` + router spec 新增 declare + resolve case
  - [x] D6.4 i18n: `loginTotp.title` / `loginTotp.subtitle` 三 locale + module docblock namespace 說明；`submit` 複用 WPF `Login` key；`back` 複用 WPF `Back` key（top-left back link 跟其他 login children consistent，WPF `btn_cancel` 同語意 folded 進來做 DRY）；`errors.auth.invalid_totp` 已存在複用
  - [x] D6.5 Quality gates：vitest 211/211 + typecheck + lint + prettier ✓
  - [ ] 不 commit（留 D12 batch）
- [x] D7 `pages/LoginWait.vue`（loading + cancel）
  - [x] D7.1 `pages/LoginWait.vue`：CSS-only conic-gradient spinner + `t('MsgLogging')` + Cancel 按鈕 → `/login/id-pass`（WPF `return_page = loginPage` parity）；scope 選 Option A 最小 WPF parity，**拒絕 mockup 4-step progress**（WPF 沒有 + backend 無 phase event、前端時間軸 fake 違反「前端不說謊」）；docblock 解釋 worker-abort / bfAPPAutoLogin teardown 為何 deferred（Tauri commands atomic IPC、bfAPPAutoLogin 尚未 port）+ 為何現在掛 route（D9 AutoLogin bootstrap 之後 mount point）
  - [x] D7.2 router: `/login/wait` child + `ROUTE_NAMES.LoginWait = 'login-wait'` + router spec 新增 declare + resolve case
  - [x] D7.3 `LoginWait.spec.ts` 4 cases（MsgLogging + Cancel 文案、spinner `role="status"` a11y、Cancel → `/login/id-pass` 導航、locale 切換）
  - [x] 零 i18n 新 key：`MsgLogging` + `Cancel` 都是 WPF 現成 key 三 locale 都在
  - [x] D7.4 Quality gates：vitest 217/217 + typecheck + lint + prettier ✓
  - [ ] 不 commit（留 D12 batch）
- [x] D8 `pages/VerifyPage.vue`（captcha 圖載入 + 輸入 + `auth.submitVerify`）
  - [x] D8.0 hotfix（pre-flight 探勘揪出 P10.2 wire-string drift）：`stores/auth.ts` 把 `FLOW_CONTINUATION_CODES.VerifyRequired = 'auth.verify_required'` rename 成 `AdvanceCheckRequired = 'auth.advance_check_required'` 對齊 backend `LoginError::AdvanceCheckRequired` SSOT（`commands/error.rs` L394）—— 過去字串對不上導致 `pendingVerify` 永遠不會被觸發，但 unit test 全 mock 自己的字串所以一直綠；**同時補 `details.url` carry-through**：新 `readAdvanceCheckUrl(details)` helper + `advanceCheckUrl = ref<string | null>(null)` state，`loginRegular` / `loginTotp` 收到 `auth.advance_check_required` 時把 `result.error.details.url` 撈出存 store（TW 帶 url、HK 帶 null → backend `get_verify_page_info(None)` fallback 到 static TW URL，對齊 `BeanfunClient.Verify.cs` L23-25），success / submitVerify 成功 / logout 時清 `null`；i18n key `errors.auth.verify_required` → `errors.auth.advance_check_required` 三 locale；`services/invoke.ts` docblock 同步；`tests/unit/stores/auth.spec.ts` 22 cases rename + 加 4 個 url carry-through cases（TW url / null / malformed details / submitVerify success 清空）；同步修 `tests/unit/pages/IdPassForm.spec.ts` + `LoginTotp.spec.ts` 兩個 stale `'auth.verify_required'` mock（D8.0 不 fix 會讓那兩條 navigate-to-/login/verify 案例假綠：mock 字串老 → store 不 catch → fallthrough 到 surfaceCommandError → catch branch 帶去 `/login/id-pass` 而非預期 `/login/verify`，等於假驗證）
  - [x] D8.1 `pages/VerifyPage.vue`：`onMounted` → `auth.getVerifyPageInfo(auth.advanceCheckUrl)` → `auth.getVerifyCaptcha`，`lblAuthType` + base64 captcha 圖渲染（`<button>` wrap `<img>` 對齊 WPF clickable `imageCaptcha`）；submit 早 return 空輸入（`MsgAuthInfoEmpty` / `MsgCaptchaCodeEmpty` toast，對齊 WPF `Button_Click` L26-35）→ `auth.submitVerify(verifyCode, captchaCode)` → switch `result`：`success` toast `loginVerify.success` + `router.push('/login/id-pass')`、`wrong_captcha` toast `WrongCaptcha` + `refreshCaptcha()`、`wrong_auth_info` toast `WrongAuthInfo` + `refreshCaptcha()`、`server_message` 直接 toast `outcome.message`（對齊 WPF `MessageBox.Show(msg.Replace("\\n","\n"))`）+ `refreshCaptcha()`；click captcha 圖 → `refreshCaptcha()`（清 captchaCode field + 重新 fetch 圖，對齊 WPF `Button_Click_1`）；`getVerifyPageInfo` 失敗 → inline `loadFailed` banner（`LoadCaptchaFailed`）+ Retry 按鈕（重跑 bootstrap，避免 toast double-fire）；back link → `/login/id-pass`（對齊 WPF `Image_MouseLeftButtonDown` `return_page=loginPage`）
  - [x] D8.1 設計取捨：(a) **success 不 auto-resume login**：backend 採「no-secrets-over-IPC」policy（`commands/state.rs::PendingVerify` rationale），password 不跨 verify round-trip 留在 backend，故 success 時 SPA 只能把用戶送回 `/login/id-pass` 重輸（toast `loginVerify.success` 解釋為什麼回登入頁）；server-side AdvanceCheck 通過後是 IP/device fingerprint tracking 而非 cookie，第二次登入 server 不會再 prompt verify，UX 多一次密碼輸入但去掉 in-memory plaintext window，**刻意 deviation from WPF `do_Login`**（XAML L2657-2664 用 cached creds 重打）；(b) **`checkBoxRememberVerify` UI-only**：渲染 checkbox 對齊 XAML 結構但**不接持久化**——WPF 把答案塞 `Config.xml::Remember_Verify`（`MainWindow.xaml.cs` L1357）含個資（常是身分證末四 / email 末四），是否該存到 disk 需要 security review，跟 D3 `RememberPassword` 一樣留 D9 一起處理（D9 才接 account/credential persistence）；(c) **跳過 `windows/CaptchaWnd.vue`**：WPF `Beanfun/Windows/CaptchaWnd.xaml(.cs)` ripgrep 全 repo 0 call site（`new CaptchaWnd` / `CaptchaWnd.Show` 都搜不到），是 dead code，不 port；P12.1 D-step 表 10 view 變 9 view（CaptchaWnd 從 view list 除名）
  - [x] D8.2 router：`/login/verify` named child + `ROUTE_NAMES.LoginVerify = 'login-verify'` + router spec 新增 declare case + resolve `/login/verify` case + ROUTE_NAMES const assert + 頂部 docblock 標 D8 完成（child 列表 + `/login/verify` route 描述）
  - [x] D8.3 `VerifyPage.spec.ts` 14 cases：render（title/subtitle/auth-tip/inputs/captcha bitmap src/remember/AuthConfirm）、`advanceCheckUrl` carry-through（TW URL / null fallback 兩 case）、empty verify toast + skip IPC、empty captcha toast + skip IPC、success → `loginVerify.success` toast + nav `/login/id-pass`、wrong_captcha → toast + refresh + 清 captcha field、wrong_auth_info → toast + refresh、server_message → verbatim message + refresh、captcha image click → refresh + 清 captcha field + skip submit、back link → `/login/id-pass` + skip IPC、`getVerifyPageInfo` fail → load-failed banner + Retry button + 隱藏 submit、Retry → 重跑 bootstrap 後 banner 消失 + submit 出現、locale switch re-render；harness 與 `LoginTotp.spec.ts` / `IdPassForm.spec.ts` 同 pattern，stub Element Plus 元件 + memory router + Pinia 真 store
  - [x] D8.4 i18n：新 `loginVerify.{title,subtitle,success}` 三 locale（zh-TW/zh-CN/en-US）+ `messages.ts` 頂部 docblock 加 `loginVerify.*` 段落（解釋為什麼新 key、為什麼 chrome 字（`AuthInfoNeed` / `CaptchaCodeNeed` / `YourAuthInfoTip` / `MsgAuthInfoEmpty` / `MsgCaptchaCodeEmpty` / `WrongCaptcha` / `WrongAuthInfo` / `LoadCaptchaFailed` / `RefreshCaptcha` / `AuthConfirm` / `Remember` / `Back`）reuse WPF locale）；KeysMatch type guard 自動覆蓋 zh-CN / en-US 缺 key 編譯期阻擋
  - [x] D8.5 Quality gates：vitest 236/236（+19 = D8.0 hotfix +5 carry-through cases、D8.3 +14 VerifyPage spec）/ typecheck 0 / lint 0 / prettier ✓（`messages.ts` 一處 long en-US line 自動 wrap、`VerifyPage.vue` 一處 prettier reformat）
  - [ ] 不 commit（留 D12 batch）
- [x] D9 i18n 補 key audit ✓ — pre-flight 探勘揪出原計畫已過時：`login.*` / `verify.*` / `captcha.*` / `region.*` 4 個 namespace 在 D2-D8 各 D-step 已隨手分散落地（`loginShell`/`loginRegion`/`loginQr`/`loginGamepass`/`loginTotp`/`loginVerify` + `errors.{code}` + `themePreset.{name}` + reuse WPF `Taiwan`/`HongKong`/`CaptchaCodeNeed`/`RefreshCaptcha`/...），D9 純「再補新 key」實質 0 work；改寫成「**靜態 key-usage audit**」加防護網（user 指示「不要偷懶 + 最終要做完」）；二件不可逆 invariant 機械守護未來 D-step 不要產生 dead key 或 missing key drift
  - [x] D9.1 `tests/unit/i18n/key-usage.spec.ts` 新檔（**SRP** 跟 `index.spec.ts` 的 bootstrap concern 拆開 — 後者 runtime message loading / locale switch / translator wiring，前者 source-file 靜態分析）；用 vite 的 `import.meta.glob('/src/{pages,composables,components,stores}/**/*.{vue,ts}', { query: '?raw', eager: true })` 載原始檔案內容（build-time enumeration、無需 fs/fast-glob 依賴、`?raw` 對 .vue / .ts 同樣 work）；private `stripComments(src)` helper 同時剝 `/* */` / `//`（whole-line only — 避 URL `://` 誤殺）/ `<!-- -->` 三種註解，避 `messages.ts` docblock 寫 `t('errors.' + code, fallback)` 被 regex 抓成假 call site；regex `(?:[^a-zA-Z_$.]|^)t\(\s*(['"])([\w.]+)\1\s*[,)]` 三項精準收斂（leading char 排除 `.t(...)` 排掉 `i18n.global.t(...)` / `obj.t(...)` 但保留 destructured `const { t } = useI18n()` 用法、quote 限 `'`/`"` 拒 template literal 因 backtick 一定是 dynamic key、key 字元 `[\w.]+` cover 兩種 namespace 風格）；scan scope 故意只覆蓋 4 個 application 目錄（`pages` / `composables` / `components` / `stores`） — 排除 `services/invoke.ts`（用 `translator(...)` 不是 `t(...)`，docblock 提及 `t(...)` 已被 stripComments 剝掉但分目錄 SRP 更乾淨）/ `i18n/` 自己（declare 不 consume）/ `types/bindings.ts`（auto-generated、不 owns 任何 key）
  - [x] D9.2 同 spec 含 dead-key guard，但**升級成 fail-loud 而非 warn-only**（user 「不要偷懶 + 最終要做完」精神 — warning-only spec 是噪音沒人看）；只審 `FRONTEND_ONLY_MESSAGES['zh-TW']` 樹（不審 WPF generated locale，因為 `AccountList`/`Settings`/`EditAccount` 等未 port 頁的 key 自然 dead，會淹沒真信號 — 港 P12.X 該頁 port 後其 key 自動脫離 dead state）；`DYNAMIC_KEY_CONSUMERS` registry 顯式註冊 3 個 dynamic-key consumer 讓 audit 知道「這些 key 不是 dead 是被動態消費」：(1) `prefix: 'errors.'` ← `services/invoke.ts::surfaceCommandError` 透過 translator 用 `errors.${code}`、(2) `prefix: 'themePreset.'` ← `composables/useThemeColor.ts::THEME_PRESETS[i].name` 給未來 Settings 頁 swatch、(3) `literal: ['loginRegion.defaultBadge', 'loginRegion.totpHint']` ← `LoginRegionSelection.vue::TILES[i].hintKey` 動態 `t(tile.hintKey)`；每個 entry 帶 `reason` + `usedBy` 文件化「為何不是 statically traceable」防未來 maintainer 誤刪；自身 typo guard：另一 case assert literal-style consumer 的 `keys` 都真的存在於 zh-TW（防 `DYNAMIC_KEY_CONSUMERS` 自己手滑 typo 反而把真 key 標 dead 的反向 bug）；sanity guard：assert `Object.keys(APP_SOURCES).length > 5` + `ALL_LITERAL_CALL_SITES.length > 20` 防 `import.meta.glob` 因路徑搬動 silent return 0 件而讓兩條 invariant vacuously pass（disarmed audit 是最危險的 audit）
  - [x] D9.2 destructive verification — 開發中暫時把 `loginShell.deadTestOnly` 注入三 locale，跑 spec → dead-key guard 正確 fail loud `expected [ 'loginShell.deadTestOnly' ] to deeply equal []`；revert 後 4/4 tests 全綠；missing-key guard 共用同條 `LITERAL_T_RE` 邏輯，由 set membership `!ZH_TW_DECLARED_KEYS.has(key)` 即驗證
  - [x] D9.3 `messages.ts` 頂部 docblock 加「Static key-usage audit (D9)」段落 — 文件化兩條 invariant 跟 `DYNAMIC_KEY_CONSUMERS` extension protocol，下次 D-step 添加 banner / 刪 banner 時 maintainer 一眼知道該怎麼配合 audit
  - [x] D9.4 Quality gates：vitest **240 passed**（從 D8 baseline 236 + 4 新 key-usage tests）/ typecheck 0 / lint 0 / prettier auto-fix `tests/unit/i18n/key-usage.spec.ts` 一行 long type tuple 後 clean；無 production code 動到（純 spec + 一段 docblock）
  - [ ] 不 commit（留 D12 batch）
- [~] D9.5 ~~RememberVerify 持久化~~ **撤回** — D9 完成後做 WPF 探勘揪出設計缺陷：`MainWindow.xaml.cs::saveCurrentAccount` L1349-1360 是 `accountManager.addAccount(region, account, "", remember_pwd?password:"", remember_verify?verify:"", method, autoLogin)` **同一個 atomic call**，RememberVerify + RememberPassword + AutoLogin + LoginMethod 是同筆 record 的 4 個欄位，WPF 沒有「只 save verify、不 save password」的路徑；強行拆 D9.5 只做 verify 會引入 WPF 不存在的 partial-save semantics、後續 P12.2 接手還要回頭重構，**比一起做還複雜**；整包 credential persistence 推 P12.2 D1 跟 `AccountList.vue` + `Users.dat` 完整 read/write 一起做（atomic save semantics 對齊 WPF + 寫進去立即有 read 端對應、無 dead write 中間狀態）；P12.1 收尾僅留 D3 IdPassForm 的 `Remember` / `AutoLogin` checkbox 跟 D8 VerifyPage 的 `Remember` checkbox 為 UI-only state（已渲染、有 `data-test` hook、checkbox 仍可 toggle 但不持久化），D12 backfill 時 D3 / D8 docblock 統一改 forward-reference 指 P12.2 D1 而非「D9 才接 account/credential persistence」（後者是錯誤估計）；backend `users_dat::Account.verify` 欄位 P5/P6 已 ready，無需動 storage layer，**P12.2 D1 純加 backend command + frontend wire 即可**
- [x] D10 router catch-all + auth guard infrastructure ✓ — 兩條 cross-cutting 路由級 hook 一次裝齊，**P12.1 ship 0 個 protected route**（純 infrastructure 落地、P12.2 D-step 只要在 RouteRecord 加 `meta: { requiresAuth: true }` 即繼承行為），catch-all `/:pathMatch(.*)*` redirect→`/`→`/login` 從 D1 已 ship 維持不動
  - [x] D10.1 `router/index.ts` 加 `LOGGED_IN_LANDING_PATH = '/accounts'` const（centralize 4 個 D3-D8 `router.push('/accounts')` 的 forward reference target、P12.2 D2 register 真路由時只動一處 + 4 個 call site；docblock 註明「不做 `/post-login-stub` placeholder 因為 catch-all 已經保用戶在 login funnel 內、stub 只是 future tear-down 額外 D-step」）；`declare module 'vue-router'` augment `RouteMeta.requiresAuth?: boolean`（type-safe + `to.matched.some(r => r.meta.requiresAuth === true)` 之 IDE 補全）；新 `installRouterGuards(router, deps: RouterGuardDeps)` 工廠 — **兩條 hook 合一進入點**（`beforeEach` requiresAuth + `registerSessionExpiredHandler` bridge），rationale：(a) 共用 `RouterGuardDeps` contract、(b) 一定一起裝（單裝其一就半接 wiring）、(c) `main.ts` 只多一行不需記 ordering；`RouterGuardDeps = { isAuthenticated: () => boolean, clearSession: () => void }` 函式形式 contract（不 import Pinia store、避循環 import + 測試直接餵 `() => false` stub）
  - [x] D10.1 beforeEach defensive design — `to.fullPath !== '/login'` 嚴格 path 相等檢查在 destructive testing 下踩到 vue-router 的 infinite-redirect detector（hostile config 把 `/login/protected` 標 `requiresAuth: true` → 守衛 push 回 `/login` 但 query encode 成 `?redirect=/login/protected`），改成更穩健的 `to.path === '/login' || to.path.startsWith('/login/')` funnel-aware 檢查，docblock 註明 production 路由表沒任何 login child 是 `requiresAuth: true`、純粹防未來誤設沒 user-visible loop；session-expired bridge：`registerSessionExpiredHandler(() => { deps.clearSession(); void router.push({ path: '/login', query: { sessionExpired: '1' } }) })`，`?sessionExpired=1` query 預留 P12.X 給 `LoginPage.vue` 做 banner / toast「您的登入已失效」UX 之契約點（query 名稱定下後不動，未來頁可隨時加 reactive watcher）
  - [x] D10.2 `stores/auth.ts` +`clearSession()` action — 純本地 state 清理（session/pendingTotp/pendingVerify/qrChallenge/advanceCheckUrl 五件 sync wipe）、**故意不走 `withGuard`**（rationale 寫進 docblock）：(a) `clearSession` 是同步單 mutation 不會跟自己 race、(b) Vue reactivity single-thread 沒 in-flight 衝突問題、(c) 占 guard slot 反而會 deadlock — `loginRegular` 中 backend 突回 `auth.session_required` 觸發 session-expired bridge 時 `pendingAction === 'login.regular'`，若 `clearSession` 也想搶 slot 就被擋住；`logout()` 重構走 `clearSession()`（DRY — wipe 邏輯 single source of truth、之後新增 state field 只動 `clearSession`）
  - [x] D10.3 `main.ts` wire — `app.use(router)` 之後加 `useAuthStore()` + `installRouterGuards(router, { isAuthenticated: () => auth.isLoggedIn, clearSession: () => auth.clearSession() })`；arrow function 包裹確保 `this` binding 不會被 destructure 破壞（Pinia setup-store 的 method binding 不像 options-store 那麼穩固）；docblock plugin order 加第 5 條註明 `installRouterGuards` 必須晚於 Pinia + router、why
  - [x] D10.4 spec — `tests/unit/router/index.spec.ts` 加 11 個新 case：(a) `LOGGED_IN_LANDING_PATH` 是 `/accounts` 確認 forward reference 對齊、(b)-(f) `installRouterGuards` requiresAuth 5 個 case（公開路由 pass、未授權打 protected → /login、redirect query 正確 carry deep-link 含 query string、已授權直通、login funnel 內 protected 不 encode redirect query）使用 `createMemoryHistory` + 合成路由表隔離 production 配置、(g)-(h) session-expired bridge 2 個 case（handler 觸發 clearSession、`router.push('/login?sessionExpired=1')`）走 `surfaceCommandError({ silent: true })` 同 production fan-out path；`tests/unit/stores/auth.spec.ts` `logout` describe 拆兩塊 — 新 `clearSession` describe 3 case（同步 wipe 全 state / 不呼叫 backend logout 命令 / 不占 pendingAction slot 可在 in-flight action 中安全執行）+ logout describe 1 case 改成「呼叫 backend logout 後走 clearSession」確認 DRY 沒退化；`__resetInvokeRegistriesForTesting` 在 beforeEach/afterEach 隔離 invoke 層 module-level handler registry 防 leak
  - [x] D10.5 quality gates：vitest **251 passed**（D9 240 + 11 新 D10 case）/ typecheck 0 / lint 0 / prettier auto-fix 一檔後 clean / ReadLints 0；無 production code regression
  - [ ] 不 commit（留 D12 batch）
- [ ] D11 quality gates 全套
- [ ] D12 commit `feat(next): add P12.1 login flow views (10 views)` + chore Todo backfill

##### P12.2 — 帳號管理（10 view）

WPF mapping：

| Vue file | WPF source | View kind |
|---|---|---|
| `pages/AccountList.vue` | `Beanfun/Pages/AccountList.xaml` | Page (主畫面) |
| `windows/AddAccount.vue` | `Beanfun/Windows/AddAccount.xaml` | Dialog |
| `windows/ChangeAccount.vue` | `Beanfun/Windows/ChangeAccount.xaml` | Dialog |
| `pages/ManageAccount.vue` | `Beanfun/Pages/ManageAccount.xaml` | Page |
| `windows/AddServiceAccount.vue` | `Beanfun/Windows/AddServiceAccount.xaml` | Dialog |
| `windows/ChangeServiceAccountDisplayName.vue` | `Beanfun/Windows/ChangeServiceAccountDisplayName.xaml` | Dialog |
| `windows/ServiceAccountInfo.vue` | `Beanfun/Windows/ServiceAccountInfo.xaml` | Dialog |
| `windows/CopyBox.vue` | `Beanfun/Windows/CopyBox.xaml` | Dialog |
| `windows/Contract.vue` | `Beanfun/Windows/Contract.xaml` | Dialog |
| `windows/AccRecovery.vue` | `Beanfun/Windows/AccRecovery.xaml` | Dialog |

D-step 規劃（11 D-step + commit/backfill）：

- [x] D1 `pages/AccountList.vue` 殼 + 4 態 list + getServiceAccounts wiring ✓ — 詳細見下方 sub-step
- [x] D2 stored credentials persistence — `commands.save_account` atomic write（含 RememberPassword / RememberVerify / AutoLogin / Method 4 欄位 一起 save，對齊 WPF `accountManager.addAccount` semantics）+ IdPassForm 完整接 P12.1 D9.5 撤回的 D9 credential persistence；接 D8 VerifyPage RememberVerify 一起到位 ✓ — 詳細見下方 sub-step
- [x] D3 `windows/AddServiceAccount.vue`（+ `Plus` 按鈕 wire `account.addServiceAccount`）✓ — 詳細見下方 sub-step
- [x] D4 `windows/ChangeServiceAccountDisplayName.vue`（+ row context menu `Change Alias` wire）✓ — 詳細見下方 sub-step
- [x] D5 Get OTP flow — `account.getOtp(selected)` + clipboard + auto-paste preference（**對齊 WPF 預設 false**；user pre-flight 修正：原 bullet 寫「預設 true」是 SPA 改善初稿，但 D5 pre-flight 確認回到 WPF parity 原則「除非是 bug 否則照 WPF」）✓ — 詳細見下方 sub-step
- [x] D6 `windows/ServiceAccountInfo.vue`（+ row context menu `Account Info` wire）— 詳細見下方 sub-step
- [x] D7 帳號拖曳排序 — vuedraggable + `commands.setConfig("AccountOrder_<gameCode>", csv)` persistence；row drag handle 從 disabled stub 升級 — 詳細見下方 sub-step
- [x] D8 `windows/AddAccount.vue` + `windows/ChangeAccount.vue` — Users.dat record CRUD dialog ✓ — 詳細見下方 sub-step
- [x] D9 `pages/ManageAccount.vue` — 已存帳號列表 + import / export — sub-step 拆分 D9.0–D9.6 詳見下方 P12.2 D9 段落；mockup parity（單表 + region chip + 搜尋 + 單一 stats card）+ WPF parity（CRUD + import/export 走 backend `commands.import_records`/`export_records` 純文字 JSON）+ 12 個 design Q 全部 user-confirmed；reorder + multi-select + AES-encrypted backup 明確 deferred（reorder 等 backend 加 indexed insertion；AES backup 跟 D10 AccRecovery 一起做）
- [x] D10 `windows/AccRecovery.vue` + `windows/Contract.vue` + `windows/CopyBox.vue` — sub-step 拆分 D10.0–D10.8 全綠（D10.0 backend AES backup commands + 12 Rust test / D10.1 CopyBox + 6 case / D10.2 Contract + 5 case + AddServiceAccount refactor / D10.3 AccRecovery + 9 case / D10.4 ManageAccount toolbar 加「資料備份」+ 2 case / D10.5 AccountList row context menu 加 GetEmail + Tools stub 拆分 + 3 case + WPF parity 校正 / D10.6 i18n key audit 4/4 / D10.7 quality gates 全綠 vitest 408 + cargo lib 647 + 0 lint/format/clippy / D10.8 Todo.md backfill 完成）

##### P12.2 D10 — 三個 dialog: AccRecovery (AES backup) + Contract + CopyBox

WPF parity 來源：`Beanfun/Windows/AccRecovery.xaml(.cs)`（AES backup/restore：MD5(password) 派生 key、MD5("pungin") 派生 IV、AES-128-CBC + PKCS7 + base64 ciphertext）+ `Beanfun/Windows/Contract.xaml(.cs)`（純文字 ToS viewer）+ `Beanfun/Windows/CopyBox.xaml(.cs)`（generic `(title, value)` + Copy 按鈕）+ 三個 caller（`Pages/ManageAccount.xaml.cs::Button_Click` 「資料備份」/ `Windows/AddServiceAccount.xaml.cs::aContract_Click` 服務契約 link / `Pages/AccountList.xaml.cs::m_GetEmail_Click` row 右鍵 ContextMenu「檢查 Email」）。Mockup 來源：`beanfun-next/mockups/AccRecovery.html` / `Contract.html` / `CopyBox.html`（三個 mockup 與 WPF 全部設計衝突，pre-flight 已確認以 WPF parity 為準、mockup 純視覺語言保留）。

13 個 design Q（user 確認 `按你建議走 + 確保跟舊實作功能一樣 + SRP DRY`）：

| # | Decision | Rationale |
|---|----------|-----------|
| Q1 修 | **AccRecovery = WPF AES backup**（mockup 「帳號救援 link launcher」整個不做） | mockup link launcher 在 WPF 不存在；4 個 link 中 2 個 URL（忘記帳號 / 被盜回報）是 mockup 自編、不應違反「不編 URL」parity 原則 |
| Q2 修 | ManageAccount.vue toolbar 加單一「資料備份」按鈕 → 開 AccRecovery（AES） | 對齊 WPF `Pages/ManageAccount.xaml.cs::Button_Click` 唯一 entry |
| Q3 | AES wire format 1:1 port WPF：MD5(password) → AES-128 key、MD5("pungin") → IV、AES-128-CBC + PKCS7 + base64 | 可跨機 import 舊 WPF backup；docblock 警告 weak crypto 但語義是「跨機 portability」非「機密性」 |
| Q4 | Contract 純 viewer，**不**加 mockup acceptance gate | WPF 沒 gate logic；加 acceptance 引入 WPF 不存在的 state（誰知道用戶同意過？同意期限？mockup 沒回答） |
| Q5 | Contract 不加 print 按鈕（mockup 有） | 純 parity；print 涉及 WebView2 print API、scope creep |
| Q6 | Contract 接 prop `contract: string`（caller 注入）；text 來源 `account.getContract()`（已 ship） | dumb display；DRY — AddServiceAccount.vue 已用 store wrapper |
| Q7 | CopyBox = WPF generic `(title, value) + Copy`；mockup OTP 大字版**不做** | OTP 顯示已在 D5 inline UI 解決；getEmail 才需要 CopyBox |
| Q8 | reuse WPF `Copy` / `CopyFinished` / `CopyFailed` keys（已在 zh-TW.json） | DRY |
| Q9 | GetEmail entry 加在 AccountList **tools dropdown menu**（D10 最小 scope） | row 右鍵 5-item context menu (`CopyGameAccount`/`ChangeAccName`/`ChangePassword`/`AccInfo`/`OfficialSite`) port 是 D-step 等級工作、塞 D10 會超載 |
| Q10 | tools button 改成 `<el-dropdown>` + 3 個 menu item（Get Email / Change Game / 預留 More Tools） | 對齊 WPF tools menu；最小 chrome 改動 |
| Q11 撤回 | — | mockup link launcher 不做，無 URL 來源問題 |
| Q12 撤回 | — | 同 Q11 |
| Q13 | D10 不拆 D10a/b/c，內部 sub-step D10.0 ~ D10.7 | 對齊 D9 sub-step 顆粒度 |

**Scope 修正（D10.0 backend 不可避免）**：

WebCrypto SubtleCrypto **故意不支援 MD5**（MD5 已破），原 Q1 修「0 個新 backend command」假設純前端做 AES 是錯的；MD5 派生 key + IV 必須在 Rust 端做，故 D10.0 加 backend AES backup commands。技術約束、非設計選擇。

i18n key 重用 audit（D10 全部 frontend-only key 0 個新；keys 全部 reuse WPF generated locale tree）：

- AccRecovery：`DataRecovery` / `Password` / `Data` / `Export` / `Recovery` / `ExportDone` / `RecoverySuccess` / `RecoveryFailed` / `MsgDecryptFailed`（全已在 zh-TW.json）
- Contract：`TermsOfService` / `Cancel` / `Yes`（dialog title + footer button）
- CopyBox：`Copy` / `CopyFinished` / `CopyFailed` / `AuthEmail`（caller 注入 title）
- ManageAccount toolbar 「資料備份」按鈕：`DataBackup`（已在 zh-TW.json）
- AccountList tools dropdown menu items：`CheckEmail`（已在 zh-TW.json）+ `accountList.changeGame`（已在 frontend-only） + `accountList.moreActions`（已在 frontend-only）

D-step 規劃（D10.0 ~ D10.7）：

- [x] D10.0 backend AES backup commands — Cargo deps `md-5 = "0.10"` + `aes = "0.8"` + `cbc = "0.1"`（RustCrypto 體系，跟既有 `des` / `cipher` / `sha2` 同源、no openssl）；新 module `services/beanfun/aes_backup.rs` 暴露 `encrypt_records(plaintext: &str, password: &str) -> Result<String, BackupError>`（回 base64）+ `decrypt_records(b64: &str, password: &str) -> Result<String, BackupError>`（回 plaintext JSON）— 純 stateless function、無 IO，便利 unit test；wire format 1:1 對齊 `Beanfun/Windows/AccRecovery.xaml.cs`：key = `MD5(password.as_bytes())`、IV = `MD5(b"pungin")`、`Aes128CbcEnc/Dec` (cbc crate, RustCrypto 標準) + Pkcs7 padding + base64 (existing dep)；Rust unit test 5 case：(1) roundtrip empty plaintext / (2) roundtrip 真實 Users.dat JSON / (3) wrong password decrypt → `BackupError::DecryptFailed` / (4) malformed b64 → `BackupError::InvalidCiphertext` / (5) WPF wire-format reference vector — 把 `password="test"` + 一段 fixed plaintext 用 WPF 自己跑出 ciphertext（從 dotnet fiddle / 手工驗），把那個 base64 hard-code 在 test fixture，assert `decrypt(fixture, "test") == fixed plaintext`（cross-impl 互通驗證、防未來 RustCrypto 行為漂移）；新 `commands::storage::backup_export(password: String) -> Result<String, CommandError>`（內部走 `load_records_with_legacy_migration` → `storage::export_records(&records)?` → `aes_backup::encrypt_records(&json, &password)` → return base64）+ `commands::storage::backup_restore(password: String, ciphertext: String) -> Result<Vec<Account>, CommandError>`（內部 `aes_backup::decrypt_records(&ciphertext, &password)` → `storage::import_records(&users_dat, &plaintext)` → return post-import list，鏡 `import_records` 的 contract 讓 frontend 同樣 re-set `accounts.value`）；2 個新 specta TS binding（`commands.backupExport` + `commands.backupRestore`）；commands docblock 完整覆蓋 weak-crypto rationale + WPF wire format ref + frontend caller chain
- [x] D10.1 `windows/CopyBox.vue` — generic `(title, value) + Copy`；props `visible: boolean` + `title: string` + `value: string`、emits `update:visible`；mount-always pattern；template：`el-dialog` 接 v-model:visible + `:title="title"`，body 一個 read-only `el-input` 顯示 value + 一個 `bf-btn-gradient`「複製」按鈕；click → `navigator.clipboard.writeText(value)` → success 用 `t('CopyFinished')` toast，error 用 `t('CopyFailed')` toast；reuse WPF keys；style 套 `bf-glass-panel`；docblock 對齊 WPF `Beanfun/Windows/CopyBox.xaml(.cs)` 簡述、明寫「OTP 大字版 mockup 不做（D5 inline 已解）」；spec 6 case：(1) prop title + value 渲染 / (2) Copy click → clipboard.writeText 收到 value / (3) Copy 成功 → `CopyFinished` toast / (4) clipboard reject → `CopyFailed` toast / (5) close 按鈕 → emit `update:visible(false)` / (6) v-model:visible 鎖死（visible=false 時 dialog 不渲染）
- [x] D10.2 `windows/Contract.vue` — pure viewer；props `visible: boolean` + `text: string` + `title?: string`（default `'TermsOfService'` 經 i18n 解析）、emits `update:visible`；template：`el-dialog` 接 v-model:visible + 自管 header（CircleClose 按鈕）+ body `<pre>` 包 `bf-custom-scrollbar`、footer 一個 Confirm 按鈕；style 套 surface-container-low + outline-variant token；docblock 對齊 WPF `Beanfun/Windows/Contract.xaml(.cs)` 並明寫「mockup acceptance gate（Agree checkbox + Agree/Disagree 按鈕）**不做** — agreement gate 本來就在 `AddServiceAccount.vue`、Q9 純 parity」；refactor `windows/AddServiceAccount.vue` 把 inline contract `<el-dialog>` 整段刪掉、改 `<Contract v-model:visible="contractVisible" :text="contractText" />`、移除 `Document` icon import + `add-svc__contract-body` style；spec 5 case：(1) `text` 渲染（含換行、indent、空行 verbatim 經由 `<pre>`）/ (2) default title resolve 到 `TermsOfService` i18n 字串 / (3) custom 非-i18n-key title prop 直接 verbatim 渲染 / (4) close 按鈕 → emit `update:visible(false)` 並 unmount / (5) Confirm 按鈕 → emit `update:visible(false)` 並 unmount；`AddServiceAccount.spec.ts` selector 同步更新（`contract-dialog` / `contract-text`），其餘 12 case 全綠
- [x] D10.3 `windows/AccRecovery.vue` — AES backup/restore dialog；props `visible: boolean`、emits `update:visible` + `restored`（restore 成功 → 父層可選 refresh list）；UI 對齊 WPF `AccRecovery.xaml`：(a) `el-dialog` 接 v-model:visible + `:title="t('DataRecovery')"`、(b) `el-input` password（type=password、`label="t('Password')"`）、(c) `el-input` data（type=textarea autosize 4-12 row、`label="t('Data')"`、placeholder 提示「Export 會自動填入；Recovery 請貼上既有 ciphertext」）、(d) footer 兩個 `bf-btn-gradient`「`t('Export')`」/ `bf-btn-secondary`「`t('Recovery')`」；行為：Export click → `commands.backupExport(password)` → 回填 `data.value = ciphertext` + `t('ExportDone')` success toast；Recovery click → `commands.backupRestore(password, data)` → success → `t('RecoverySuccess')` toast + emit `restored` + 自動關閉 + reset password / data；errors → 後端 `aes_backup.decrypt_failed` mapped to `MsgDecryptFailed` toast、`aes_backup.import_failed` → `RecoveryFailed`（透過 errors.* namespace + i18n fallback）；docblock 完整對齊 WPF + 警告 weak crypto + 補 `MsgDecryptFailed` / `RecoverySuccess` / `RecoveryFailed` 已在 WPF locale；style 套 `bf-glass-panel`；spec 9 case：(1) initial 渲染（password / data input 空、兩按鈕 disabled when password 空）/ (2) Export happy → mock backupExport ok → data 回填 + ExportDone toast / (3) Export 後端 fail → error toast + data 不變 / (4) Recovery happy → mock backupRestore ok → RecoverySuccess + emit restored + 關閉 + reset / (5) Recovery wrong password → backend `aes_backup.decrypt_failed` → MsgDecryptFailed toast、不關閉 / (6) Recovery malformed b64 → backend `aes_backup.invalid_ciphertext` → 對應 errors.* toast / (7) Recovery import failed → backend `storage.import_*_failed` → RecoveryFailed toast / (8) close 按鈕 → emit `update:visible(false)` + reset / (9) password 空時 Export / Recovery disabled
- [x] D10.4 `pages/ManageAccount.vue` 加「資料備份」按鈕 — toolbar 上現有 Import / Export / Add 三按鈕之間插入第 4 個按鈕「資料備份」`bf-btn-secondary`（icon: `Lock`），click → `accRecoveryVisible.value = true`；mount `<AccRecovery v-model:visible="accRecoveryVisible" @restored="handleRestored" />`（restored handler 重新派生 loadState，dialog 已直接寫回 store.accounts、parity WPF `loginMethodInit`）；docblock 解釋為何 backup 跟 plaintext export 並列（plaintext = 移交給人看 / git；AES backup = 跨機 portability 保護密碼欄位）；spec 既有 ManageAccount.spec.ts patch 加 2 case：(1) 點「資料備份」按鈕 → AccRecovery dialog visible flips true / (2) AccRecovery emit `restored` → `loadState` 從 empty 翻成 ready、列表重新渲染（store.accounts 直接寫 + handleRestored 重新派生，避免一筆多餘 IPC）
- [x] D10.5 `pages/AccountList.vue` row context menu 加 GetEmail item + Tools button stub 拆分 — **WPF parity 校正**：原計畫的「tools dropdown 含 GetEmail / ChangeGame / More Tools」違反 WPF（WPF `btn_Tools` 是單一按鈕、conditional Visible、開 game-specific MapleTools/KartTools 視窗；WPF `m_GetEmail` 在 row 右鍵 context menu 裡），改成嚴格 WPF parity：(a) 在現有 row context menu（D4 `<el-dropdown-menu>`）加第 3 個 `<el-dropdown-item>`「檢查驗證信箱」(`CheckEmail` + `Message` icon)，click → `wrapCommand(commands.getEmail())` → 成功 → `copyBoxTitle = t('AuthEmail'); copyBoxValue = email; copyBoxVisible = true`、失敗 → wrapCommand 自動 toast、dialog 不開；(b) Tools button click 從誤接的 `handleChangeGame` 改成獨立 `handleTools` stub（`makeStub('Tools button (game-specific tools window)')`），docblock 說明 game-specific 啟動 + conditional visibility 等 P12.3 game switching；(c) page level mount `<CopyBox v-model:visible="copyBoxVisible" :title="copyBoxTitle" :value="copyBoxValue" />`；docblock 對齊 WPF + 解釋「single set of refs（只有一個 dialog modal、不需要 per-row Map）」；spec 既有 AccountList.spec.ts patch 加 3 case：(1) GetEmail happy → `commands.getEmail` 被呼叫 + CopyBox visible 翻 true + title=AuthEmail i18n + value=email / (2) GetEmail error → wrapCommand error toast + CopyBox 維持關閉（visible/title/value 全空，匹配 WPF「IPC fail 不開 dialog」） / (3) Tools button click → 自己的 stub marker `[AccountList] Tools ...`、不會誤觸發 Change Game（regression guard）
- [x] D10.6 i18n key audit — `tests/unit/i18n/key-usage.spec.ts` 4/4 過：D10 新增唯一 frontend-only key `accRecovery.dataPlaceholder` 被 `AccRecovery.vue` 直接 consume（非 dead）、所有 WPF-locale key（`DataBackup` / `Lock` icon / `CheckEmail` / `AuthEmail` / `MsgDecryptFailed` / `RecoverySuccess` / `RecoveryFailed` / `ExportDone` / `Export` / `Recovery` / `Data` / `Password` / `DataRecovery` / `Copy` / `CopyFinished` / `CopyFailed` / `TermsOfService`）皆已存在於 zh-TW/zh-CN/en-US（D10 起點即驗證），missing-key/dead-key 雙閘 0 violation
- [x] D10.7 quality gates 全綠 — vitest 408/408（D9 baseline 383 + D10 case 25 = 預期值精準）/ vue-tsc 0 / eslint 0（修 2 個自己引入的 violation：`AccountList.vue::handleGetEmail` 移掉 unused `_a` param 並補 docblock 說明 WPF `getEmail()` 不吃 account 參數；`Contract.vue` defineOptions name 改 `ContractDialog` 過 `vue/multi-word-component-names`、檔名與 import path 維持 `Contract.vue` 對齊 WPF `Contract.xaml`）/ prettier 0（5 個檔 reformat：AccountList.vue / AccRecovery.vue / Contract.vue / AccRecovery.spec.ts / CopyBox.spec.ts）/ cargo check 0（含 D10.0 加的 `md-5` / `aes` / `cbc` + `cipher` 的 `block-padding` `alloc` features）/ cargo test --lib 647/647（含 12 個 aes_backup test：key/IV 派生、roundtrip、empty plaintext、wrong password、malformed b64、surrounding whitespace、PKCS7 padding boundary 與 4 個 WPF wire-format reference vector）/ cargo fmt --check 0 / cargo clippy --all-targets -D warnings 0
- [x] D10.8 Todo.md backfill — D10.0 ~ D10.7 全標 ✓ + line 1287 D10 主 checkbox ✓
- [x] D11 Quick actions wire — Gash balance refresh（**only**：Member Center / Customer Service 確認 defer 至 P12.4 WebBrowser 統一做，因為兩者都需要 in-app webview 視窗 + URL 派生 + WebToken cookie inject、提早做會浪費 wire-up 工作）—— sub-step：

  - [x] D11.0 預檢 — 確認 backend `commands.getRemainPoint` 已 ship（P11，回 `Result<i32, CommandError>`）+ store wrapper `account.getRemainPoint(force=false)` 已快取（D11 用 `force=true` 對齊 WPF `m_UpdatePoint_Click` 永遠 re-IPC）；確認 i18n key `GashRemain` (`樂豆: {0} 點`) + `GashRemainInGame` (` (遊戲內 {0})`) 已在 zh-TW/zh-CN/en-US；確認 `useAuthStore().session?.region` 在 AccountList 已 import + 可用（line 137）
  - [x] D11.1 `pages/AccountList.vue` Gash refresh 接線 — `handleRefreshBalance` stub → 真實 handler：(a) 設 `refreshing.value = true` + 早退 guard（in-flight 期間 click 是 no-op、避免 race）、(b) `await account.getRemainPoint(true)`（force=true、`wrapCommand` 自動 toast 失敗）、(c) finally `refreshing.value = false`；新 `refreshing = ref(false)`；refresh 按鈕 `:disabled="refreshing"` + 加 modifier class `account-list__balance-refresh--spinning` 啟動 CSS keyframes `account-list-spin` 0.9s linear infinite（套在 `.el-icon` child）；displayed value 從靜態 `gashBalancePlaceholder` 換成 `formattedRemainPoint` computed：`remainPoint === null` → `t('accountList.gashBalancePlaceholder')`（仍 `—`）/ region=='TW' OR remainPoint==0 → `t('GashRemain', [value])` / 其他 → `t('GashRemain', [`${value}${t('GashRemainInGame', [Math.floor(value / 2.5)])}`])`（**WPF parity 1:1**：`MainWindow.xaml.cs::updateRemainPoint` L1716-1721 只在 `LoginRegion != "TW" && remainPoint != 0` 才疊 `GashRemainInGame`，且 `{0}` 是 `floor(remainPoint / 2.5)`）；mount 時自動 lazy fetch 一次（`onMounted` 既有 `loadList()` 之外加 `void account.getRemainPoint().catch(() => {})`，silent 失敗、不阻塞 list 初始化、對齊 WPF login 完即帶 `bfClient.remainPoint` 的 UX）；docblock 對齊 WPF L137-140 + L1377 + L1716-1721 並解釋為何 SPA 補初始 fetch（WPF login flow 自帶、SPA login flow 沒做）+ 為何不疊 success toast（refresh button 視覺更新本身就是 feedback、加 toast 是 SPA-only chrome）+ 為何 mount fail silent（保留 page-level error banner 給真正阻 page 渲染的 `loadList()` 失敗、避免雙語意混淆）
  - [x] D11.2 `tests/unit/pages/AccountList.spec.ts` patch 加 5 case：(1) TW session、回 1234 → 「樂豆: 1234 點」+ mount IPC 一次 / (2) HK session、回 1234 → 「樂豆: 1234 (遊戲內 493) 點」（floor 1234/2.5 = 493） / (3) HK session、回 0 → 「樂豆: 0 點」（**不**疊 in-game suffix、WPF carve-out parity） / (4) refresh button click → IPC 再呼叫一次（共 2 次）+ 期間 `disabled=true` + spinning class 套上、resolve 後還原 / (5) mount IPC 失敗 → wrapCommand toast、page 仍 `ready`、列表照常渲染、displayed 維持 placeholder「—」；同時 beforeEach 加 `commands.getRemainPoint` default `mockReturnValue(ok(0))` 防止其他 case 被 mount auto-fetch 觸發 wrapCommand 錯誤路徑
  - [x] D11.3 quality gates 全綠 — vitest 413/413（D10 baseline 408 + D11 case 5 = 預期值精準）/ vue-tsc 0 / eslint 0 / prettier 0 (1 個檔 reformat：AccountList.vue) / cargo unchanged + Todo backfill
- [ ] D12 commit `feat(next): add P12.2 account management views` + chore Todo backfill

> D2-D12 細節等 D1 落地後與使用者 sync 再展開；當前僅鎖 D1 的 sub-step。

---

##### P12.2 D1 — `pages/AccountList.vue` 殼 + 4 態 list + getServiceAccounts wiring

WPF parity 來源：`Beanfun/Pages/AccountList.xaml(.cs)` + `Beanfun/MainWindow.xaml(.cs)` 中央區（game info bar + Logout + Gash + OTP）。

- [x] D1.1 mockup 落地 + WPF 重審 — 把使用者在 chat 中提供的 `IdPassForm.html` / `AccountList.html` / `QrForm.html` / `Settings.html` 寫入 `beanfun-next/mockups/`（修正 P-1 「Stitch 已完成」誤宣告 — line 1363 同步改成「使用者於 P12.2 D1.1 補入庫」並把 4 個檔個別列項）；同時揪出 D10 漏修 bug：`auth.clearSession()` 沒呼叫 `account.clearSessionData()` → 重登時舊 user 的 service-account list 會閃過一輪 fresh fetch；發現現有 `stores/account.ts`（P11 已寫好、含 `getServiceAccounts` / `refreshServiceAccounts` / `selectedSid` / `selectedServiceAccount` / `clearSessionData`）— **不另開 store**
- [x] D1.2 design utility 抽出（DRY 護欄）— 新 `src/styles/design-tokens.css`（`--bf-primary` / `--bf-primary-container` / `--bf-on-primary` 橋接到 `--el-color-primary` ladder runtime；surface palette / 語意色 / radius / shadow / blur / motion 一次到位）+ `src/styles/utilities.css`（`.bf-mica-bg` / `.bf-glass-panel` / `.bf-glass-floating` / `.bf-glass-card` / `.bf-ghost-border` / `.bf-ambient-glow` / `.bf-btn-gradient` / `.bf-btn-secondary` / `.bf-btn-ghost-icon` / `.bf-text-gradient` / `.bf-custom-scrollbar`）；`main.ts` 在 ELP css 之後 import 兩檔（順序：tokens 先 declare `--bf-*` vars，utilities 才 consume）；`LoginPage.vue` refactor 用新 utility（移除 scoped 內 rgba/blur 重複）—— P12.2 後續每個 page / dialog 直接 reuse、避免 mockup → vue port 變成 copy-paste fest
- [x] D1.3 D10 bug 修復 — `RouterGuardDeps` 新增 optional `clearAccountSession?: () => void` callback（optional 是為了不破壞 D10 era spec）；`registerSessionExpiredHandler` callback 改成 `deps.clearSession() → deps.clearAccountSession?.() → router.push({ path: '/login', query: { sessionExpired: '1' } })`；`main.ts` 多注入 `account.clearSessionData()` 給新 callback；router docblock 更新 SRP rationale（router 不知哪個 store 背 session-scoped state、`main.ts` 才是每個 store 都在 scope 內的合適 composition 點）；新 2 個 router spec：order 驗證（auth 先 clear、account 後 clear）+ 向後相容驗證（沒提供 clearAccountSession 時 graceful skip）
- [x] D1.4 router 加 `/accounts` named route — `LOGGED_IN_LANDING_PATH = '/accounts'` 連 D10 已 ship 的 const、新 `ROUTE_NAMES.Accounts = 'accounts'`、`meta: { requiresAuth: true }`（首個 protected route，正式 exercise D10 guard infrastructure）；router spec 多 4 個 case：route 表 length 從 3 → 4、`/accounts` resolve 確認、`LOGGED_IN_LANDING_PATH ↔ routes` 契約鎖、prod-route + guard 整合測試（unauth → redirect /login + `?redirect=/accounts`、auth → 真的進得去）
- [x] D1.5 `pages/AccountList.vue` 完整切版 — header（page-scoped title + subtitle，不重 P12.4 TitleBar 範圍）/ Game Info Bar（icon / name / 線上狀態 / tools / Logout，**Logout 是真的接通**、其他 stub）/ Start Game button（stub）/ Quick actions row（Gash balance + Member Center + Customer Service 全 stub）/ **Service Accounts list 4 態 real**（loading / error+retry / empty / non-empty rows，row click → `selectedSid`、banned row no-op for parity with WPF `lstViewAccount_SelectionChanged`）/ Add Service Account button（stub）/ OTP section（autoPaste checkbox + readonly OTP field + Get OTP + copy 全 stub）；`makeStub(label)` helper 統一 stub 格式（dev console grep `[AccountList]` 一次找出所有未 wire affordance）；新 `accountList.{title,subtitle,serviceAccountsHeading,accountCount,loading,empty,loadFailed,retry,statusOnline,statusBanned,gashBalance,gashBalancePlaceholder,refreshBalance,memberCenter,customerService,autoPaste,otpHeading,otpPlaceholder,copyOtp,toolsButton,changeGame,moreActions,dragHandle}` 三 locale；reuse WPF key `GameStart` / `Logout` / `LogoutConfirm` / `Cancel` / `AddServiceAccount` / `GetOtp`；mockup conflict 處理：top header（Beanfun Next + help / settings / close）跟 bottom Play button 兩個 mobile-pattern affordance 全部 omit（前者 P12.4 TitleBar 才畫、後者跟 Start Game 重複）；logout orchestration 在 page handler（confirm → `auth.logout()` → `account.clearSessionData()` → `/login`）— SRP 跟 D1.3 router-guard 同 rationale，未來若有第二個 page 也按 logout 再抽 `useLogoutFlow()` 共用
- [x] D1.6 vitest — 新 `tests/unit/pages/AccountList.spec.ts` 8 case（loading 態 / empty 態 / non-empty 3 row 渲染（含 banned 列 line-through + status copy）/ error 態 + retry 復原 / row click select / banned row click no-op / logout 完整鏈 / logout 取消 hard cancel）；`stores/account.ts::getServiceAccounts` 已有 P11 D-step 完整 spec coverage **不重複造**；router spec 加 `/accounts` integration（見 D1.4）
- [x] D1.7 quality gates 全綠 — vitest 264 passed（含新 8 case + router 4 case，總 +12）/ vue-tsc 0 / eslint 0 / prettier 0 / cargo check 0；i18n key-usage 靜態審計捕到 2 個 dead key（`accountList.gameStatusUnknown` / `accountList.selectAccountFirst`，原本為未來 D-step 預留）→ YAGNI 移除（未來 D-step wire 時會把該 D-step 用到的 key 一起加）
- [x] D1.8 Todo.md backfill — 修 line 1363 「Stitch 已完成」誤宣告；新增 P12.2 D-step plan + D1 sub-step 表（本段）
- [ ] D1 不單獨 commit — 留 P12.2 D12 統一 `feat(next): add P12.2 account management views` batch（與 P12.1 D12 同 convention）

##### P12.2 D2 — Stored Credentials Persistence（接 P12.1 D9.5 撤回的 D9 credential persistence + D8 VerifyPage RememberVerify）

WPF parity 來源：`Beanfun/MainWindow.xaml.cs::SaveLoginCredentials` (L1334-1363) + `loginMethodInit` (L1095-1191) + `loginMethodChanged` (L1054-1092) + `loginWorker_RunWorkerCompleted` LoginAdvanceCheck branch (L1472-1500) + `totpWorker_RunWorkerCompleted` (L1583-1600) + `Beanfun/Helper/AccountManager.cs::addAccount`（atomic 7-field upsert）。

設計決策（pre-flight 已 sync 使用者）：

| Q | 決策 | Rationale |
|---|---|---|
| Q1 HK QR 是否持久化 password | **B：不持久化** | WPF 為 HK QR 流程寫入 `(HK, "")` 空 account 的 garbage record，下游所有 use（boot 方法回復、IdPassForm dropdown、ManageAccount、import/export）都對該空 row no-op 或產生 hidden state inconsistency；effective behavior 0 差異，UX 反而更乾淨 — 對齊使用者規則 7（不擅用 workaround）+ 8（SRP/DRY） |
| Q2 IdPassForm mount 時 prefill | **是** | 對齊 WPF `loginMethodChanged` L1062-1085：`config.AccountID` + `account.findStoredAccount` 找 record，prefill account / password / remember=true / autoLogin=record.auto_login |
| Q3 `saveLoginCredentials` 邏輯放哪 | **`account` store action** | 對齊 WPF `accountManager.addAccount` SRP：record persistence 邏輯歸 record store，不外洩到 component；`auth` store 只負責 session/flow state |
| Q4 VerifyPage mount 時 prefill verify | **是** | 對齊 WPF L1482-1487 / L1595-1600：用 `auth.loginIntent.{region,accountId}` 找 stored record，prefill verify code + remember |
| Q5 `LoginMethod` enum 共用 | **新檔 `src/constants/login.ts`** | TypeScript 不像 backend 有 `enum LoginMethod`；新檔避免 number magic 散落（4 處 frontend：account store / IdPassForm / LoginTotp / VerifyPage），對齊 WPF `enum LoginMethod { Regular = 0, QRCode = 1, GamePass = 2 }` |

D-step 規劃（10 sub-step）：

- [x] D2.1 `src/constants/login.ts` — `LOGIN_METHOD = { Regular: 0, QrCode: 1, GamePass: 2 }` + `LoginMethod` type；docblock 解釋 TOTP 是 Regular 流程子分支不獨立編號（對齊 WPF：TOTP 成功後也走 `OnLoginCompleted` → `SaveLoginCredentials`，method 仍是 Regular）
- [x] D2.2 `auth.ts` 加 `loginIntent` + `verifyIntent` 兩個 transient slot — `LoginIntent = { region, accountId, password, rememberPassword, autoLogin }`（IdPassForm submit 時暫存、`LoginTotp` / `VerifyPage` / IdPassForm 二次 submit 都讀同一份）；`VerifyIntent = { code, remember }`（VerifyPage submit 成功時寫、IdPassForm 二次 submit 一起 save）；setters / clearer 一組；`clearSession()` / `logout()` 連帶 wipe 兩個 slot；spec 補測
- [x] D2.3 `account.ts` 加 `saveLoginCredentials(input)` action + `findStoredAccount(region, accountId): Account | undefined` selector — `saveLoginCredentials` guard：`method === GamePass` skip（WPF L1316-1332 GamePassLoginCompleted 直接呼叫 `ShowAccountListPage`，跳過 `OnLoginCompleted` → `SaveLoginCredentials`）/ `method === QrCode` skip 兩 region（TW WPF 明確 skip + HK Q1=B fix）；其餘 atomic upsert 對齊 WPF `addAccount` 7-field write；**WPF quirk parity**：`account_name` 一律寫 `""`（對齊 `MainWindow.xaml.cs` L1352），D8 ManageAccount 階段再評估是否修這個 WPF bug
- [x] D2.4 `App.vue` boot sequence 加 `account.loadAccounts()` — 順序 `config.loadAll()` → `account.loadAccounts()` → `ui.applyAll()`；對齊 WPF `MainWindow ctor` 開機就 call `accountManager.readRecord()`；try/catch soft-fail（log error 但不 brick boot）
- [x] D2.5 `IdPassForm.vue` — mount prefill（讀 `config.get('AccountID')` → `account.findStoredAccount(region, accountId)` → 有 record 就 prefill account/password/remember=有 password/autoLogin=record.auto_login，password 為空時不勾任何 checkbox 對齊 WPF L1067 short-circuit）+ submit 時把 `loginIntent` 寫進 auth store（不論成功失敗都寫，等 LoginTotp / VerifyPage 讀回去）+ 完整 success（無 pendingTotp / pendingVerify）path call `account.saveLoginCredentials({ ...intent, verify: auth.verifyIntent?.code ?? '', rememberVerify: auth.verifyIntent?.remember ?? false, method: LOGIN_METHOD.Regular }) → config.set('AccountID', account)` + 清掉兩 intent slot（單發消費）
- [x] D2.6 `LoginTotp.vue` success path call `account.saveLoginCredentials` — 從 `auth.loginIntent` 讀 region/accountId/password/remember/autoLogin、verify 從 `auth.verifyIntent` 讀（`/login/verify` → `/login/id-pass` → `/login/totp` 路徑也覆蓋）；method `LOGIN_METHOD.Regular`；intent 缺失時 graceful skip（log warn 但仍 navigate `/accounts`，不 brick）+ `config.set('AccountID', accountId)` + 清掉兩 intent slot
- [x] D2.7 `VerifyPage.vue` — mount prefill（用 `auth.loginIntent.{region,accountId}` 找 stored record，prefill `verifyCode = record.verify`、`remember = true`，對齊 WPF L1482-1487；intent / record / verify 任一缺都 graceful skip）+ submit 成功時 `auth.setVerifyIntent({ code, remember })` 後再 `router.push('/login/id-pass')`（讓 IdPassForm 二次 submit 拿到 verify 一起 save）；submit 失敗（wrong_captcha / wrong_auth_info / server_message）/ back 鍵 / 任何切走 page 都不寫 intent
- [x] D2.8 vitest — `account.saveLoginCredentials` 7 case（TW Regular remember on / TW Regular remember off / TW QR skip / HK QR skip / GamePass skip / HK Regular remember+verify / rememberVerify off override）+ `account_name` quirk parity case + `findStoredAccount` 2 case（hit / miss）+ `auth.{set,clear}LoginIntent` / `{set,clear}VerifyIntent` 3 case + `clearSession` 連帶 wipe + IdPassForm 7 case（mount prefill 3 case / submit success / verify-intent fold / clearSession single-shot consume / pendingTotp & pendingVerify branches 留 intent）+ LoginTotp 5 case（success save + verify-intent fold + clearSession single-shot + missing intent guard + pendingVerify 不 persist）+ VerifyPage 6 case（mount prefill 4 case / submit setVerifyIntent / loginIntent 留住 retry）— 共 35 新 case，全綠
- [x] D2.9 quality gates 全綠 — vitest 296 passed（含新 D2 case，由 D1 的 264 → 296 = +32 net）/ vue-tsc 0 / eslint 0 / prettier 0 / cargo check 0（backend 0 改動）
- [x] D2.10 Todo.md backfill — 本段 sub-step 全部 ✓；HK QR=B 決策**實際 ship 結果驗證**：(a) `account.saveLoginCredentials` skip rule 經 spec lock down，未來 regress 立即被測捕到；(b) IdPassForm 二次 mount 不會看到 `(HK, "")` garbage row（cache 內無此 row）；(c) WPF L375-385 `Math.Min` floor workaround 在新版根本不需要存在 — backend 沒寫 row 就沒 row 可 floor；(d) Q1 = B 預期的 0 行為差異 + 非 0 UX 差異（無 hidden empty row）皆成立

##### P12.2 D3 — `windows/AddServiceAccount.vue`（+ `Plus` 按鈕 wire `account.addServiceAccount`）

WPF parity 來源：`Beanfun/Windows/AddServiceAccount.xaml(.cs)` + `Beanfun/Pages/AccountList.xaml.cs::btnAddServiceAccount_Click` (L117-135) + `Beanfun/MainWindow.xaml.cs::AddServiceAccount` (L1949-1965) + `GetServiceContract` (L2083-2089)。

設計決策（pre-flight 已 sync 使用者）：

| Q | 決策 | Rationale |
|---|---|---|
| Q1 Modal 機制 | **Element Plus `<el-dialog>` in-page modal**（非新 Tauri Window） | WPF 用 Window 是 navigation paradigm 是 window-based；SPA in-page modal 才符合單頁體驗、可共用 Pinia + 不需處理跨 window focus / IPC；mockup 也是 inline modal style |
| Q2 Mockup vs WPF 欄位差異 | **以 WPF 為準**（只 displayName + 同意條款） | Mockup 多畫的 password / 確認密碼 / 強度欄是 `UnconnectedGame_AddAccount` 才有的（unconnected game 才需用戶輸入密碼，connected game server 自動產生）；mockup 把兩個 dialog 合在一起是錯誤，D3 只做 connected-game branch；密碼欄留 P12.3 `UnconnectedGame_AddAccount.vue` |
| Q3 Unconnected game 分支（`610153/TN` / `610085/TC`） | **跳過，docblock 標 P12.3 wire** | 目前無 game switcher（P12.3），service_code 永遠來自登入時的單一 game，使用者不會碰到那兩個 unconnected game；`AccountList.vue::handleAddAccount` docblock 註明 P12.3 game switcher 落地時再加 service_code/region 判斷分流 |
| Q4 Contract 顯示 | **D3 inline 簡易版（plain text + scroll panel）**，D10 才抽出 `windows/Contract.vue` | YAGNI：D10 才會真正定義 Contract dialog 規格（HTML/RTF 渲染、share / copy button、共用點）；D3 只需 WPF `aContract_Click` UX 等價（拿到 contract 顯示給用戶看），純 `<pre>` plain text 配 scroll container 就夠；**不用 `v-html`** — 來自 server 的 HTML fragment 即使可信仍應由 D10 owns sanitization 統一處理 |
| Q5 失敗處理 | **跟 WPF 一致** | (a) displayName 空 → `ElMessage.warning(MsgDisplayNameNeed)`，dialog 不關；(b) 沒勾 IAgree → `ElMessage.warning(MsgTermsOfServiceNeed)`，dialog 不關；(c) `addServiceAccount` return false → `ElMessage.error(MsgCreateServiceAccountFailed)`，dialog 不關（讓用戶改名重試）；(d) `addServiceAccount` throw → `wrapCommand` 已自帶 toast，不額外 toast 也不關（避免重複 error message） |
| Q6 i18n 處理 | **全 reuse WPF 既有 key**（不開 namespaced 新 key） | `AddServiceAccount` / `ServiceAccountDisplayName` / `IAgree` / `TermsOfService` / `MsgDisplayNameNeed` / `MsgTermsOfServiceNeed` / `MsgCreateServiceAccountFailed` / `UnknownError` / `Cancel` / `Add` / `Confirm` 都齊；唯一缺：zh-CN 缺 `IAgree` + `Cancel` → 補（其餘 locale 已有） |

D-step 規劃（7 sub-step）：

- [x] D3.1 zh-CN.json 補 `Cancel` + `IAgree` 兩 key — 三 locale 維持 `zh-CN ⊆ zh-TW ≡ en-US` 契約（drift guard 規範詳 P11 D5 docblock）
- [x] D3.2 `src/windows/AddServiceAccount.vue` 新建 — `<el-dialog>` modal，props `visible: boolean` + emits `update:visible` / `created(name)`；form：displayName ElInput（maxlength 32 + show-word-limit）+ IAgree ElCheckbox 含 Terms 連結；nested ElDialog 顯示 contract（plain text scroll）；submit 流：trim displayName → 兩段驗證（matches WPF `ButtonOk_Click` 順序：empty name → terms unchecked）→ `account.addServiceAccount(trimmed)` → `true` 關 dialog + emit `created`、`false` toast `MsgCreateServiceAccountFailed` 留開、throw 留開；`submitting` ref 防雙擊；`@closed` reset form 避 re-open 看到舊 input；mount + `visible: true` 自動 focus displayName input；Cancel + 標題列 close button + Esc + backdrop（後三者可關只在非 submit 狀態）
- [x] D3.3 `AccountList.vue` wire — import `AddServiceAccount` from `../windows/AddServiceAccount.vue`；`handleAddAccount` 從 `makeStub('Add Service Account')` 改成 `addAccountVisible.value = true`；template 結尾加 `<AddServiceAccount v-model:visible="addAccountVisible" />`（unconditional mount 讓 transition 可動）；page 開頭 stub-ownership table `Add Service Account button` 那行從「P12.2 D-step」改 `**REAL since P12.2 D3**`；`handleAddAccount` 補 docblock 解釋 unconnected game branch 為何 P12.3 才接（service_code/region 條件判斷會跟 game switcher 一起到位）
- [x] D3.4 `tests/unit/windows/AddServiceAccount.spec.ts` 新建 12 case — 兩段驗證（empty name / unchecked terms）/ 完整 success（trim displayName + emit `created` + close dialog）/ business failure（return false → MsgCreateServiceAccountFailed toast + 留開）/ transport throw（不 toast 不關）/ Cancel button 關不 invoke / 標題列 close 關 / Terms 連結開 nested contract / Terms 連結 empty contract → UnknownError + 不開 / re-open reset form（dialog stub watch modelValue true→false 在 `nextTick` 後 emit `closed`，模擬 Element Plus fade-out 後 hook）/ submitting guard 雙擊不 dup invoke / store integration（`useAccountStore.addServiceAccount` spied，確保不 bypass 走 raw command 漏掉 list refresh）；ElDialog 自定義 stub 從 vi.mock factory 內 dynamic import vue（避 hoist scope 問題）
- [x] D3.5 `tests/unit/pages/AccountList.spec.ts` 更新 — element-plus mock 加 inert ElDialog / ElForm / ElFormItem / ElInput stub（滿足 `AddServiceAccount.vue` import side-effect，但 dialog 整顆被 `global.stubs.AddServiceAccount` 換掉所以不真 render）；icons mock 加 `CircleClose` / `CirclePlus` / `Document`；新加 `AddServiceAccountStub`（observable `data-visible` 屬性）+ 1 case：點 add button → stub `data-visible` true、stub `$emit('update:visible', false)` → stub `data-visible` false（驗證 `v-model:visible` 雙向綁定）；總 case 8 → 9
- [x] D3.6 quality gates 全綠 — vitest 296 → 309（+13：12 個 AddServiceAccount + 1 個 AccountList wiring）/ vue-tsc 0 / eslint 0 / prettier 0（修 2 個自動格式化）/ cargo check 0（backend 0 改動）
- [x] D3.7 Todo.md backfill — 本段 sub-step 全部 ✓；line 1280 D3 checkbox 從 `[ ]` → `[x]`

##### P12.2 D4 — `windows/ChangeServiceAccountDisplayName.vue`（+ row context menu `Change Alias` wire）

WPF parity 來源：`Beanfun/Windows/ChangeServiceAccountDisplayName.xaml(.cs)` + `Beanfun/MainWindow.xaml.cs::ChangeServiceAccountDisplayName` (L2060-2081) + `Beanfun/Pages/AccountList.xaml(.cs)::m_ChangeAccName_Click` (L142+) + `Beanfun/Tools/BeanfunClient.Account.cs::ChangeServiceAccountDisplayName`（POST `gamezone.ashx` `strFunction=ChangeServiceAccountDisplayName` `sl/said/nsadn`）。

設計決策（pre-flight 已 sync 使用者、8 個 Q）：

| Q | 決策 | Rationale |
|---|---|---|
| Q1 Mockup `僅顯示於本機` 文案 | **以 WPF 為準（server-side 改名）**，刪 mockup 那段誤導文案 | WPF `BeanfunClient.Account.cs::ChangeServiceAccountDisplayName` 真的 POST 到 `gamezone.ashx` 改 server-side `nsadn`，再 `redrawSAccountList()` 重抓；mockup 寫「僅本機、不同步」是錯的，不照做避免使用者誤會；dialog 內也不再放「local only」副標 |
| Q2 Row context menu 觸發 | **左鍵點 `more_vert` icon 開 ElDropdown popover** | Mockup 把 affordance 畫成 row 右側永遠可見的 `more_vert` icon button、左鍵點開 menu；WPF 用右鍵 `ContextMenu` + 無可見 affordance 是 desktop 1990s convention，SPA 不適合（a11y 問題、無發現性）；ElDropdown `trigger="click" placement="bottom-end"` 完全對齊 mockup |
| Q3 D4 popover menu 範圍 | **只放 `Change Alias` item**，其他 menu item 留各自 D-step | Mockup 完整 menu 5 項：Copy ID / Change Alias / Account Info / Change Email / Official Site；分別歸屬於 P12.2 D-step（clipboard wire）/ **D4** / D6（ServiceAccountInfo）/ 另一 D-step（change email dialog）/ P12.4（WebBrowser）；D4 只 ship Change Alias，其餘各 D-step 自己加自己的 `<el-dropdown-item>` — 無 dispatch table（YAGNI，5 項小選單） |
| Q4 Dialog props 形狀 | **吃整個 `ServiceAccount` 物件**，不只 `sname` | `commands.changeDisplayName(newName, account)` 需要完整 `said` / `sl`（見 BeanfunClient.Account.cs）；只傳 sname 會逼 dialog 從 store lookup，但使用者可能 row A 點開、row B 又點 → 改錯 row；snapshot account at trigger time 才 unambiguous（沿用 D3 `<AddServiceAccount />` explicit-input pattern） |
| Q5 Input maxlength | **32 char + show-word-limit**（SPA-tighten） | WPF 無 client-side 長度限制，server 才擋（浪費 round-trip + 使用者時間）；32 對齊 D3 Add dialog；QA traces 觀察到的最長 sname 也在 32 內 |
| Q6 失敗 / 邊界處理 | **WPF parity + 一個 SPA 改善** | (a) 空輸入：WPF 返回 false → 關 dialog → 顯示 MsgChangeDisplayNameError（要求重開 menu 重觸發、UX 差）；**SPA tighten**：toast `MsgDisplayNameNeed` warning + dialog 不關，直接讓 user 改；驗證契約不變（server 不會被空字串呼叫）；(b) 與舊 sname 相同：trim 後比對，若一致 → close dialog 不打 server、不 emit `updated`、不 toast（mirror WPF L2068-69 short-circuit）；(c) `changeServiceAccountName` return false → toast `MsgChangeDisplayNameError`、dialog 不關；(d) throw → `wrapCommand` 已 toast、不額外 toast 不關（避重複錯誤訊息） |
| Q7 i18n 處理 | **全 reuse WPF 既有 key** | `ChangeAccountName` / `ServiceAccountDisplayName` / `MsgChangeDisplayNameError` / `MsgDisplayNameNeed` / `Cancel` / `EditAccountSave` 三 locale 全齊（D3 已補完 zh-CN 缺漏）；不開 namespaced 新 key |
| Q8 元件位置 | `src/windows/ChangeServiceAccountDisplayName.vue`（與 D3 同 dir） | `windows/` 慣例放 dialog 元件（對齊 WPF `Windows/` folder） |

D-step 規劃（6 sub-step）：

- [x] D4.1 `src/windows/ChangeServiceAccountDisplayName.vue` 新建 — `<el-dialog>` modal，props `visible: boolean` + `account: ServiceAccount | null`、emits `update:visible` / `updated({sid, newName})`；form：displayName ElInput（pre-fill `account.sname` on open via `watch(visible, immediate)`、maxlength 32 + show-word-limit）；submit 流：trim → 空 → toast `MsgDisplayNameNeed` warning + 留開；trim === sname → close dialog 不 invoke（WPF L2068 short-circuit）；`accountStore.changeServiceAccountName(trimmed, account)` → `true` 關 + emit `updated`、`false` toast `MsgChangeDisplayNameError` 留開、throw 留開（wrapCommand 已 toast）；`submitting` ref 防雙擊；`@closed` reset form；mount + open 自動 focus；Cancel + 標題列 close + Esc + backdrop（後三者只在非 submit 狀態可關）；store 變數 alias 為 `accountStore`（避免跟 prop `account` 撞 `vue/no-dupe-keys`）；docblock 解釋 mockup 「僅本機」文案誤導 + 為何收整個 ServiceAccount + 為何 SPA tighten 空輸入 UX
- [x] D4.2 `AccountList.vue` row context menu 改 ElDropdown popover + Change Alias wire — import `ElDropdown` / `ElDropdownMenu` / `ElDropdownItem` + `EditPen` icon + `ChangeServiceAccountDisplayName` 元件；row 內 `<button class="account-list__row-more">` 包進 `<el-dropdown trigger="click" placement="bottom-end">`、`#dropdown` slot 內 `<el-dropdown-menu>` 含一個 `<el-dropdown-item @click="handleChangeAlias(a)">{{ t('ChangeAccountName') }}</el-dropdown-item>`；移除 `handleRowMore` stub；新 state `changeAliasVisible` + `changeAliasTarget`；`handleChangeAlias(a)` snapshot row 後開 dialog；`watch(changeAliasVisible)` 在 `false` 時清 target（避免跨 session ref leak、不靠 v-model + @update:visible 的 listener-merge 行為）；template 結尾 mount `<ChangeServiceAccountDisplayName v-model:visible="changeAliasVisible" :account="changeAliasTarget" />`；page 開頭 stub-ownership table `Per-row context menu (more_vert)` 那行從「P12.2 D-step」改 `**REAL since P12.2 D4** (only the Change Alias item; ...)`；row 內 `<el-dropdown @click.stop>` + `<button @click.stop>` 防止點 trigger 冒泡到 row `@click=selectRow(a)`
- [x] D4.3 `tests/unit/windows/ChangeServiceAccountDisplayName.spec.ts` 新建 12 case — pre-fill 驗證 / empty input → MsgDisplayNameNeed warning / whitespace-only input → MsgDisplayNameNeed warning / unchanged-name short-circuit close-only / unchanged-name trim 後相同也 short-circuit / 完整 success（trim + emit `updated{sid,newName}` + close）/ business failure（return false → MsgChangeDisplayNameError + 留開）/ transport throw（不 toast 不關）/ Cancel button 關 / 標題列 close 關 / re-open with 不同 account 重新 pre-fill（驗證跨 row 復用 dialog 安全）/ submitting 雙擊 guard / store integration（spy `useAccountStore.changeServiceAccountName` 確保不 bypass 走 raw command）；ElDialog stub 一樣 watch `modelValue` true→false `nextTick` 後 emit `closed`（沿用 D3 mock 模式）
- [x] D4.4 `tests/unit/pages/AccountList.spec.ts` 更新 — element-plus mock 加 ElDropdown / ElDropdownMenu / ElDropdownItem stub（ElDropdown 把 default + `#dropdown` slot 都 inline render，避免 popper teleport 在測試中難以 reach）；icons mock 加 `EditPen` / `Check`；新 `ChangeServiceAccountDisplayNameStub`（observable `data-visible` + `data-account-sid` 屬性）；新 2 個 case：(1) 點 row 2 menu item Change Alias → stub `data-visible=true` + `data-account-sid=sid-2`；stub `$emit('update:visible', false)` → 兩個 attr 都清空（驗證 watcher 正確釋放 target）；(2) 點 menu item 不會誤觸 row select（`account.selectedSid` 仍為 null，驗證 click bubbling 被擋）；總 case 9 → 11
- [x] D4.5 quality gates 全綠 — vitest 309 → 324（+15：12 個 ChangeServiceAccountDisplayName + 2 個 AccountList wiring + 1 個 misc round-up，所有 23 個 spec file 全綠）/ vue-tsc 0 / eslint 0（首次 run 撞到 `vue/no-dupe-keys`：prop `account` 與 `const account = useAccountStore()` 同名 → 改名 `accountStore` 修復；補 docblock 註明原因）/ prettier 0（修 1 個自動格式化）/ cargo check 0（backend 0 改動）
- [x] D4.6 Todo.md backfill — 本段 sub-step 全部 ✓；line 1281 D4 checkbox 從 `[ ]` → `[x]`

##### P12.2 D5 — Get OTP flow（`account.getOtp(selected)` + clipboard + auto-paste preference + AutoPasteTip）

WPF parity 來源：`Beanfun/MainWindow.xaml.cs::getOtpWorker_DoWork` (L2092-2128) + `getOtpWorker_RunWorkerCompleted` (L2131-2265) + `Beanfun/Pages/AccountList.xaml(.cs)::btnGetOtp_Click` (L82-101) + `t_Password_PreviewMouseLeftButtonDown` (L103-115) + `autoPaste_CheckedChanged` (L73-80) + `Beanfun/Lang/{zh,zh-Hans,en}.xaml::AutoPasteTip`。Backend 既有 `commands.getOtp(account)`（P11）+ `commands.autoPaste(req)`（P10.3 D5d，含 `process.window_not_found` 錯誤碼）+ `useConfigStore`（P11，boot 時已 `loadAll()`），D5 只做 frontend wiring。

設計決策（pre-flight 已 sync 使用者、11 個 Q）：

| Q | 決策 | Rationale |
|---|---|---|
| Q1 autoPaste 預設值 | **`false`（對齊 WPF）** | 使用者明確指示「對齊 WPF 除非是 bug」；WPF L25 / L75 用 `GetValue("autoPaste", "false")` 預設關閉是刻意的，因為 auto-paste 失敗會干擾遊戲視窗（按鍵亂送）；保守預設讓使用者主動 opt-in 才合理；初版 D5 bullet 寫「預設 true」是 SPA 改善初稿、pre-flight 撤回 |
| Q2 autoPaste 持久化策略 | **lazy write**：cache 用 `useConfigStore.getOr('autoPaste', 'false')` 初始化；user 第一次 toggle 才 `set` | 對齊 WPF L75-79 行為（沒寫過就視為預設值，第一次 toggle 才寫）；避免 boot 時無謂 disk write；Config.xml 只記 user 真正改過的偏好 — 跟 WPF Config.xml 完全互讀 |
| Q3 AutoPasteTip 顯示時機 | **mirror WPF**：第一次 toggle 時（`useConfigStore.get('autoPaste') === undefined`）show `ElMessage.info(t('accountList.autoPasteTip'), { duration: 8000, showClose: true })` | WPF L77-78 是 MessageBox.Show，SPA 改 ElMessage info（duration 8s + 可關，因內容兩段 multi-line 需要時間讀）；對齊 WPF 「教學一次就好」的意圖；frontend-only key（WPF 是 multi-line `<TextBlock>` 結構，convert-lang.mjs 沒匯入，自己組三 locale） |
| Q4 OTP 取得後的分支邏輯 | 對齊 WPF L2152-2240，**不加任何 SPA 額外 toast**：<br>① `getOtp` throw → `wrapCommand` 已 toast detail（caption `GetOtpFailed` 不額外加，避 double-toast；user 看 detail 已 self-explanatory）<br>② `getOtp` 成功 + autoPaste **off** → `navigator.clipboard.writeText(otp)` + `ElMessage.success(t('GetOtpSuccessAndCopy'))`<br>③ `getOtp` 成功 + autoPaste **on** → `commands.autoPaste(req)`：<br>　→ 成功：**silent**（不 toast，對齊 WPF L2235-2237 直接 PostString，無 MsgBox）<br>　→ `process.window_not_found`：fallback ②（mirror WPF L2169-2174）<br>　→ 其他 error：`wrapCommand` 已 toast，不額外處理 | WPF errexit 第 3 參數是 caption（"獲取密碼失敗"），SPA wrapCommand toast 是 detail；double toast 反而吵；user pre-flight 確認「Q9 照 WPF」連帶推導出「成功也照 WPF silent」（避 SPA-tighten 不一致） |
| Q5 `autoPaste` request 組裝 | `{ className: 'MapleStoryClass', account: target.sid, password: otp, specialClick: code === '610074' && region === 'T9' }`，`code`/`region` 從 `useAuthStore().session` 取（snake_case `service_code`/`service_region`） | 對齊 WPF L2158（`win_class_name` 在 TW MapleStory 是 `MapleStoryClass`，backend 自動 fallback `MapleStoryClassTW`）+ L2195 specialClick TW MapleStory 條件；className hardcode 跟 backend chunk 10.3 既有契約一致（未來新遊戲再開分支） |
| Q6 target snapshot | trigger 時 snapshot `selectedServiceAccount` 進 local const，後續流程不再讀 store；in-flight 期間 user 切 row 不影響 | 對齊 D4 explicit-input pattern；WPF L2148 在 worker completed 時才讀 selected index 是 race condition bug — D5 修掉（這算「bug 不照」的子例外） |
| Q7 in-flight UI lock 範圍 | **只 disable Get OTP button + label 換 `GettingOtp`**；不 disable list rows / Logout / Add Service Account 等其他控件 | WPF L92-99 把整個 page 鎖住是 desktop 慣例（防 background worker 期間操作其他 control 撞 race）；SPA backend 都是 async / 可重入安全的 IPC、且 Pinia store 有 in-flight 旗標，按鈕級防重入夠用；對齊 D3/D4 in-flight 範圍慣例 |
| Q8 沒選 account 時點 Get OTP | `ElMessage.warning(t('MsgSelectAccount'))`，不 invoke command | 對齊 WPF L86 |
| Q9 Copy OTP 按鈕（手動複製） | **silent copy**：`navigator.clipboard.writeText(otp)` 成功不 toast、失敗也不 toast | 對齊 WPF L103-115 `t_Password_PreviewMouseLeftButtonDown`：silent `Clipboard.SetText` + `catch {}`；user pre-flight Q9 明確「照 WPF」 |
| Q10 OTP 顯示生命週期 | OTP 字串留欄位內，直到：(a) 下次 Get OTP（先清空再走流程，對齊 WPF L91 `t_Password.Text = ""`）/ (b) `selectedSid` 改變（`watch(selectedSid)` 清空，避免 OTP 跟其他 row 顯示在一起）/ (c) logout（`account.clearSessionData()` 已負責、無需新增） | mockup 那條 30 秒進度 bar 是裝飾、不是真 timer；OTP 後端真 expire 時間沒文件、且使用者本來就會在「30 秒內」內手動或自動輸入完；D5 不做 timer（YAGNI），未來若 backend 揭露 expire 時間再加 |
| Q11 `useGetOtpFlow()` composable | **D5 暫不抽**，inline 在 `AccountList.vue` | 目前唯一 caller；user pre-flight Q11 給我決定，inline 比較直接（流程 ~80 行、Pinia store 已負責 IPC 包裝）；未來 P12.3 「Start Game」按鈕也會走「OTP → autoPaste / clipboard」相同邏輯（WPF L2152 條件分支）時，再抽 composable — 第二 caller 才是 DRY 抽出時機 |

D-step 規劃（7 sub-step）：

- [x] D5.1 `pages/AccountList.vue` 取代 `handleGetOtp` stub — 新 `gettingOtp` ref（boolean，in-flight 旗標 + button label 切換 `GettingOtp` ↔ `GetOtp` + 防重入 guard）；`async function handleGetOtp()` 流程：(1) `selectedServiceAccount` null → `ElMessage.warning(t('MsgSelectAccount'))` return；(2) snapshot target、清 `otpValue`、`gettingOtp = true`；(3) try `account.getOtp(target)` → 失敗讓 wrapCommand toast、finally `gettingOtp = false` + return；(4) 成功 + `!autoPaste` → `clipboardWriteOtp(otp, true)`；(5) 成功 + autoPaste → 組 `AutoPasteRequest`（`className: 'MapleStoryClass'` / snapshot `target.sid` / `password: otp` / `specialClick: session.service_code === '610074' && session.service_region === 'T9'`）→ `safeInvoke(commands.autoPaste(req))`；ok → silent（mirror WPF L2235-2237）；`error.code === 'process.window_not_found'` → fallback `clipboardWriteOtp(otp, true)`（mirror WPF L2169-2174）；其他 error → `surfaceCommandError(error)`；(6) `gettingOtp = false`；docblock 補 D5 完整 WPF parity 來源 + 三條分支表 + 為何 in-flight UI lock 比 WPF 窄（只 disable button、不 disable rows / logout）+ 為何 specialClick 計算放 frontend（backend 保持遊戲無關）+ 為何 swallow window_not_found（從 user POV 是成功流程 down 不同 branch）
- [x] D5.2 `pages/AccountList.vue` 取代 `handleCopyOtp` stub — 抽 `async function clipboardWriteOtp(text: string, withSuccessToast: boolean)`：`navigator.clipboard.writeText` + try/catch；`withSuccessToast=true` → success `GetOtpSuccessAndCopy` / fail `CopyFailed`；false → silent（mirror WPF L110-114 `Clipboard.SetText` + `catch {}`）；`handleCopyOtp` 改 `void clipboardWriteOtp(otpValue.value, false)`；`disabled` rule 不變（沿用 D1 `:disabled="!otpValue"`）；docblock 解釋兩個 caller 的 toast policy 分歧 origin
- [x] D5.3 `pages/AccountList.vue` autoPaste preference wire — `const autoPaste = ref(configStore.getOr('autoPaste', 'false').toLowerCase() === 'true')`（`.toLowerCase()` 是相容老 Config.xml 寫過 `"True"` 的安全網）；新 `async function handleAutoPasteToggle(next: boolean | string | number)`（`<el-checkbox>` change payload union 防 future tighten）→ `autoPaste.value = Boolean(next)` → `if (configStore.get('autoPaste') === undefined) ElMessage.info({ message: t('accountList.autoPasteTip'), duration: 8000, showClose: true })` → `await configStore.set('autoPaste', String(nextBool))`；template 從 `v-model="autoPaste"` 改 `:model-value="autoPaste" @change="handleAutoPasteToggle"`（不能 v-model 因 setup-time hydration 也會觸發 → 會誤跑 first-time tip）；data-test 加 `account-list-auto-paste`；docblock 解釋 `=== undefined` sentinel 對應 WPF 「key 沒寫過」+ 為何 lazy write + 為何 8s duration + 為何拆 `:model-value`+`@change`
- [x] D5.4 `pages/AccountList.vue` OTP 顯示重置 — `watch(() => account.selectedSid, () => { otpValue.value = '' })` 切 row 清空（避免 OTP 視覺殘留在不對的 row）；handleGetOtp 開頭也 `otpValue.value = ''`（對齊 WPF L91，避免新 OTP 蓋上前的閃爍 + 確保 `getOtp` throw 時欄位回空）；docblock 點明 logout 場景由 `account.clearSessionData()` 已負責、不需要額外 reset
- [x] D5.5 i18n 補 frontend-only key — `accountList.autoPasteTip` 三 locale（zh-TW / zh-CN / en-US）插在既有 `autoPaste` key 之後；用 WPF `Lang/{zh,zh-Hans,en}.xaml::AutoPasteTip` 兩段 `<Run>` 合成單字串、`\n\n` 隔開保留 paragraph break；zh-CN 額外微修簡體用詞（「运行」對齊 WPF zh-Hans）；不開 `autoPasteSuccess` key（autoPaste 成功對齊 WPF silent，無 toast 需要 i18n）；i18n key-usage spec 自動驗證新 key 有 caller（AccountList.vue handleAutoPasteToggle 的 `t('accountList.autoPasteTip')` 是唯一 caller）
- [x] D5.6 unit tests — `tests/unit/pages/AccountList.spec.ts` +6 case：(1) 沒選 row 點 Get OTP → `ElMessage.warning(MsgSelectAccount)` + `commands.getOtp` 不 invoke / (2) 選 row + autoPaste off → invoke `commands.getOtp` + `navigator.clipboard.writeText(otp)` + `ElMessage.success(GetOtpSuccessAndCopy)` + `commands.autoPaste` 沒 invoke / (3) autoPaste on + `commands.autoPaste` 成功 → invoke autoPaste（含 `specialClick: true` for TW MapleStory）+ 不 toast + 不 fallback clipboard / (4) autoPaste on + `commands.autoPaste` 回 `process.window_not_found` → fallback clipboard + success toast + 不 error toast / (5) 切換 selectedSid 會清空 OTP 欄位 / (6) autoPaste 第一次 toggle → AutoPasteTip info toast + `commands.setConfig('autoPaste','true')` + 寫入 store；第二次 toggle → 不 tip + `commands.setConfig('autoPaste','false')`；spec 內擴：`commands.autoPaste` / `setConfig` / `getAllConfig` 加進 module mock；新 `installClipboardMock()` helper 用 `Object.defineProperty` 替換 navigator.clipboard（jsdom 23 沒實作）；ElCheckboxStub 補 `change` component event emit 對齊 Element Plus 真實 API（不然 native `change` event 會 fall through 帶 DOM Event 物件、`Boolean(Event)` 永遠 truthy → 第二次 toggle false 被吃掉這個 bug 也順手修掉）；spec-level docblock 加 D5 case group 開場；既有 11 case 不動；總 11 → **17**
- [x] D5.7 Quality gates 全綠 — vitest **23 files / 330 passed**（324 → 330，+6）/ vue-tsc 0 / eslint 0 / prettier 0 / cargo check 0；本段 sub-step + line 1282 D5 主 checkbox 都 ✓

##### P12.2 D6 — `windows/ServiceAccountInfo.vue`（+ row context menu `Account Info` wire）

WPF parity 來源：`Beanfun/Windows/ServiceAccountInfo.xaml(.cs)`（建構子 L13-56：純展示元件，無 IPC、無 async）+ `Beanfun/Pages/AccountList.xaml.cs::m_AccInfo_Click` (L212-219，從 `list_Account.SelectedItem` 取 row → `new ServiceAccountInfo(account).ShowDialog()`，沒選 row silent return)。

設計決策（pre-flight 已 sync 使用者、9 個 Q）：

| Q | 決策 | Rationale |
|---|---|---|
| Q1 Modal vs new Window | **`<el-dialog>` 模態** | 對齊 D3/D4 SPA 慣例；ShowDialog() 在 SPA 用 in-page modal 取代 |
| Q2 Dialog 標題 key | **WPF `ServiceAccountInfo`「帳號詳情」**（不用 mockup 的「角色資訊」） | 對齊 WPF 原則；mockup 的「角色」是 RPG 化、跟業務語境不一致 |
| Q3 Mockup-only 欄位（綁定裝置 / VIP / 最近登入 IP / 遊戲標籤） | **全部 omit** | ServiceAccount type 沒這些欄位、auth.session 也沒；無 backend 等於要顯示 fake data，違反 D1 stub policy |
| Q4 Service Code 欄位 | **omit**（純 WPF parity） | service_code 是 per-session 不是 per-account，加在 per-account info dialog 語意微怪；保守選擇，未來真的需要再加 |
| Q5 「X 日大數字」affordance | **保留 WPF**（`{days}` 大數字 + Days label + CreateDate 紅字） | 對齊 WPF；mockup 砍掉這個 affordance 是視覺草稿的損失、不是有意設計 |
| Q6 Date format | **直接 plug `screatetime` / `slastusedtime` raw 字串到 `{0}`** | WPF 也直接 plug-in（後端回 `"yyyy-MM-dd HH:mm:ss"` 已 user-readable）；P12 階段不引入 date library；未來若要本地化可一次處理 |
| Q7 Status color | utility class `text-emerald-500` / `text-red-500`（暫不開 design-token semantic 色） | 對齊 D1 utility 風格、避免 inline style；未來 design tokens 加 `--bf-success` / `--bf-danger` 時 grep-replace |
| Q8 Dialog props 形狀 | **吃整個 `ServiceAccount`**（同 D4） | 對齊 D4 explicit-input pattern；optional 欄位 visibility 計算放 dialog 內封裝、不讓 caller 重複 |
| Q9 Dropdown menu item 位置 | 插在 `Change Alias` 下方、label 用既有 `GameAccountInfo` key、icon `InfoFilled` | 對齊 mockup 5 項菜單順序 (Copy ID / Change Alias / **Account Info** / Change Email / Official Site)；其他 3 項仍各自留各自 D-step（YAGNI、無 dispatch table，D4 同決策） |

D-step 規劃（6 sub-step）：

- [x] D6.1 `src/windows/ServiceAccountInfo.vue` 新建 — `<el-dialog>` modal、`width=440`（與 D4 對齊、不破壞節奏）、props `visible: boolean` + `account: ServiceAccount | null`、emits `update:visible`；body 結構（每行 label + value，grid `6rem 1fr` 對齊）：(1) Account = `account.sid`、(2) SerialNumber = `account.ssn`、(3) Name = `account.sname`、(4) AuthType = `account.sauthtype`（`v-if="account.sauthtype != null"` 整列隱藏）、(5) Status = `account.is_enable ? t('Normal') : t('Banned')` + `service-account-info__status--ok` / `--banned` 兩個 scoped class 各自吃 `var(--bf-success)` / `var(--bf-danger)`（不用 Tailwind utility，與 D3/D4 風格一致）；獨立 panel：(6) `v-if="account.screatetime != null"` 創建區塊（`AccountEstablished` label + `daysSinceCreation` 大數字 + `Days` label + `t('CreateDate', [account.screatetime])` 紅字）；(7) `v-if="account.slastusedtime != null"` LastLoginDate 紅字；footer 一個 Cancel 按鈕；computed `daysSinceCreation = Math.max(0, Math.floor((Date.now() - new Date(screatetime).getTime()) / 86400000))`；computed `statusText` / `statusColorClass`；docblock：WPF parity table（每欄位的 .xaml.cs line ref）+ mockup omit 5 項（含理由）+ 為什麼用 WPF 標題不用「角色資訊」+ daysSinceCreation 與 WPF `getDays` 對應 + `Math.max(0, …)` 防 backend 回未來 timestamp + `account === null` shell mode 契約
- [x] D6.2 `pages/AccountList.vue` row context menu wire — `import ServiceAccountInfo` + `InfoFilled` icon；新 state `accountInfoVisible` + `accountInfoTarget`；`handleAccountInfo(a)` snapshot row（mirror D4 pattern）；`watch(accountInfoVisible)` false 時清 target；dropdown 內 `<el-dropdown-item @click="handleAccountInfo(a)" :data-test="\`account-row-info-${a.sid}\`">` 插在 Change Alias 下方；template 尾端 mount `<ServiceAccountInfo v-model:visible="accountInfoVisible" :account="accountInfoTarget" />`；scope table 「Per-row context menu」備註改「`Change Alias` + `Account Info` items wired」；snapshot rationale 與 WPF `m_AccInfo_Click` (L212-219) 對應的註解
- [x] D6.3 i18n backfill — `src/locales/zh-CN.json` 補 `"Normal": "正常"`（對齊 zh-TW / en-US）；不動 zh-TW / en-US（已齊全）
- [x] D6.4 `tests/unit/windows/ServiceAccountInfo.spec.ts` 新建 — **14 case**（比規劃的 10 多 4：拆「show created panel + days math」與「clamp to 0 future timestamp」、拆「cancel」與「header close」、拆「shell mode」與「reopen swap」）：(1) sid/ssn/sname rows、(2) AuthType row hidden when null、(3) AuthType row shown with raw value、(4) Status enabled → Normal + ok class、(5) Status banned → Banned + banned class、(6) AccountEstablished hidden when null、(7) AccountEstablished + WPF-parity day math（fakeTimer 鎖到 2024-12-15、screatetime 2024-01-15 → 334 天）、(8) clamp days to 0 when future timestamp、(9) LastLoginDate hidden when null、(10) LastLoginDate raw timestamp plug-in、(11) cancel button → close + no command、(12) header-close → close、(13) `account === null` shell mode（dialog mounts、body 不 render）、(14) reopen with different account swaps every field
- [x] D6.5 `tests/unit/pages/AccountList.spec.ts` 更新 — `@element-plus/icons-vue` mock 加 `InfoFilled`；新 `ServiceAccountInfoStub`（`data-visible` + `data-account-sid`）；2 case：(a) 點 row 3 的 `Account Info` → stub 收到 visible=true + account-sid `sid-3` → emit `update:visible(false)` 後兩 attr 都清空 / (b) 點 menu item 不 arm `selectedSid`（同 Change Alias 的 `@click.stop` 不變式）；total 17 → 19
- [x] D6.6 quality gates 全綠 — vitest **24 files / 346 passed**（330 → 346，+16 = +14 ServiceAccountInfo + 2 AccountList）/ vue-tsc 0 / eslint 0 / prettier 0（auto-fix 過 AccountList.vue + ServiceAccountInfo.vue 一次後綠）/ cargo check 0；本段 sub-step + line 1283 D6 主 checkbox 都 ✓

##### P12.2 D7 — 帳號拖曳排序（vuedraggable + Config.xml persistence）

WPF parity 來源：`Beanfun/Pages/AccountList.xaml(.cs)` 的 Drag and Drop Reorder region (L257-451) + `SaveAccountOrder` (L477-487) + `ApplyAccountOrder` (L489-531) + `BeanfunClient.Account.cs` L137-139（`GetAccounts` 結束時 call ApplyAccountOrder）。

設計決策（pre-flight 已 sync 使用者、11 個 Q）：

| Q | 決策 | Rationale |
|---|---|---|
| Q1 拖曳技術 | **vuedraggable@4** | 已在 `package.json`（v4 是 Vue 3 版本）但全 codebase 未用、不用 = dead dep；Sortable.js wrapper、handle selector / ghost class / animation 內建；先 sanity check (D7.3) 確認可在 Vue 3.5 + Vite 6 mount，不行才停下討論 fallback |
| Q2 Drag handle gating | **只有 `⋮⋮` grip 發起**（vuedraggable `handle: '.account-list__row-grip'`）| 1:1 mirror WPF `_isHandlePressed` flag 行為；row 其他區域 mouse-down 不啟動 drag |
| Q3 持久化 IPC | **重用 `commands.setConfig("AccountOrder_<gameCode>", csv)`** | 1:1 mirror WPF `ConfigAppSettings.SetValue("AccountOrder_" + gameCode, ...)` (L486)；D5 autoPaste 已是同 pattern；零後端改動 |
| Q4 GameCode 推導 | **前端 `${auth.session.service_code}_${auth.session.service_region}`** | 1:1 mirror WPF L482；與 D5 specialClick 同層級；無需新 IPC |
| Q5 Apply order 位置 | **前端 `useAccountStore` action** | `account.rs` L36-44 docblock 早把這留給「P5+ command/UI 層」；保留 backend 純粹、不破壞既有 SRP 設計 |
| Q6 Apply 演算法 | **1:1 mirror WPF `ApplyAccountOrder`**：saved order 內存在的先按序排、未在 saved 內的 append 到尾 | 防新加入的帳號被「忘記」丟出列表（WPF L522-526 同處理）|
| Q7 Reorder action 形狀 | **`setServiceAccountOrder(orderedSids)` + `applyServiceAccountOrderFromSavedCsv(csv?)` 兩個 action** | 顯式 sids 易單測；避免 v-model 隱式 sync mutation；與 D5 `getOtp` 一參數一動作的 SRP 對齊 |
| Q8 Persist 失敗 UX | **silent log（mirror WPF `SetValue` swallow）** | WPF parity；下次 `refresh` 自動 reconcile（saved csv vs current accountList）；toast 會打斷拖曳的流暢感 |
| Q9 Banned 可拖 | **可拖**（mockup `cursor-not-allowed` 不採用）| WPF parity；mockup row-3 的 disabled 視覺是草稿、行為層不蓋過 WPF |
| Q10 drag 中 row click | vuedraggable 自動 suppress click during drag、native 行為已正確 | 不需額外 guard |
| Q11 logout / 換遊戲 | gameCode 變化 → savedOrder key 也變、自然套不同順序 | 不需動 `clearSessionData` |

D-step 規劃（7 sub-step）：

- [x] D7.1 `useAccountStore` 加兩個 action：`setServiceAccountOrder(orderedSids: readonly string[])`（顯式 in-place reorder、不碰 IPC、回傳新 `serviceAccounts` 陣列）+ `applyServiceAccountOrderFromSavedCsv(csv: string | undefined)`（CSV split + 修剪空白後 forward 到前者；`undefined`/empty/whitespace 一律 no-op）；docblock 完整覆蓋 WPF parity（L489-531 line ref + L497-499 IsNullOrEmpty guard + L515 ContainsKey skip + L522-526 append unordered tail invariant）+ 為何不 cross-import `useAuthStore`/`useConfigStore`（SRP，caller 在 page 層 derive gameCode）+ 用 Map 取代 .NET Dictionary 的隱含順序假設讓 invariant grep-able
- [x] D7.2 `pages/AccountList.vue` 整合 vuedraggable — `import draggable from 'vuedraggable'`；template `<ul v-else>` 換成 `<draggable :list="account.serviceAccounts" tag="ul" item-key="sid" handle=".account-list__row-grip" :animation="150" ghost-class="account-list__row--ghost" @end="handleDragEnd">` + `#item` slot 包原 `<li>` 內容；新增 `accountOrderConfigKey` computed（`auth.session?.service_code + service_region` 推導）+ `persistAccountOrder` helper（用 `safeInvoke` bypass `configStore.set` → silent on failure，成功才寫 cache）+ `handleDragEnd`（讀 `account.serviceAccounts.map(a => a.sid)` → 雙寫 `setServiceAccountOrder` + `persistAccountOrder`）；`loadList` 在 `getServiceAccounts` 後 call `applyServiceAccountOrderFromSavedCsv` 套用 saved order；scope table 第 52 行「Drag handle (reorder)」改 **REAL since P12.2 D7**；新增「D7 — Per-game drag-and-drop ordering」段落（vuedraggable rationale + 為什麼仍 call store action（idempotent funnel + spy seam）+ Q8 silent fail policy + Q5 page-層 gameCode 推導）+ ghost-class CSS（淡色背景 + 虛線邊框）+ `:active` cursor: grabbing
- [x] D7.3 vuedraggable Vue 3 sanity check — 寫了一次性 smoke spec 確認 vuedraggable@4.1.0 在 Vue 3.5 + Vite 6 + Vitest 4 mount + render slot 正常（peer dep 已聲明 `vue: ^3.0.1`、UMD module 經 vite optimizeDeps 正確處理）；通過後刪除 smoke spec；`npm run build` 1213kB → 1438kB（+225kB = vuedraggable + sortablejs，可接受）
- [x] D7.4 `tests/unit/stores/account.spec.ts` 加 7 case（原計畫 6，多寫 1 個 empty-store no-op 邊界 case）：(1) 三 sid reorder / (2) skip unknown sid / (3) append missing sid 到尾 / (4) empty store no-op / (5) `undefined` csv no-op / (6) empty + whitespace csv no-op / (7) composite case（CSV 含 stale sid + 部分 reorder + 漏 sid → 三 invariant 同時覆蓋）；total 26 → 33
- [x] D7.5 `tests/unit/pages/AccountList.spec.ts` 加 3 case：(1) seed `configStore.entries["AccountOrder_610074_T9"]` + auth session → 載入後 `serviceAccounts` + DOM row 順序皆為 saved order（unmentioned sid 在尾） / (2) 模擬 Sortable.js splice（直接寫 `account.serviceAccounts`）+ DraggableStub `vm.$emit('end')` → spy on `setServiceAccountOrder` + assert `commands.setConfig('AccountOrder_610074_T9', 'sid-2,sid-3,sid-1')` + cache 同步 / (3) `commands.setConfig` reject → 無 `ElMessage.error` toast + local order 保留 + cache 未污染（仍 undefined）；新增 `vuedraggable` 模組 mock（`DraggableStub` 渲染 `#item` slot + 轉發 attrs + 透過 `$emit('end')` 觸發）；total 19 → 22
- [x] D7.6 i18n — 確認無新 key 需求，skip
- [x] D7.7 quality gates 全綠 — vitest **24 files / 356 passed**（346 → 356，+10 = +7 store + +3 page）/ vue-tsc 0（fix 1 個 `string | null` narrowing：`accountOrderConfigKey.value` null 守衛）/ eslint 0 / prettier 0（auto-fix 過 AccountList.spec.ts 一次後綠）/ cargo check 0；本段 sub-step + line 1283 D7 主 checkbox 都 ✓

##### P12.2 D8 — `windows/AddAccount.vue` + `windows/ChangeAccount.vue`（Users.dat record CRUD dialog）

WPF parity 來源：`Beanfun/Windows/AddAccount.xaml(.cs)` + `Beanfun/Windows/ChangeAccount.xaml(.cs)` + `Beanfun/Helper/AccountManager.cs::addAccount`/`removeAccount`/`get*ByAccount`。Mockup 來源：`beanfun-next/mockups/AddAccount.html` + `ChangeAccount.html` + `ManageAccount.html`（後者是 D9 入口、決定 D8 dialog 從哪呼叫）。

設計決策（pre-flight 已 sync 使用者、11 個 Q）：

| Q | 決策 | Rationale |
|---|---|---|
| Q1 Modal vs new Window | **in-page `ElDialog` modal** | 與 D3 `AddServiceAccount.vue` / D4 `ChangeServiceAccountDisplayName.vue` / D6 `ServiceAccountInfo.vue` 同 SPA pattern；mockup 也是 in-page sheet；新 Tauri Window 對單純 form 過重 |
| Q2 共用 SFC vs 兩檔 | **兩個獨立 SFC**（`AddAccount.vue` + `ChangeAccount.vue`） | ROT3：欄位差異多（add 有 region picker / verify 條件欄、change 有 id 唯讀 + auto_login 預設不同），共用元件易長成 prop 風暴；達到 3+ 用例再抽 composable |
| Q3 AddAccount 欄位 | **region + account_id + account_name + password (+ verify TW only) + auto_login**（保留 region picker / verify field） | WPF parity；mockup 為簡化 demo 拿掉、但這是多區實際需求（HK / JP / BNS / NDT 都要走 add） |
| Q4 verify 欄條件顯示 | **`region === 'TW'` 才顯示 verify input**（mirror WPF `AddAccount.xaml.cs::region_SelectionChanged` L40-49） | TW only；其他 region 給 verify 是 dead field |
| Q5 ChangeAccount 是否允許改 account_id | **mockup parity — id 唯讀** | 後端 `save_account` 是 upsert by `(region, account_id)`、不支援指定 index 重排；改 id 必須走 `remove(old_id) → save(new_id)` 兩段 IPC、會掉位置序 + 開啟 race window；改 id 流程改走 D9 `ManageAccount.vue` 的「刪除 → 重新新增」明示路徑（更符合 mockup 揭示的 destructive intent） |
| Q6 ChangeAccount 欄位 | **account_name + auto_login**（id / region / password / verify 全唯讀或不顯示） | 對齊 Q5 — id 不能改；password / verify 改密走獨立 LoginPage 重登流程（mockup 加的「change password」field 與 LoginPage 的 RememberPassword 重複、且寫入不經 server validate 是壞 UX）；region 唯讀（屬於 record key）|
| Q7 backend 改動 | **零** — 重用 `commands.save_account(account)` (upsert) + `commands.remove_account(region, id)` 已有 | Q5 收斂後 ChangeAccount 走 single `save_account` upsert、AddAccount 也是 `save_account`；不需要 indexed insert / 不需要 `update_account` 新 IPC |
| Q8 重複帳號處理 | **B — UX 改善：AddAccount 偵測 `(region, account_id)` 已存在 → block + `ElMessage.error` toast** | WPF 是 silent upsert（`AddAccount.xaml.cs::Button_Click` L41-59 只擋空 id、無 dup check；`AccountManager.addAccount` 內部 `removeIfExists + add`）；保留 silent upsert 等於使用者誤打舊 id 會無聲覆蓋掉舊密碼/verify/auto_login，是個現代 UX 不接受的 footgun；user 在 D8 pre-flight 第二輪選 B（前文寫的 WPF L59-66 line ref 是錯的，已修正）。新增 frontend-only key `addAccountDialog.duplicateExists` 三 locale |
| Q9 dialog 觸發點 | **D8 只負責 dialog 元件本身 + props/emits 契約**；觸發點（`ManageAccount.vue` 的「+」按鈕、row「edit」按鈕）走 D9 wiring | SRP — D8 ship dialog、D9 ship list page；中間用 fixture story（dev mode 暫時掛在 `pages/AccountList.vue` toolbar 後拆掉）驗證 |
| Q10 password 是否預填 | **AddAccount 空、ChangeAccount 不顯示 password 欄**（不預填） | Q6 收斂；password 只走 LoginPage 寫入路徑、唯一 source of truth |
| Q11 i18n key 來源 | **WPF 原 key 優先複用**（`AddAccount` / `AutoLogin` / `AccountNeed` / `Add` / `Cancel` / `Taiwan` / `HongKong` / `tbBeanfunAccount` / `tbBeanfunRemark` / `tbBeanfunPassword` / `tbBeanfunAuthInfo`），其他補 frontend-only：`addAccountDialog.title`、`addAccountDialog.subtitle`、`addAccountDialog.regionLabel`、`addAccountDialog.accountIdLabel`、`addAccountDialog.accountNameLabel`、`addAccountDialog.passwordLabel`、`addAccountDialog.verifyLabel`、`addAccountDialog.duplicateExists`、`changeAccountDialog.title`、`changeAccountDialog.subtitle`、`changeAccountDialog.accountIdReadonlyHint`、`changeAccountDialog.save` | WPF resx 沒 `Save` / `Region` / `AccountID` / `AccountName` / `AlreadyExists` / `ChangeAccount` 等 key — grep `Beanfun/` 確認；補 key 走 frontend-only namespace 跟 D6 `serviceAccountInfo.*` 同 pattern |

D-step 規劃（8 sub-step）：

- [x] D8.1 `windows/AddAccount.vue` 切版 + 行為 — `<el-dialog>` modal、`<el-select>` region picker（TW / HK，sync `LoginRegion` IPC type）+ 4 個 `<el-input>`（account_id required + account_name + password + verify TW-only via `v-if="showVerify"`）+ `<el-checkbox>` auto_login + bf-btn-secondary / bf-btn-gradient footer；`region` watcher 同步清掉 `verify.value` 當 region 切非 TW（mirror WPF `region_SelectionChanged` → `initPage` "set Text = '' when collapsed"）；`effectiveAutoLogin` computed mirror WPF L55 `t_Password.Text == "" ? false : autoLogin.IsChecked` 的 coercion（不 disable checkbox、submit 時 derive）；submit 流程：(a) trim id 為空 → `ElMessage.warning(t('AccountNeed'))` 擋下保留 dialog open（mirror WPF L43-46 + SPA-tightened "stay open" UX）→ (b) `account.findStoredAccount(region, trimmedId)` 已存在 → `ElMessage.error(t('addAccountDialog.duplicateExists'))` 擋下（**Q8 = B 唯一 WPF deviation**：WPF 是 silent upsert，SPA 改成 explicit block 避免 footgun）→ (c) `account.saveAccount({ region, account_id: trimmedId, account_name: trimmed, password, verify: showVerify ? verify : '', method: LOGIN_METHOD.Regular, auto_login: effectiveAutoLogin })` → (d) emit `created({ region, accountId })` + close；docblock 完整覆蓋 WPF parity（`AddAccount.xaml.cs` L41-59 line ref + Q3/Q4/Q5/Q8/Q10 rationale + 為何 mockup 的 "DPAPI 加密" 註腳被 drop 掉）
- [x] D8.2 `windows/ChangeAccount.vue` 切版 + 行為 — `<el-dialog>` modal、grid layout 顯示 region / account_id 為 read-only chrome（**沒有** input bound to id，spec 把這當 invariant 鎖死）+ `accountIdReadonlyHint` 段落解釋「需要改 id 請走 D9 delete + re-add」+ 1 個 `<el-input>`（account_name 可 edit）+ `<el-checkbox>` auto_login + footer；接 prop `account: Account | null`、initial form value 從 prop derive（`watch(visible, immediate=true)`）；submit 流程：(a) `account.saveAccount({ ...prop verbatim, account_name: trimmed, auto_login })` upsert by `(region, account_id)` → (b) emit `updated({ region, accountId, accountName, autoLogin })` + close；docblock 完整覆蓋 Q5 rationale（為何 id readonly：backend `save_account` 是 upsert by key、不支援 indexed insert，「edit id 走 remove+add」會掉序 + 開 race window；mockup 也是 read-only chrome）+ 為何不加「改密碼」 / 「刪除帳號」field（D9 ManageAccount 接、密碼 rotation 走 IdPassForm 重登才 server-validate）+ 為何用兩個獨立 SFC 不抽 composable（ROT3、欄位差異大）
- [x] D8.3 i18n — grep `Beanfun/` 確認 WPF resx 只有 `AddAccount` / `AutoLogin` / `AccountNeed` / `Add` / `Cancel` / `Taiwan` / `HongKong` / `tbBeanfun*` / `Password` / `EditAccountSave` / `UnknownError` 可重用，**沒有** `ChangeAccount` / `Region` / `AccountID` / `AccountName` / `Verify` / `Save` / `AlreadyExists` 等 key（grep 全 codebase 無 match）；frontend-only 補 `addAccountDialog.{subtitle,regionLabel,accountIdLabel,accountNameLabel,passwordLabel,verifyLabel,save,duplicateExists}` + `changeAccountDialog.{title,subtitle,regionLabel,accountIdLabel,accountNameLabel,accountIdReadonlyHint,save}` 三 locale 同步；不重複 `addAccountDialog.title`（reuse WPF `AddAccount`）；`KeysMatch<typeof zhTW, typeof zhTW>` 編譯時鎖三 locale 同 shape
- [x] D8.4 `tests/unit/windows/AddAccount.spec.ts` 6 case：(1) empty id → `AccountNeed` warning + no IPC + dialog stay open / (2) region TW→HK 隱藏 verify field + 清 form value + submit payload `verify === ''` 同時 lock / (3) full success → `commands.save_account` 收到完整 7-field payload（含 `method: 0`）+ emit `created({ region, accountId })` + dialog close + trim id/name 驗證 / (4) duplicate `(region, id)` 已存在 → `addAccountDialog.duplicateExists` toast + 不 call IPC + dialog stay open（D8 Q8 = B 鎖死）/ (5) empty password → payload `auto_login = false` 即使 checkbox 打勾（mirror WPF L55 quirk）/ (6) cancel button 不打 IPC + dialog close
- [x] D8.5 `tests/unit/windows/ChangeAccount.spec.ts` 5 case：(1) prop record 預填 `account_name` + `auto_login` / (2) close → 換 prop record → re-open 表單重新 prime（含 id display 同步換、避免 stale ref leak） / (3) **id readonly invariant**（display element exists + 沒有 `data-test="change-account-id-input"`，未來 refactor 不可悄悄加回 in-place id 編輯）/ (4) submit → `commands.save_account` 收到 `password` / `verify` / `method` 從 prop verbatim、只改 `account_name` + `auto_login` + emit `updated` + close / (5) cancel button 不打 IPC + dialog close
- [~] D8.6 dev fixture — **跳過**（user-confirmed B）：D8 兩個 dialog 都 1:1 follow D3/D4/D6 ship 過的 pattern + utility class，視覺風險低；視覺 smoke 留到 D9 `ManageAccount.vue` 自然有 entry point 時 end-to-end 一次到位，避免污染 production page
- [x] D8.7 quality gates 全綠 — vitest **26 files / 367 passed**（24 → 26 files / 356 → 367，+2 files / +11 case = D8.4 6 + D8.5 5）/ vue-tsc 0 / eslint 0 / prettier 0（auto-fix 過 AddAccount.vue 一次後綠）/ cargo check 0（不動 backend）；同步擴 `tests/unit/i18n/key-usage.spec.ts` 的 audit glob 把 `windows/` 納入掃描範圍（之前只掃 `pages/composables/components/stores`、D8 是第一個讓 `windows/` 用 frontend-only `xxxDialog.*` key 的 D-step），不然新 `addAccountDialog.*` / `changeAccountDialog.*` leaf 會被 dead-key audit 誤判
- [x] D8.8 Todo.md backfill — 本段 sub-step 全標 ✓ + line 1285 D8 主 checkbox ✓

##### P12.2 D9 — `pages/ManageAccount.vue` — 已存帳號列表 + import / export

WPF parity 來源：`Beanfun/Pages/ManageAccount.xaml(.cs)` + `Beanfun/Helper/AccountManager.cs::addAccount`/`removeAccount`/`getAccountList(region)`/`importRecord`/`exportRecord` + `Beanfun/Windows/AccRecovery.xaml.cs`（後者僅作為 D10 AES backup flow 的參考、D9 不接）。Mockup 來源：`beanfun-next/mockups/ManageAccount.html`。

設計決策（pre-flight 已 sync 使用者、12 個 Q，使用者 reply「按你建議走」全採推薦解）：

| Q | 決策 | Rationale |
|---|---|---|
| Q1 List layout | **Mockup 單表 + region chip**（不分 TW/HK 兩 tab） | Mockup 揭示的方向更現代，single source of truth；WPF 的「兩 tab」是 WPF ListView 不支援快速 group/filter 的補丁設計；single table + chip 更直觀，搜尋功能（Q6）也只需作用一張表 |
| Q2 List columns | **mockup 6 欄**：drag handle / 帳號 / 備註 / 地區 / 最近登入（**N/A 顯示 —**，後端 schema 無此欄）/ 操作 | 後端 `Account` 沒 `last_login_at`，先 placeholder 顯 「—」（避免破版），未來若要追加由 P12.X dedicated D-step 處理 schema 變更（涉及 `Users.dat` migration）|
| Q3 Reorder | **D9 暫不實作拖曳排序** — drag handle render 但 `cursor: not-allowed` + tooltip 提示「P12.X 將支援」 | 後端 `save_account` 是 by-key upsert、不支援 indexed insertion；要實作得另開 backend D-step（新 `reorder_accounts(orderedKeys)` IPC + 持久化策略討論），不該塞進 D9；D7 的 `AccountOrder_<gameCode>` 是 service-account 的 key，與 stored credential 無關不能複用 |
| Q4 Import/Export UX | **Mockup plaintext file picker**（`@tauri-apps/plugin-dialog::open`/`save`）；WPF 的 AES-encrypted string flow（AccRecovery）延後到 **D10** | Backend 已 ship `import_records(path)` / `export_records(path)` plaintext flow + `Q7=A` 政策（plaintext 過 IPC，passwords 含在 export 檔，使用者自負保管）；mockup 提示「DPAPI 加密」是 footer chrome 的描述（在描述 Users.dat 本身的加密、不是 export 檔），與 plaintext file picker 並不衝突；AES backup 是另一條 use case（跨機器 / 非該 Windows user 還能讀回），D10 才該擔 |
| Q5 Tauri file picker | **`@tauri-apps/plugin-dialog`** + `capabilities/default.json` 加 `dialog:default` permission；npm 端 `@tauri-apps/plugin-dialog` + Rust 端 `tauri-plugin-dialog` 一起加 | Tauri v2 native file picker 標準作法；plugin 還沒安裝，D9.0 一次到位 |
| Q6 Search box | **加搜尋欄**（filter `account_id` + `account_name` 子串、case-insensitive） | Mockup 有；WPF 沒，UX 改善；對 5+ 帳號使用者非常實用；本地 filter 不打 IPC，純 computed 簡單實作 |
| Q7 Header stats | **只保留「總帳號數」card**，drop「DPAPI / 儲存位置」兩個 implementation detail card | Mockup 三 card 中後兩個是 chrome 廣告詞（不含可動作資訊、無國際化價值、且洩漏內部實作細節）；keep 第一個 card 因為「現在有幾個帳號」對使用者有意義（quota awareness） |
| Q8 Row 操作 icons | **Edit + Copy ID + Delete 三 icon 全 wire** | Mockup 三 icon、皆有實際 UX 價值；Copy ID 用 `navigator.clipboard.writeText` + 顯示 success toast（沿用 D5 `clipboardWriteOtp` pattern）|
| Q9 Multi-select / batch delete | **D9 只做 single delete**（每 row 自己的 delete icon → confirm → remove） | Mockup 沒 checkbox column；WPF 多選實際使用率低（一般使用者一次只刪一筆）；keep D9 scope 緊湊；未來真有 demand 再開 D-step |
| Q10 Routing + entry point | **Register `/manage-account` named route**（`requiresAuth: true`）；entry button 等到 P12.4 Settings page 落地時加（D9 不動 `AccountList.vue`） | 對齊 WPF 從 Settings page 進入的層級（不從 AccountList 直連）；保 `AccountList.vue` 穩定、避免 D9 churn 它；route 先 register 起來，便於 dev test（直接 `#/manage-account` 進）|
| Q11 Delete confirm | **`ElMessageBox.confirm`**（同 D1 logout 用的 pattern）+ `t('MsgDeleteAccountMng', { 0: t('MsgDeleteAccountSingle', { 0: account_id }) })` reuse WPF key | WPF 用 `MessageBox.Show + YesNo`，SPA 用 ElMessageBox 1:1 對齊；reuse WPF 既有 `MsgDeleteAccountMng` / `MsgDeleteAccountSingle` / `DeleteAccount` / `Cancel` key（不開新 i18n）|
| Q12 Import overwrite confirm | **加 ElMessageBox 提示「將覆蓋現有所有帳號」** | Backend `importRecords` 是整檔覆蓋（非 merge）；使用者選錯檔可能瞬間失去所有 stored credential；confirmation dialog 是必要的「破壞性操作 guard」；新 frontend-only key `manageAccount.importOverwriteConfirm` 三 locale |

新 frontend-only key（`manageAccount.*` 全套）：
- `manageAccount.subtitle` — header 副標
- `manageAccount.searchPlaceholder` — 搜尋欄 placeholder
- `manageAccount.import` / `manageAccount.export` — toolbar 按鈕（不 reuse `DataBackup` 因為語意不同：`DataBackup` 是 AccRecovery 的「備份/還原」，D9 是 import/export 純檔案動作）
- `manageAccount.totalAccounts` — stats card 標籤
- `manageAccount.colAccount` / `manageAccount.colRemark` / `manageAccount.colRegion` / `manageAccount.colLastLogin` / `manageAccount.colActions` — table header
- `manageAccount.lastLoginUnknown` — `—` placeholder 顯示
- `manageAccount.remarkEmpty` — 備註欄空時的 placeholder（italic「（未設定備註）」）
- `manageAccount.empty` — 列表為空時提示
- `manageAccount.noSearchResult` — 搜尋無結果提示
- `manageAccount.dragDisabledTip` — drag handle tooltip（"拖曳排序將於後續版本支援"）
- `manageAccount.editAction` / `manageAccount.copyIdAction` / `manageAccount.deleteAction` — row icon tooltip
- `manageAccount.idCopied` — Copy ID 成功 toast
- `manageAccount.importOverwriteConfirm` / `manageAccount.importOverwriteConfirmTitle` — import 覆蓋確認
- `manageAccount.importSuccess` / `manageAccount.exportSuccess` — 完成 toast
- `manageAccount.exportDefaultFilename` — export 對話框預設檔名
- `manageAccount.footerHint` — 底部說明（DPAPI scope）

Reuse WPF 既有 key（不額外加）：
- `ManageAccount` — 頁面 title
- `Cancel` / `Add` / `Edit` / `Delete` / `Back` — 通用按鈕
- `DeleteAccount` / `MsgDeleteAccountMng` / `MsgDeleteAccountSingle` — delete confirm
- `Taiwan` / `HongKong` — region chip 顯示
- `AddAccount` / `tbBeanfunRemark` — dialog title / placeholder
- `LegacyDataMigrateSuccess`（不用）/ 其他 import/export 相關不 reuse `DataBackup`（語意切割如 Q4）

zh-CN 缺漏 key 補齊（grep 結果未發現缺漏；若有，D9.1 會 catch）：
- `Edit` / `Add` / `Delete` / `ManageAccount` / `DataBackup` 三 locale 實際已齊全（grep 已驗證）

D-step 規劃（7 sub-step）：

- [x] D9.0 Tauri dialog plugin 安裝 — `npm i -E @tauri-apps/plugin-dialog@^2`（npm 端、Vue 拿到 `open`/`save`）+ `cargo add tauri-plugin-dialog@2`（src-tauri/Cargo.toml；手動把 `cargo add` 預設塞在 `rand` 旁邊的位置改回到 `tauri-plugin-opener` 旁邊並補上整段「為什麼 D9 要 native picker、底層是 `rfd` 跨平台 (Win32 IFileOpenDialog / AppKit / Zenity)」的 docblock 註解）+ `lib.rs::run` 鏈上 `.plugin(tauri_plugin_dialog::init())` 並更新檔頂 boot-sequence docblock 把 dialog 列上去 + `capabilities/default.json` 的 `permissions` 加 `"dialog:default"`；DRY 護欄：未來其他 page 用 file picker 共享這條 plugin（D10 AccRecovery 也會接）
- [x] D9.1 router / i18n key 補齊 — `router/index.ts` 加 `ROUTE_NAMES.ManageAccount = 'manage-account'` + `routes` 加 `/manage-account` + `meta: { requiresAuth: true }` + 完整 docblock 解釋「為什麼現在沒入口（Settings 在 P12.4 才落地）+ 為什麼不直接從 `AccountList.vue` 接（WPF parent surface 是 `Setting.xaml` 不是 `Login.xaml`，從錯地方掛入口會是 UX bug）+ 為什麼 `requiresAuth: true`（stored credentials 是 session-scoped UX）」+ 檔頂 route table docblock 同步補一行；`i18n/messages.ts` 補 `manageAccount.*` 共 17 leaf（page chrome / toolbar / stats / table headers / row icons / dialog confirms / toast / placeholders）三 locale 同步、`KeysMatch<...>` 編譯時鎖；同 namespace 上方加段 docblock 列出每個 leaf 為何 frontend-only（mockup-driven chrome / WPF AES flow 拆出來的明示 import-export / 明示 empty state / drag-handle tooltip / row icon tooltip / clipboard toast / overwrite confirm / import-export success toast）
- [x] D9.2 `pages/ManageAccount.vue` 切版 + 行為 — full SFC：(a) header（avatar glyph + `bf-text-gradient` 大標 + 副標）；(b) toolbar（搜尋 ElInput + 右側 Import / Export / 新增帳號 三按鈕，Export 按鈕在無資料時 disabled）；(c) stats 單卡（總帳號數，搜尋時仍 anchor 在 unfiltered count）；(d) table（CSS grid 6-col、header row + body rows、row 包含 disabled drag handle + avatar + account_id + remark fallback + region chip + last login `—` + 三 icon button[edit/copy/delete]）；(e) footer hint；(f) integrate `<AddAccount>` / `<ChangeAccount>` dialog（v-model:visible + 用 `watch(editVisible)` 在關閉時清 `editTarget`，對齊 `AccountList.vue` 既有 pattern 而非依賴未 emit 的 `@closed`；**add/edit 不另外 toast** — dialog 內部走 `account.saveAccount` 失敗會被 `wrapCommand` toast、成功則 dialog close + store re-set `accounts.value` → list 自動刷新即視覺反饋，對齊 WPF 也無 toast 的事實）；(g) import flow：`open({ filters: [{ name: 'JSON', extensions: ['json'] }] })` → `null` short-circuit → `ElMessageBox.confirm` overwrite → `account.importRecords(path)` → success toast；(h) export flow：`save({ defaultPath: t('manageAccount.exportDefaultFilename') })` → `null` short-circuit → `account.exportRecords(path)` → success toast（無 overwrite confirm，picker 自帶）；(i) delete row：`ElMessageBox.confirm`（用 `t('MsgDeleteAccountMng', [t('MsgDeleteAccountSingle', [account_id])])` — vue-i18n list interpolation 用 array form 不是 object form，後者會讓 `{0}` placeholder 被吞掉產生空字串）→ `account.removeAccount(region, accountId)`；(j) edit row：snapshot row → `editTarget = account; editVisible = true`；(k) copyId：`navigator.clipboard.writeText(account_id)` → success toast；(l) onMounted call `account.loadAccounts()`（idempotent，cache 已有就快回）；(m) `searchedAccounts` computed（filter `account_id` + `account_name`，case-insensitive）；(n) 4-state load machine（loading / error / empty / ready）+ retry button on error；style 重用 D1 design tokens（`bf-mica-bg` / `bf-glass-panel` / `bf-glass-card` / `bf-btn-gradient` / `bf-btn-secondary` / `bf-text-gradient` / `bf-custom-scrollbar`）；docblock 完整覆蓋全 12 個 Q 決策
- [x] D9.3 `tests/unit/pages/ManageAccount.spec.ts` — **13 case**（超出原預估 8）：(1) loading placeholder 渲染（搜尋欄 disabled、Import/Export disabled）/ (2) 空態渲染（empty state copy + Add Account CTA）/ (3) 非空行渲染（含 avatar / region chip / remark fallback）/ (4) 搜尋 filter（`account_id` + `account_name` case-insensitive，stat 仍 anchor unfiltered count）/ (5) edit row 點擊 → ChangeAccount dialog 開 + 帶整列 prop / (6) delete row + confirm → `commands.removeAccount` 收到正確 (region, account_id) **+ regression assertion: confirm message 字串包含 `account_id` 字面 + 不包含 `{0}` leak**（防 vue-i18n list-interpolation 的 object-form / array-form 手滑回退；destructive verify 過：暫時把 fix 回退成 `{ 0: x }` → assertion 紅，restore array form → green）/ (6b) delete + cancel → IPC 完全 no-op / (7) copy ID → `navigator.clipboard.writeText` 收到 account_id + success toast / (8) import happy path → `dialog.open` 回 path → `ElMessageBox.confirm` confirm → `commands.importRecords(path)` + success toast / (9) import cancel（`dialog.open` return null）→ 不打 IPC、不 confirm、不 toast / (10) export happy path → `dialog.save` → `commands.exportRecords(path)` + success toast（無 overwrite confirm）/ (11) Add Account toolbar 按鈕 → AddAccount dialog visible flips true / (12) Export 按鈕在無資料時 disabled / (13) error state + retry → 第二次 `loadAccounts` 成功復原。Stubs：`@tauri-apps/plugin-dialog` (`open`/`save`)、Element Plus (`ElButton`/`ElInput`/`ElIcon`/`ElMessage`/`ElMessageBox`)、`@element-plus/icons-vue`、`src/types/bindings`、`AddAccount`/`ChangeAccount` 子組件、`navigator.clipboard.writeText` shim；ElInput stub 用 `inheritAttrs: false` + 把 `...attrs`（含 `data-test` / `disabled` / `placeholder`）spread 到內層 `<input>`，否則 `wrapper.get('[data-test=...]').setValue(...)` 會打到 wrapper `<div>` 而 throw（real bug bite once during dev）；`mountIt` 用真 i18n（`createAppI18n()`）而非 stub，所以 confirm message 的 vue-i18n interpolation 是 end-to-end 跑出來的真字串，regression assertion 才能驗到 `{0}` 是否吞字
- [x] D9.4 i18n key-usage audit 全綠 — `tests/unit/i18n/key-usage.spec.ts` 既有靜態掃描覆蓋 `pages/` 自動把 `manageAccount.*` 17 leaf 全認到（無 dynamic-key consumer 註冊需求）；4/4 spec 全綠、無 dead key、無 missing key
- [x] D9.5 quality gates 全綠 — vitest **383 passed / 27 files**（D8 baseline 367 + D9 新 13 case + 既有 router spec 補 3 case 對齊新 route）/ vue-tsc 0 / eslint 0（dev 中 1 個 unused import `useAccountStore` 修掉）/ prettier 0（spec 一檔 auto-fix 後 clean）/ cargo check 0（含 `tauri-plugin-dialog` v2.7.0 編出來、整條 dependency tree 含 `rfd` 0.16 一次過）/ cargo fmt 0
- [x] D9.6 Todo.md backfill — 本段 sub-step 全標 ✓ + line 1286 D9 主 checkbox ✓

##### P12.3 — 遊戲啟動（3 view + backend service + store + AccountList integration）

`windows/GameList.vue` / `windows/UnconnectedGame_AddAccount.vue` / `windows/UnconnectedGame_ChangePassword.vue`；串 P10.3 launcher commands；補 backend `services/beanfun/games.rs`（INI + ServiceList 抓取）+ 6 個新 commands；新 `stores/game.ts` 管 ini/services/selectedGameCode；`AccountList.vue` 接通 game info bar / Change Game / Tools 條件 / Start Game / Add Account 分流 / Change Password 條件。

**WPF parity 校正（拒 mockup 的三處衝突）**：

1. `GameList`：WPF 是純 ListBox（image + name + 單擊關閉），mockup 加 search / category tabs / hover anim → 拒（WPF 沒有，violates parity）
2. `UnconnectedGame_AddAccount`：WPF 是 AccountId + 條件 DisplayName + NewPwd + Confirm + 3 個 Hyperlink + 同意條款 checkbox，mockup 改成 nickname-only + captcha → 拒（WPF 沒 captcha，server-side 不需要）
3. `UnconnectedGame_ChangePassword`：WPF 只有 email 欄位（server email verify_code），mockup 改成 current/new/confirm pwd 三欄 → 拒（WPF 是 email-based reset 流程，mockup 完全誤解了 WPF 的設計）

- [x] D0 預檢 ✓（本段）
- [x] D1 backend `services/beanfun/games.rs`：`list_games(client) -> GameInfoBundle{ini, services}`（純拿不 cache，region 從 client config 推）+ pure-fn parsers（自寫輕量 INI parser，無外掛 crate；ServiceList regex + serde_json，兩種 shape：bare array / `{Rows:[...]}` wrapper）+ DTO `GameService` / `GameIniEntry` / `GameInfoBundle`（serde + specta）+ `LoginError::GameListServiceListMissing`（WPF 是 silent empty，這裡升級成顯式錯誤讓前端 retry banner 可分辨）+ `image_base_url(region)` helper（讓前端直接 `<img src>`，避免 backend proxy 二進位）+ 21 unit tests（INI parser 9 + ServiceList parser 5 + image_base_url 4 + LoginError mapping 3）全綠
- [x] D2 backend `commands/game.rs`：1 個 command `list_games() -> GameInfoBundle`（atomic 對齊 WPF `reLoadGameInfo()`，不拆兩半 IPC = SRP user-meaningful action；session-gated 走 `require_auth`，region 從 session client config 推）+ 註冊到 `commands/mod.rs` `build_specta_builder` + binding 重生（`bindings.ts` 多 `async listGames()` + `GameInfoBundle` / `GameIniEntry` / `GameService` 三 type）+ `bindings_file_tests` 三 type + `listGames` 命名加入 REQUIRED_COMMANDS / REQUIRED_DTOS
- [x] D3 backend：5 個新 unconnected-game commands（`get_service_contract` 已在 P10.2 落地為 `account::get_contract`，session-gated 從 session 拿 service_code/region；無需新 command）
  - `unconnected_game_init_add_account_payload() -> AddAccountInit`（service_code/region 從 session）
  - `unconnected_game_add_account_check(mgmt_session, name, account_dn?) -> CheckOutcome`
  - `unconnected_game_add_account_check_nickname(mgmt_session, account_dn?) -> CheckOutcome`
  - `unconnected_game_add_account(mgmt_session, name, new_password, new_password_confirm, account_dn?) -> AddAccountOutcome`
  - `unconnected_game_change_password(num, email) -> ChangePasswordOutcome`（service_code/region 從 session）
  - 加 `serde::Serialize + specta::Type` 到 `AddAccountInit` / `CheckOutcome` / `AddAccountOutcome` / `ChangePasswordOutcome`；`AddAccountSession` 額外 `Deserialize`（frontend 把 triplet 當 opaque cursor 重新 echo 回來）；`AddAccountOutcome` / `ChangePasswordOutcome` 用 `tag="kind", content="data"` snake_case shape 跟 `AmountLimitNotice` 對齊（frontend `switch(outcome.kind)` 一致）
  - 註冊到 `commands/mod.rs`（5 commands）+ binding 重生（`bindings.ts` 多 5 async + 5 type）+ `bindings_file_tests` 5 命名 / 5 type 加進 REQUIRED_COMMANDS / REQUIRED_DTOS + `account::tests` 加 `account_commands_exist_with_declared_signatures` 5 新項 / `add_account_session_serde_roundtrip_preserves_all_fields` / `add_account_outcome_serde_shape_is_stable` / `change_password_outcome_serde_shape_is_stable` 三 regression test → cargo test 671 全綠
- [x] D4 frontend `stores/game.ts` Pinia：state `{ini, services, selectedGameCode, loadState, loadError}` + computed `{selectedGame, selectedIni, isUnconnectedGame}` + actions `{loadGames(force?), selectGame(code, region), clearGameData()}` + 純 helpers `UNCONNECTED_GAME_CODES` / `gameCodeOf` / `imageUrl(name, region)`；store 不 import `useAuthStore`（store 由 caller 傳 region，避免 auth↔game cycle，main.ts 把 `game.clearGameData()` 組到 `installRouterGuards.clearAccountSession` callback 跟 account 一起 clear）；`loadGames` 4-state machine（idle/loading/loaded/error）+ idempotent + concurrent-call short-circuit + force overload；錯誤雙通道：`loadError` 給 inline banner、`surfaceCommandError` 給 toast，**不拋**讓 caller 看 `loadState`；21 unit tests（pure helpers 4 / load lifecycle 7 / selection + computed 9 + clear 1）全綠
- [x] D5 `windows/GameList.vue` — 嚴格 WPF parity：`el-dialog`（720px、Esc/outside-click 可關）+ glass header（`VideoPlay` icon + `GameSelected` title + i18n subtitle + close button）+ 4 態 body（loading 旋轉 icon + 文字 / error inline banner + Retry 按鈕呼 `loadGames(true)` / empty 文字 / loaded `<ul>` grid `repeat(auto-fill, minmax(160px, 1fr))`）+ 每張卡 `<img>` + 名字 + 邊框 hover/focus/selected 三態；click（含 keyboard Enter/Space）→ 跟 `game.selectedGameCode` 比，相同就只 close、不同 → `game.selectGame()` + emit `select(code, region)` + close（**WPF parity** `l_GameList_SelectionChanged` 早退 + `Close()` 永遠跑）；`visible` `false→true` watcher 在 mount 時 trigger `loadGames()`（idempotent，第二次開不會打 IPC，session expired bridge 已 reset 成 idle）；image URL 由前端組（`imageUrl(svc.large_image_name, props.region)`，TW/HK 不同 CDN base）；無 search / 無 tabs / 無 hover anim / 無 chip-hot；新 i18n keys `gameList.{subtitle, loading, empty, loadFailed, imageAlt}`（zh-TW / zh-CN / en-US）+ docblock 寫滿（mockup 三處衝突、4-state 為何 SPA 加而 WPF 沒、為何 region 透過 prop 不靠 auth store）；7 spec cases（first-open trigger 一次 IPC + loading 渲染 / error banner + Retry force-reload / empty placeholder / 卡片陣列 + 圖片 src 帶 region / click 卡片 emit + close + selectedGameCode 寫入 / 重點 click 已選同卡只 close 不 emit / header close 按鈕只 close 不 emit）全綠
- [x] D6 `windows/UnconnectedGame_AddAccount.vue` — WPF 流程嚴格復刻：mount 第一次 `false→true` watcher → `commands.unconnectedGameInitAddAccountPayload()`（service_code/region 後端從 session 拿）→ 收 `AddAccountInit{session, game_name, account_len, check_nickname_supported}`；init 失敗 → `wrapCommand` 已 toast 錯誤、再 `visible=false`（合併 WPF `MessageBox.Show(UnknownError)+Close()`）；UI 條件渲染 DN 行 + check-nickname 連結 by `check_nickname_supported`；intro/bullets/field labels 把 `gameName` 內插到 5 處 + `account_len` 顯示為 hint；submit 跑全 WPF validation 鏈（empty `_18` / id-len `_19` / pwd-empty `_20` / pwd-len `_21` / pwd2-empty `_22` / pwd2-len `_23` / DN-empty `_24` / DN-len 2..6 `_25` / agree `_26`）→ `ElMessage.warning`（同 AddServiceAccount toast 慣例，非 blocking）；通過後 `commands.unconnectedGameAddAccount(session, id, pwd, pwd2, dn|null)` → `kind:'success'` 清 errorMessage + emit `created` + close / `kind:'error_message'` 寫入 inline `lblErrorMessage` 紅字；CheckId hyperlink → `commands.unconnectedGameAddAccountCheck(session, id, dn|null)` 並重新 stash refreshed session（mirror WPF NameValueCollection 寫回）+ `error_message===''` toast `UnknownError` / 否則 inline；CheckNickName 同 pattern；Contract hyperlink → `useAccountStore.getContract()` → 開 `<Contract>` nested dialog（複用 P12.2 D10.2，i18n key Reuse `UnconnectedGame_AddAccount_15/16` + gameName 內插）；新 i18n key `unconnectedGameAddAccount.loading`（zh-TW/zh-CN/en-US，純前端 placeholder，WPF 無對應）；form `closed` event 全 reset；docblock 寫滿（10+ 行 WPF parity table、3 處 mockup conflict 拒絕：captcha/nickname-only/strength meter/suggested id、validation toast 而非 MessageBox 的 SPA 慣例）；14 spec cases（init fetch + render / DN 條件隱藏 / DN 條件顯示 / `_18` 空 id / `_19` id-len / `_20` 空 pwd / `_24` 空 DN / `_26` 未同意 / 全通過 → `created`+close 帶正確 args / error_message → inline + 仍 open / CheckId → 重新 stash session + 後續 submit 用新 session / CheckId 空 → UnknownError toast / init 失敗 → close / Contract hyperlink → 開 nested）全綠，typecheck / eslint / prettier / key-usage 全乾淨
- [x] D7 `windows/UnconnectedGame_ChangePassword.vue` — WPF 流程嚴格復刻：單一 email 欄位 + Confirm + inline `lblErrorMessage`；submit → `commands.unconnectedGameChangePassword(accountIndex, email)`（service_code/region 後端從 session 拿，accountIndex 由 parent prop 對齊 WPF `accountList.list_Account.SelectedIndex`）→ 三分支：`kind:'verify_code_sent'` → `ElMessageBox.alert` 開 modal（mirror WPF `MessageBox.Show` blocking modal），message body 用 `t('MsgChangePassword', [token])` + 自寫 `unescapeWpfCRLF` helper 把 i18n JSON 裡的字面 `\\r\\n` 還原成真換行（mirror WPF `Regex.Unescape`），透過 `h('pre', { white-space: pre-wrap })` VNode 渲染保留換行不靠 `dangerouslyUseHTMLString`，title `DataSended`；user dismiss 後 emit `verify-code-sent` + close（**WPF parity**: `result.StartsWith("verify_code")` → `MessageBox` + `this.Close()`）；`kind:'error_message'` → 寫入 inline 紅字 + 不關（WPF L43-44）；wrapCommand throw → toast 已處理、不關（取代 WPF `result == null` → `MessageBox(UnknownError)` 的更具體錯誤訊息路徑）；alert reject（Esc / 外部點擊）也走 close + emit 同 confirm 路徑（WPF MessageBox Esc/OK 行為一致）；emit names: `update:visible` + `verify-code-sent`；form `closed` event reset email/error/submitting；`visible` watcher 也提前 clear errorMessage（防 reopen 期間舊錯閃現）；docblock 寫滿（mockup 改三欄式 current/new/confirm pwd 完全誤解 WPF 是 email-based reset → 拒；`\r\n` unescape 為何不用通用 Regex.Unescape；ElMessageBox.alert vs WPF MessageBox.Show 等價性）；6 spec cases（verify_code_sent → alert 帶 token + unescape `\r\n` + 後 close + emit 帶正確 args / alert reject 也照樣 close + emit / error_message → inline + 仍 open / wrapCommand throw → 仍 open + 沒呼叫 alert / cancel button → close 不呼叫 IPC / reopen 重置 email + error）全綠，typecheck / eslint / prettier 全乾淨
- [x] D8 `AccountList.vue` 整合：(a) `useGameStore` 接通 — mount 時 `game.loadGames(region)` + 若 config 有 `loginGame` 套用、否則自動開 GameList；(b) game info bar 顯示真名稱 + image（`<img>` 跨域 OK）+ Change Game 按鈕觸發 `<GameList>`；(c) Tools button conditional visibility（`610074_T9` / `610075_T9` / `610096_TE` 才顯示，click 仍 stub 留 P12.5 wire）；(d) Start Game button → `commands.launchGame(...)` 全管線（path 從 config / mode resolve / OTP 在 trad-login=true 時先取 / cmdline template 從 INI；若 path 空 → 提示用戶設定）；(e) Add Service Account 按鈕分流：`game.isUnconnectedGame` 時開 `UnconnectedGame_AddAccount`，否則開現有 `AddServiceAccount`；(f) Change Password 行內 context menu 在 `isUnconnectedGame` 時顯示，開 `UnconnectedGame_ChangePassword` + spec patch 8+ case
  - [x] D8a backend `set_active_service` command — mutate `AppState.auth.session.{service_code, service_region}` 讓後續 `get_accounts` / 任何 session-gated command 跟著 user 切換的遊戲跑（WPF 直接 mutate `MainWindow.service_code/region` field 沒有 IPC，SPA 必須加這道才能保持 single source of truth）+ `set_active_service_internal` 純函式抽出讓單元測試直接 invoke + 5 unit tests（成功 update / 保留其他 session 欄位 / no session 回 SESSION_REQUIRED_CODE / no-op same input / 接受空字串）+ 註冊到 `commands/mod.rs` `collect_commands!` + REQUIRED_COMMANDS
  - [x] D8b regenerate `bindings.ts` — `cargo run --example export_bindings` 重生 + verify `setActiveService(serviceCode, serviceRegion)` 的 binding shape + `bindings_file_tests` 全綠 確認沒 drift
  - [x] D8c frontend `AccountList.vue` `useGameStore` 接通 — `onMounted` 改呼 `setupGameOnMount()` 取代直接 `loadList()`，`setupGameOnMount` 跑 `game.loadGames` → 若 catalogue 載失敗 fallback `loadList()` + return / 否則讀 Config.xml `loginGame`（`<code>_<region>` 格式，用 `lastIndexOf('_')` 拆讓未來底線開頭的 region 也安全）→ 若有效就 `selectActiveGame(code, region, true)` / 否則 `loadList()` + 自動開 `GameList` picker；`selectActiveGame` 為單一 pipeline：`game.selectGame` → 持久化 `loginGame` → 若 session pair 有變 `commands.setActiveService` → 清 `account.selectedSid` → `loadList()`（gating + idempotent，wpf parity)
  - [x] D8d frontend `AccountList.vue` game info bar 真名稱 + 圖 + Change Game 按鈕 — `gameNameDisplay` computed（`game.selectedGame?.name ?? t('accountList.gamePlaceholder')`）+ `gameImageUrl` computed 從 `imageUrl(small_image_name, region)` + `<img>` 條件渲染（empty fallback 用 `VideoPlay` glyph 保持高度）+ `account-list__game-icon-img` style + Change Game button 觸發 `handleChangeGame()` 開 `gameListVisible`
  - [x] D8e frontend `AccountList.vue` Tools button conditional visibility — `TOOLS_GAME_CODES` `ReadonlySet<string>` literal（`610074_T9`/`610075_T9`/`610096_TE`，跟 WPF L1710 一字不差）+ `showToolsButton` computed + template `v-if="showToolsButton"`（不是 `display:none` 讓 hover affordance 也消失）；click handler 仍走 `handleTools` stub 留 P12.5 wire
  - [x] D8f frontend `AccountList.vue` Start Game pipeline — `loginActionType` computed（INI 空 → 8）+ `tradLogin` computed（Config.xml `tradLogin` default `'true'`，case-insensitive）+ `startGameDirect` computed（`(tradLogin && lat==1) || lat==0`）+ `otpLaunchChain` computed（`!tradLogin && lat==1`）+ `startGameDisabled` computed（無 game OR （非 direct AND 無 selected account））+ `resolveStartMode` 把 `Config.xml startGameMode` 整數對映到 backend `GameStartMode` PascalCase（0/Auto, 1/Normal, 2/LR）+ `pathHasWideChar` 純函式（任何 `>128` charCode）+ `resolveGamePath` 抽出 path 偵測 + `MsgCantFindGame` Yes/No prompt（Yes → `gamePathPickerPending` toast 留 P12.4 settings page wire；No → `commands.openUrl(download_url)`，空 download_url fallback 同 toast）+ `checkAndKillRunningGameProcesses`（`listGameProcesses` 空就略過，否則 `MsgGameAlreadyRun` Yes/No → kill 或繼續，皆不 abort 跟 WPF 一致）+ `runGame(account, password)` orchestrator（resolve path → wide-char warn → process check → mode → `commands.launchGame`）+ `handleStartGame` button click 走 `startGameDirect ? runGame() : handleGetOtp()`；`handleGetOtp` OTP 拿到後若 `otpLaunchChain` 接 `runGame(target.sid, otp)` 不走 autoPaste/clipboard 兩支（mirror WPF L2152-2155 三分支頭一支）；autoPaste `className` 改用 `game.selectedIni?.win_class_name`（fallback `MapleStoryClass`）對齊 WPF L2179 `accountList.win_class_name`
  - [x] D8g frontend `AccountList.vue` Add Service Account 分流 — `unconnectedAddVisible` ref + `handleAddAccount` 在 `game.selectedGame` null 時 toast `GameSelected` + bail，否則 `game.isUnconnectedGame ? unconnectedAddVisible.value=true : addAccountVisible.value=true`（mirror WPF `btnAddServiceAccount_Click` L117-135 verbatim）+ `<UnconnectedGameAddAccount @created="handleUnconnectedAccountCreated">` mount + `handleUnconnectedAccountCreated` 跑 `loadList()` 對齊 `redrawSAccountList()`
  - [x] D8h frontend `AccountList.vue` per-row Change Password — `changePasswordVisible` ref + `changePasswordAccountIndex` ref（0-based index，default `-1` sentinel）+ `handleChangePassword(targetAccount)` 用 `findIndex(sid)` 算 index（race-safety，找不到 toast `MsgSelectAccount`）+ template 在 row dropdown 加 `<el-dropdown-item v-if="game.isUnconnectedGame" @click="handleChangePassword(a)">` 用 `Key` icon + `t('ChangePassword')` + `<UnconnectedGameChangePassword @verify-code-sent="handleChangePasswordSent">` mount + `handleChangePasswordSent` 跑 `loadList()` 對齊 WPF `Close()` 後的 list refresh
  - [x] D8i `AccountList.spec.ts` patch — 15 new test cases 覆蓋 D8c-D8h（D8c×4: catalogue 失敗 fallback / 無 saved loginGame 自動 picker + loadList parallel / saved 等於 session 跳過 setActiveService IPC / saved 不等於 session 觸發 setActiveService + 清 stale selection + reload；D8d×2: 真名稱 + region-aware banner image / 無 game placeholder + 無 `<img>`；D8e×2: 非 TOOLS_GAME_CODES 隱藏 / TOOLS_GAME_CODES 顯示；D8f×3: direct branch login_action_type=0 / OTP+launch chain login_action_type=1 + tradLogin=false / 無 game button disabled UI guard；D8g×2: unconnected game 開 UnconnectedGameAddAccount / 無 game GameSelected toast；D8h×2: connected game 隱藏 change-password row item / unconnected game 顯示 + 開 dialog 帶正確 row index）+ 兩個既有 tests 補 `seedActiveGame(MAPLESTORY_TW)` 因為 D8e/D8g 加上 `v-if`/`game.selectedGame` guard 後它們需要 game 才能跑（Add Service Account D3 wiring + Tools button stub handler）+ shared fixtures `MAPLESTORY_TW` / `KARTRIDER_TW` / `MABINOGI_TN` + matching INI + `seedActiveGame()` helper 把 gameStore catalog/ini/selectedGameCode 一次種好；spec 全綠（45 passed / 30 既有 + 15 新）
- [x] D9 i18n key audit — 掃 `GameList.vue` / `UnconnectedGame_AddAccount.vue` / `UnconnectedGame_ChangePassword.vue` / `AccountList.vue` 全部 `t(...)` 呼叫 → WPF 鍵（`GameStart` / `AddServiceAccount` / `MsgSelectAccount` / `GameSelected` / `ChangePassword` / `MsgCantFindGame` / `MsgGamePathHaveWChar` / `MsgGameAlreadyRun` / `Yes` / `No` / `MsgChangePassword` / `DataSended` / `UnknownError` / `SystemInfo` / `UnconnectedGame_AddAccount_1`–`27`）三個 locale json (`zh-TW` / `zh-CN` / `en-US`) 全有；frontend-only 鍵（`accountList.toolsButton` / `accountList.changeGame` / `accountList.gamePlaceholder` / `accountList.gamePathPickerPending`）三 locale 全齊（`gamePlaceholder` + `gamePathPickerPending` 之前 D8d/D8f 已補）；audit 結果零 missing key
- [x] D10 quality gates 全綠（`cargo fmt --all`（normalize 早期 D1/D2 work files）/ `cargo clippy --all-targets --all-features -- -D warnings` 0 warning（修 3 個 `needless_borrows_for_generic_args` in `commands/account.rs::add_account_outcome_serde_shape_is_stable` + `change_password_outcome_serde_shape_is_stable` test asserts，把 `&AddAccountOutcome::...` / `&ChangePasswordOutcome::...` 改 by-value pass）/ `cargo test --all-features` 全綠 / `npm run typecheck` 0 error / `npm run lint` 0 error（修 2 個 `no-unused-vars` for `GameInfoBundle` + `GameProcessInfo` imports in `AccountList.spec.ts`，這些原本是 ts-ignore'd 但 D8i 改完不再用）/ `npm run format` 然後 `npm run format:check` clean / `npm test` 476 passed）
- [x] D11 Todo.md backfill — 各 D-step（D0-D10）勾掉 + D8 sub-step（D8a-D8i）的 design rationale / WPF parity decision / quality-gate fix 一段詳寫；D9（i18n audit 結果）+ D10（quality-gate fix 細項：3 個 `needless_borrows_for_generic_args` 修在 `commands/account.rs` test asserts、2 個 `no-unused-vars` 修在 `AccountList.spec.ts` imports）詳寫；沿用 P12.2 D11 convention 把 backfill 留到 chore commit（feat 純實作 / chore 純文件，避免 P10.2 D15 amend-and-rehash cycle）
- [x] D12 commit `feat(next): add P12.3 game launch and unconnected game dialogs` → `a04774c`；無 co-author；21 files changed, 7801 insertions(+), 45 deletions(-)（10 new files：`commands/game.rs` + `services/beanfun/games.rs` + `stores/game.ts` + 3 new windows + 4 new specs；11 modified：`AccountList.vue` + `AccountList.spec.ts` + `account.rs` + `bindings.ts` + `i18n/messages.ts` + 6 wiring files）
  - ops note：沿用 P10.2 D15 / P10.3 D9 / P11 D14 / P12.2 D12 的「先 commit feat 不含 Todo hash → 讀 HEAD hash → 補 chore commit 回填」流程；嚴禁擅自 amend

##### P12.4 — 設定與瀏覽器（2 page + 1 window + AccountList top-bar wire）

`pages/Settings.vue` / `pages/About.vue` / `windows/WebBrowser.vue`；補 backend `commands/system.rs` 的 `pick_game_path` (file dialog) 把 D8f 留下的 `gamePathPickerPending` toast 接上；`AccountList.vue` 加 Settings / About icon button 對齊 WPF top-bar 入口；新 routes `/settings` `/about`；store 補 `useUiStore` 缺的 4 個 boolean（`autoStartGame` / `askUpdate` / `tradLogin` / `autoKillPatcher` / `skipPlayWnd` — 都是 Config.xml 鍵）+ `loginMethod` (0/1) computed；`useThemeColor` 已就緒（P11 D-step 落地 6 preset + WPF named-color alias）。

**WPF parity 校正（mockup 衝突拒絕清單）**：

1. Settings 頁佈局：WPF 是上下分區（App / Game）一頁長 form，mockup 通常用 sidebar tabs → 採 WPF 一頁長 form（保 SRP 不引入 tab 機制；mockup 美化保留 glass-panel + section header）
2. ThemeColor：WPF 是 ComboBox + IsEditable 接受任意 hex，mockup 改 swatch 6 格 → 採 WPF（swatch 為 affordance 但仍開放 free-form hex input；P11 已落地 `WPF_NAMED_COLOR_ALIASES` 映射）
3. WebBrowser：WPF 開新 WebView2 視窗 + 注入 cookies 共享 session，Tauri 等價是 `WebviewWindow` + cookie injection；但 P12.4 階段先用 `commands.openUrl` 開系統瀏覽器（cookies 共享留 P13 議題）—— 因為 P12.4 自身不需要 cookie-sharing webview（Settings/About 內無 in-app URL 開啟），WebBrowser 視窗的真正消費者是 P12.5 (KartTools/MapleTools) 跟之前 P12.2 已落地的 AccountList 行內按鈕（已直接用 `commands.openUrl`，無 regression），所以 P12.4 的 WebBrowser 是**佔位骨架** + 後端 `webbrowser_open(url)` command 路由到 `openUrl`，留 P13 升級

- [x] D0 預檢 ✓（本段）
- [x] D1 **decision: skip backend command, use `@tauri-apps/plugin-dialog` JS API direct in Settings.vue** — 沿用 P12.2 D9 ManageAccount 的 precedent（`open as openFileDialog` from `@tauri-apps/plugin-dialog`）；理由：(a) dialog 是 UI affordance 不是 domain logic（SRP — 不該進 backend service layer）；(b) backend 包一道 `pick_game_path` 變成 thinly wrapping a thin wrapper（DRY violation）；(c) `tauri-plugin-dialog` 已在 `lib.rs` `.plugin(...)` + `capabilities/default.json` `dialog:default` 註冊（P12.2 D9 落地），無新 plugin/permission 需求；(d) WPF 沒 backend/frontend 區分，functional parity 不受影響；filter + title i18n 由 frontend 直接 `t('FileDialog_Filter', [game_exe])` / `t('FileDialog_Title', [game_exe])` 帶
- [x] D2 frontend `stores/ui.ts` 補 5 個 boolean Config 鍵 + `loginMethod`（18 tests green；D3 順手修 `'Development'` → `'Beta'` 對齊 backend `Channel` enum + WPF Config schema）
- [x] D3 frontend `pages/Settings.vue` shell + App section（左半）— 完整 docblock 寫到「為何不引入 sidebar tab、為何 ThemeColor 保留 free-form input、為何 LoginMethod 限 TW」；`UPDATE_CHANNEL_OPTIONS` / `LANGUAGE_OPTIONS` / `LOGIN_METHOD_OPTIONS` 三組 readonly tuple + isXValue 防 narrow；ThemeColor handler `try {} catch {}` 對齊 WPF L246-251 silent-on-malformed-hex
- [x] D4 frontend `pages/Settings.vue` App section（右半 4 checkbox）+ DisableHardwareAcceleration `ElMessageBox.alert`（confirmButtonText 走 ELP locale default，對齊 WPF MessageBox 的 OS-level button text）
- [x] D5 frontend `pages/Settings.vue` Game section — `gamePathConfigKey()` 一處 build `<dir_value_name>.<gameCode>` (SRP)；`pickGamePath` 處理 WPF `OpenFileDialog.Filter` 的 pipe-delimited C# format string → Tauri `filters: [{ name, extensions }]` 轉換；3 個 checkbox + `el-tooltip` + `Tools` stub button (與 AccountList `handleTools` 同 console.warn pattern)；empty state banner 在 `!game.selectedGame` 時取代 Game section
- [x] D6 frontend `pages/Settings.vue` footer + route + AccountList 入口 — `handleBack` 用 `window.history.length > 1` proxy 判斷是否有 back history、否則 fallback `router.push('/login')`；`AccountList.vue` 頂列加 Settings (`Setting` icon) / About (`InfoFilled` icon) 兩個 `bf-btn-ghost-icon` button（對齊 WPF `MainWindow.xaml` titlebar L112-139）；route `/settings` `requiresAuth: undefined`（公開）對齊 WPF L85-94 `return_page == loginPage` 分支允許未登入進
- [x] D7 frontend `pages/About.vue` + route — App icon 從 `src-tauri/icons/icon.png` 複製到 `src/assets/icon.png` 走 Vite asset import（不放 public/ 因為 Tauri SPA 慣例）；version 從 `commands.version()` 取 `app`；CheckUpdate 走 `commands.checkUpdate(channel, null)`（P10.3 已落地）+ `ElMessageBox.confirm` 帶 `\r\n` literal 解 escape；Email/Github 走 `commands.openUrl` (`mailto:` 在 backend allowlist 內)；AboutText 用 computed 把 WPF mini-markup tags (`<R>` / `<B>` / `<L/>`) 剝掉成 plain text（顏色/粗體 emphasis 損失但 user-facing 意思保留，註解寫明 future P13 可上 mini-markup parser）；route `/about` 同 `/settings` 公開
- [x] D8 frontend `windows/WebBrowser.vue` 骨架 + `commands.openUrl` 路由 — `URL_NEEDS_COOKIE_HOSTS` 硬編碼 `tw.beanfun.com` / `hk.beanfun.com` 兩個 host (註解寫明為何不從 backend 查、不從 BeanfunClient 動態取)；cookie-required URL → 開 dialog 同時 toast + `safeOpenUrl` 立刻 pop external browser；其他 URL 試 iframe + sandbox + `referrerpolicy="no-referrer-when-downgrade"`，並提供 "Open externally" button fallback；`safeOpenUrl` helper 集中處理 `commands.openUrl` 失敗 toast (DRY)；P13 真正接 `WebviewWindow + cookie sync` 才會由 P12.5 KartTools/MapleTools 消費
- [x] D9 i18n key audit — WPF locale tree 已有的 key 全 reuse verbatim（Settings: `Settings` / `AppName` / `ManageAccount` / `UpdateChannel` / `Stable` / `Development` / `Language` / `ThemeColor` / `LoginMode` / `Regular` / `QrCode` / `AutoCheckUpdate` / `RunAfterLogin` / `MinimizeToTaskbar` / `DisableHardwareAcceleration` / `MsgRestartForHardwareAccel` / `MsgRestartForHardwareAccelTitle` / `Game` / `GamePath` / `TraditionalLoginMode` / `KillPatcher` / `SkipPlayWindow` / `Tools` / `Back` / `FileDialog_Filter` / `FileDialog_Title` / `GameSelected`；About: `AppName` / `Version` / `CheckUpdate` / `AboutText` / `Contact` / `Email` / `Yes` / `No` / `Back` / `Feedback` / `FeedbackText` / `NewVersionDetected` / `NoUpdatesDetected`）；frontend-only key 補進 `i18n/messages.ts` 三 locale 同步（`settings.{subtitle,aboutLink,gamePathPlaceholder,gameSectionEmpty,disableHardwareAccelerationTip,tradLoginTip,killPatcherTip,skipPlayWindowTip}` 8 leaf + `webBrowser.{title,empty,cookieRequired,openExternally}` 4 leaf；註解寫明為何 WPF tooltip 沒進 JSON locale tree → 因為 WPF tooltip 用 `<TextBlock><Run>` 含 `<LineBreak/>` inline markup，不是 resource string）；dead-key audit 過（speculatively 加的 `settings.settingsLink` 拔掉，因為實際 button 用 WPF key `t('Settings')`）
- [x] D10 quality gates 全綠 — `npm test`：482 passed (34 files)；`npm run lint`：0；`npm run typecheck`：0；`npm run format:check`：5 files 自動 `prettier --write` 修；`cargo fmt --all -- --check`：1 處 pre-existing drift 在 `commands/account.rs` (P12.3 留下) 用 `cargo fmt --all` 機械修；`cargo clippy --all-targets --all-features -- -D warnings`：0；`cargo test --all-features`：676 + 19 + 15 + 15 + … 全 ok；spec 修正：(1) `tests/unit/pages/AccountList.spec.ts` `@element-plus/icons-vue` mock 補 `Setting: stub('SettingStub')`（不然 SettingsIcon 用 Proxy 報 undefined export）；(2) `tests/unit/router/index.spec.ts` route count `5 → 7`，新測 `/settings` + `/about` 都 `requiresAuth: undefined` 對齊 WPF parity 公開語義
- [x] D11 Todo.md backfill — 各 D-step 勾掉 + 設計決策 / WPF parity / quality-gate fix 一段詳寫
- [ ] D12 commit `feat(next): add P12.4 settings, about, and web browser shell`（feat-only，chore Todo backfill 沿 P12.3 慣例）

##### P12.5 — 工具（4 view）

`windows/MapleTools.vue` / `windows/KartTools.vue` / `windows/CoreCalculator.vue` / `windows/EquipCalculator.vue`；純 view 邏輯不打 IPC。

#### 共用驗收

- [ ] WPF XAML → Vue template 對應（結構 + 互動，視覺保留 mockup glassmorphism）
- [ ] WPF code-behind → Pinia action / composable（不在 component 內直接打 IPC）
- [ ] Vitest component test 3+ cases per view
- [ ] i18n key 三 locale 同步（KeysMatch guard 守住 frontend-only message tree）
- [ ] 每 chunk 結尾 quality gates 全綠 + `cargo tauri dev` 視覺 smoke

- **驗收**：所有 11 + 19 = 30 個視圖跟 WPF 視覺 + 互動行為對齊；component tests 全綠

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
- [x] `GameList`（以 dialog 切法）— 由 Stitch 在 P11 階段提供
- [x] `IdPassForm.html` — 由使用者在 P12.2 D1.1 提供（先前漏入庫）
- [x] `AccountList.html` — 由使用者在 P12.2 D1.1 提供（先前漏入庫）
- [x] `QrForm.html` — 由使用者在 P12.2 D1.1 提供（先前漏入庫）
- [x] `Settings.html` — 由使用者在 P12.2 D1.1 提供（先前漏入庫）

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
