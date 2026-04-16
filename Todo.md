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

### P0 — 專案骨架 + CI

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
- [ ] **0.4 Lint / Format 設定**
  - [ ] `beanfun-next/rustfmt.toml`
  - [ ] `beanfun-next/src-tauri/clippy.toml`
  - [ ] `beanfun-next/.eslintrc.cjs` + `@vue/eslint-config-typescript` + `eslint-plugin-vue`
  - [ ] `beanfun-next/.prettierrc`
  - [ ] `.editorconfig`（repo 根）
- [ ] **0.5 Smoke tests**
  - [ ] 前端：`beanfun-next/tests/unit/smoke.spec.ts`
  - [ ] 後端：`beanfun-next/src-tauri/tests/smoke.rs`
- [ ] **0.6 GitHub Actions CI**（`.github/workflows/beanfun-next-ci.yml`）
  - [ ] matrix: `windows-latest` + `macos-latest`
  - [ ] job: rust fmt + clippy + test
  - [ ] job: frontend lint + test
  - [ ] 只在 `beanfun-next/**` 變動或手動觸發
- **Chunk 2 驗收**：本機 `cargo fmt --check && cargo clippy -- -D warnings && cargo test && npm run lint && npm run test` 全綠

**Chunk 3 — commitlint + README**
- [ ] **0.7 Commitlint**（CI-only）
  - [ ] `commitlint.config.js`（repo 根）
  - [ ] `.github/workflows/commitlint.yml`
- [ ] **0.8 README 骨架**
  - [ ] `beanfun-next/README.md`（dev / build / test 指令）
- **Chunk 3 驗收**：CI 跑過、README 資訊齊

- **P0 總驗收**：`cargo check && cargo clippy -- -D warnings && cargo fmt --check && npm run lint && npm run test` 全綠、CI 綠、`npm run tauri dev` 可跑

### P1 — Rust `core/wcdes`（DES/ECB/NoPadding）

- [ ] `core/wcdes/mod.rs`：`encrypt_hex(str: &str, key: &str) -> Result<String>`、`decrypt_hex(hex: &str, key: &str) -> Result<String>`
- [ ] 行為對齊 C# `DES.Create() + Mode=ECB + Padding=None + Encoding.ASCII`
- [ ] 單元測試：8-byte / 16-byte / 24-byte plaintext
- [ ] Fixture 測試：用 WPF 版跑的 (key, plaintext, ciphertext) 三元組驗證
- **驗收**：`cargo test core::wcdes` 全綠、cipher 字節級等同 WPF

### P2 — Rust `core/version` + `core/parser`

- [ ] `core/version/mod.rs`：`is_newer(local: &str, remote: &VersionInfo) -> bool`
- [ ] 覆蓋 WPF `IsNewerVersion` 所有 case（5.8.9 < 5.8.10、timestamp 相同、舊格式無 patch）
- [ ] `core/parser/viewstate.rs`：`extract_viewstate(html: &str) -> Result<ViewStateForm>`（`__VIEWSTATE` / `__VIEWSTATEGENERATOR` / `__EVENTVALIDATION`）
- [ ] `core/parser/account.rs`：從 `game_server_account_list.aspx` HTML 抽出 ServiceAccount 清單
- [ ] `core/parser/akey.rs`：從 redirect URL 抓 `akey=xxx`
- [ ] `core/parser/token.rs`：從 HTML 抓 `__RequestVerificationToken`
- [ ] 單元測試：每個 parser 5+ cases（含 WPF 實際 response 當 fixture）
- **驗收**：parser 全部單元測試綠、行覆蓋 >= 95%

### P3 — Rust `services/beanfun` Login

- [ ] `services/beanfun/client.rs`：`BeanfunClient`（`reqwest` + `cookie_store` + header helpers）
- [ ] `services/beanfun/headers.rs`：`SetBaseHeaders` / `SetJsonHeaders` 等價
- [ ] `services/beanfun/login_tw.rs`：Regular TW 完整 flow（`CheckAccountType` → `AccountLogin` → `SendLogin` → `return.aspx` 取 `bfWebToken`）
- [ ] `services/beanfun/login_hk.rs`：Regular HK 完整 flow（含 VIEWSTATE）
- [ ] `services/beanfun/totp.rs`：TOTP 6 格 flow
- [ ] `services/beanfun/qrcode.rs`：`init_login` / `get_qr_image` / `check_login_status` / `qrcode_login` / `send_login`
- [ ] `services/beanfun/gamepass.rs`：接收前端傳來的 cookies + webtoken，完成 `get_accounts` + `get_remain_point`
- [ ] `services/beanfun/session.rs`：`get_sessionkey` / `logout`
- [ ] `services/beanfun/misc.rs`：`ping` / `get_remain_point` / `get_email`
- [ ] 錄製 wiremock fixture（從現行 WPF 版跑出真實回應）
- [ ] Integration tests（wiremock）：
  - [ ] Regular TW 成功 / 密碼錯 / AdvanceCheck 觸發
  - [ ] Regular HK 成功 / TOTP 觸發 / 驗證碼觸發
  - [ ] QR 完整 flow（InitLogin → poll 3 次 → Success）
  - [ ] QR Token Expired
  - [ ] Logout
  - [ ] Ping / getRemainPoint / getEmail
- **驗收**：15+ integration cases pass

### P4 — Rust `services/beanfun` Account / OTP / Verify

- [ ] `services/beanfun/account.rs`：
  - [ ] `get_accounts(service_code, service_region)`
  - [ ] `add_service_account(name, ...)`
  - [ ] `change_service_account_display_name(...)`
  - [ ] `get_service_contract(...)`
  - [ ] `unconnected_game_init_add_account_payload(...)`
  - [ ] `unconnected_game_add_account_check(...)` / `check_nickname(...)`
  - [ ] `unconnected_game_add_account(...)`
  - [ ] `unconnected_game_change_password(...)`
- [ ] `services/beanfun/otp.rs`：`get_otp(account, service_code, service_region)` 完整 long-polling flow，呼叫 `core/wcdes::decrypt_hex`
- [ ] `services/beanfun/verify.rs`：
  - [ ] `get_verify_page_info()`
  - [ ] `get_verify_captcha(sample: &str) -> base64 png`
  - [ ] `submit_verify(viewstate, eventvalidation, sample, code, captcha)`
- [ ] Integration tests：每個 endpoint 至少 2 cases（成功 + 錯誤）
- **驗收**：15+ integration cases pass

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
