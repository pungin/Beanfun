# Beanfun

[![GitHub all releases](https://img.shields.io/github/downloads/pungin/Beanfun/total)](https://github.com/pungin/Beanfun/releases)
[![Lint, Format & Test](https://github.com/pungin/Beanfun/actions/workflows/ci.yml/badge.svg)](https://github.com/pungin/Beanfun/actions/workflows/ci.yml)

> **遊戲橘子數位科技旗下遊戲的第三方啟動器**

> **📢 開發狀態與雙線更新公告 (Project Status)**
>
> 本專案目前正進行底層現代化重構（採用 Rust + Vue + Tauri）。
> 為提供社群更具彈性的選擇，本專案將與另一開源啟動器 **[MapleLink](https://github.com/lshw54/maplelink)** 維持雙線並行開發：
>
> - **Beanfun**：以支援**所有橘子旗下遊戲**為目標，持續優化並提供高相容性的全生態服務。
> - **MapleLink**：專為純《新楓之谷》玩家打造，作為新技術實驗與快速排查登入異常問題的先行區。
>
> 詳情請參閱：[關於 Beanfun 與 MapleLink 雙線並行開發的說明 (#294)](https://github.com/pungin/Beanfun/issues/294)

**免責聲明：** 本軟體 **不是** 遊戲橘子旗下科技所開發的官方客戶端程式。若您的帳號使用第三方的方式登錄，請自行三思並且確認下載當前程式的來源是否安全。

- 本程式使用部份 `BeanfunLogin` 的代碼。
- 程式使用 [Locale_Remulator](https://github.com/InWILL/Locale_Remulator) 作為語言模擬元件，支援 32-bit 及 64-bit 遊戲。

---

## 下載與使用 (Getting Started)

### 系統要求 (Prerequisites)

- **作業系統：** Windows 10 或以上
- **必備元件：** [WebView2 Runtime](https://developer.microsoft.com/en-us/microsoft-edge/webview2/)（Windows 11 已預裝）

### 使用方法 (Usage)

前往 **[最新發行版 (Releases)](https://github.com/pungin/Beanfun/releases/latest)** 下載：

| 版本         | 檔案                          | 說明                                                                                 |
| ------------ | ----------------------------- | ------------------------------------------------------------------------------------ |
| **免安裝版** | `Beanfun.exe`                 | 放至任意全英文路徑的資料夾，直接執行即可。需系統已安裝 WebView2 Runtime。            |
| **安裝版**   | `Beanfun_x.x.x_x64-setup.exe` | 適用於精簡版 Windows 等未預裝 WebView2 的環境，安裝過程會自動安裝 WebView2 Runtime。 |

> **⚠ 注意事項說明：**
>
> - 啟動遊戲時程式會在執行資料夾生成 `LRProc.exe`、`LRHookx32.dll`、`LRHookx64.dll` 等件。
> - `LRProc.exe` — 負責 Hook DLL 載入至程序中
> - `LRHookx32.dll` / `LRHookx64.dll` — 語言模擬元件

---

## 技術棧 (Built With)

- **[Tauri v2](https://tauri.app/)** — 桌面殼層（WebView2）
- **[Rust](https://www.rust-lang.org/)** — 後端核心
- **[Vue 3](https://vuejs.org/) + [TypeScript](https://www.typescriptlang.org/) + [Vite](https://vite.dev/)** — 前端框架
- **[Element Plus](https://element-plus.org/)** — UI 元件庫
- **[Locale_Remulator](https://github.com/InWILL/Locale_Remulator)** — 語言模擬元件

---

## 架構 (Architecture)

採前後端分離設計：

- **前端 (`src/`)**：Vue 3 + TypeScript + Vite，負責 UI 與多視窗（In-App Browser、ServiceAccountInfo…）。
- **後端 (`src-tauri/`)**：Rust + Tauri v2，提供 IPC commands 與服務層（登入 / 帳號 / 遊戲啟動 / 設定檔 / 加密儲存 / 自動更新…）。
- **語系**：`Lang/`（WPF 時代 XAML 來源）→ `src/i18n/` + `src/locales/`（vue-i18n）。

完整目錄樹與模組說明請見 **[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)**。

---

## 開發 (Development)

### 環境需求

| 需求                      | 版本                                  |
| ------------------------- | ------------------------------------- |
| Node.js                   | >= 22 LTS                             |
| Rust                      | stable (x86_64-pc-windows-msvc)       |
| WebView2 Runtime          | Windows 11 預裝；Windows 10 需安裝    |
| Visual Studio Build Tools | 安裝 **Desktop development with C++** |

### 快速開始

```sh
npm install
npm run tauri dev
```

### 常用指令

```sh
# 前端
npm run lint              # ESLint
npm run format:check      # Prettier
npm run typecheck         # TypeScript 型別檢查
npm run test              # Vitest 單元測試

# 後端 (src-tauri/)
cargo fmt --check         # Rust 格式檢查
cargo clippy -- -D warnings
cargo test
```

---

## 貢獻 (Contributing)

1. 從 `code` 分支出新 feature branch。
2. PR 到 `code` 時會跑 CI（lint / format / typecheck / test）。
3. 送 PR 前請先跑 `npm run format` + `cargo fmt`。
