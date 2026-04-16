# beanfun-next

> 🚧 **開發中 (alpha)** — 以 **Tauri v2 + Rust + Vue 3** 重寫 [pungin/Beanfun](https://github.com/pungin/Beanfun)（原 C# / WPF）的 beanfun! 啟動器。

本資料夾為重寫版原始碼。原 C# / WPF 版本仍保留在 repo 根目錄的 [`Beanfun/`](../Beanfun)，持續接受小修正，直到 beanfun-next 功能追平後才會正式退場。重寫目標、里程碑與 P0 ~ P14 進度請見 repo 根目錄的 [`Todo.md`](../Todo.md)。

---

## 技術棧 (Tech Stack)

| 分層               | 技術                                                                                                                                |
| ------------------ | ----------------------------------------------------------------------------------------------------------------------------------- |
| 桌面殼層           | [Tauri v2](https://tauri.app/) + WebView2 (Windows)                                                                                 |
| 後端               | Rust stable（`reqwest` / `reqwest_cookie_store` / `tokio` / `serde` / `quick-xml` / `tracing` / `des` + `sha2`）                    |
| Windows FFI        | [`windows`](https://crates.io/crates/windows) / [`winreg`](https://crates.io/crates/winreg) / [`wmi`](https://crates.io/crates/wmi) |
| 前端               | Vue 3 + TypeScript + [Vite](https://vite.dev/)                                                                                      |
| UI 元件            | [Element Plus](https://element-plus.org/) + `@element-plus/icons-vue`                                                               |
| 狀態 / i18n / 路由 | Pinia (+ `pinia-plugin-persistedstate`) / vue-i18n v11 / vue-router v4                                                              |
| 測試               | Vitest (jsdom) + `@vue/test-utils` / `cargo test` + `wiremock` + `axum`                                                             |

---

## 開發環境 (Development Environment)

| 需求                      | 版本                                  | 用途             |
| ------------------------- | ------------------------------------- | ---------------- |
| Node.js                   | ≥ 22 LTS                              | 前端 / Tauri CLI |
| Rust                      | stable (x86_64-pc-windows-msvc)       | 後端             |
| WebView2 Runtime          | Windows 11 預裝；Windows 10 需安裝    | 嵌入式瀏覽器核心 |
| Visual Studio Build Tools | 安裝 **Desktop development with C++** | Rust MSVC linker |

macOS / Linux 僅支援跑前端與 `cargo check`（正式產品只針對 Windows），CI 會在 `macos-latest` 做交叉平台理智檢查。

---

## 快速開始 (Quick Start)

```sh
# 第一次安裝前端相依
npm install

# 啟動 Tauri dev（自動跑 Vite + cargo run，視窗載入 http://localhost:1420）
npm run tauri dev

# 產出 installer（之後 P13 Release 會再加簽章 / MSI / NSIS 選項）
npm run tauri build
```

首次執行 `tauri dev` 會編譯約 500 個 Rust crate，耗時約 1 分鐘；後續 incremental build 約 5 ~ 15 秒。

---

## 常用指令 (Common Commands)

### 前端 (`beanfun-next/`)

| 指令                                | 說明                                            |
| ----------------------------------- | ----------------------------------------------- |
| `npm run dev`                       | 只跑 Vite dev server（不啟動 Tauri 視窗）       |
| `npm run build`                     | 前端 `vue-tsc --noEmit` + Vite build            |
| `npm run preview`                   | Preview built frontend                          |
| `npm run typecheck`                 | `vue-tsc --noEmit` 型別檢查                     |
| `npm run lint` / `lint:fix`         | ESLint（ESLint 9 flat config，Vue + TS preset） |
| `npm run format` / `format:check`   | Prettier 格式化 / 檢查                          |
| `npm run test` / `test:watch`       | Vitest 單元測試                                 |
| `npm run tauri dev` / `tauri build` | Tauri CLI 入口                                  |

### 後端 (`beanfun-next/src-tauri/`)

```sh
cd src-tauri

cargo check                           # 只檢查語法 / 型別
cargo fmt --check                     # rustfmt 格式檢查
cargo clippy --all-targets -- -D warnings  # 所有 warning 當 error
cargo test                            # 跑所有測試
```

---

## 資料夾結構 (Project Structure)

```
beanfun-next/
├── mockups/              # P-1 UI mockups（Glassmorphism + Fluent + Soft Depth）
├── public/               # Vite 靜態資源
├── src/                  # Vue 3 前端程式碼
│   ├── App.vue
│   ├── main.ts
│   └── assets/
├── src-tauri/            # Rust 後端 + Tauri 設定
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   ├── build.rs
│   ├── capabilities/     # Tauri v2 permission / capability 設定
│   ├── icons/            # App icons（由 Beanfun/Resources/icon.ico 產生）
│   ├── src/              # Rust 原始碼
│   └── tests/            # Rust integration tests
├── tests/
│   └── unit/             # Vitest 前端單元測試
├── eslint.config.js      # ESLint 9 flat config
├── rustfmt.toml          # Rust formatter
├── vitest.config.ts      # Vitest 設定（jsdom）
└── vite.config.ts        # Vite 設定
```

---

## 測試 (Testing)

- 前端單元測試：`beanfun-next/tests/unit/**/*.spec.ts`（Vitest + `@vue/test-utils` + jsdom）
- 後端整合測試：`beanfun-next/src-tauri/tests/**/*.rs`（Cargo integration tests）
- HTTP mock：後續 P2+ 將使用 [`wiremock`](https://docs.rs/wiremock/) + `axum` 做真實 HTTPS 流程錄製 / 回放

---

## 開發規範 (Contribution)

1. **分支**：從 `code` 分支出新 feature branch（例：`feat/next-<topic>`）。
2. **Commit**：遵循 [Conventional Commits](https://www.conventionalcommits.org/)。PR 到 `code` 時會跑 `commitlint`（見 repo 根目錄 [`commitlint.config.js`](../commitlint.config.js)）。
3. **CI**：`.github/workflows/beanfun-next-ci.yml` 會在 `beanfun-next/**` 變動時跑 `lint / format:check / typecheck / vitest / cargo fmt / cargo clippy -D warnings / cargo test`，`windows-latest` + `macos-latest` 兩平台都要綠才能 merge。
4. **格式化**：送 PR 前請先跑 `npm run format` + `cargo fmt`。

---

## 目前進度 (Roadmap)

詳細里程碑表請見 repo 根目錄的 [`Todo.md`](../Todo.md)。當前狀態：

- ✅ **P-1 UI Mockups**：25 頁 HTML mockups 完成
- ✅ **P0 專案骨架 + CI**：scaffold、lint/format、smoke tests、CI matrix、commitlint、README 完成
- ⏳ **P1+**：尚未開始

---

## 授權 (License)

與主專案 [pungin/Beanfun](https://github.com/pungin/Beanfun) 相同。
