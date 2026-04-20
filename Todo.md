# beanfun-next Todo

## Working tree — 待 commit（P12.4-followup-B-fix F1-F9）

已實作 + smoke test 通過，等待與下方新需求合併後一次 commit。

- [x] F1 backend allowlist 改 `*.beanfun.com` suffix match
- [x] F2 砍 VideoReport button（目標 URL 長期 404）
- [x] F3-redo mirror WPF L494 game image URL passthrough + unified fallback host
- [x] F6 ManageAccount 補「上一頁」按鈕
- [x] F7 WebviewWindow 開窗黑屏修復（visible(false) + on_page_load show + 5s safety net）
- [x] F8 客服中心按鈕 wire-up（frontend region → static URL → useInAppBrowser）
- [x] F9 會員中心按鈕 wire-up（backend command, web_token 不離開 Rust）

## 新需求

### A. About 頁修改

- [x] A1 作者署名改為 `By Pungin and YCC3741`
- [x] A2 GitHub 連結改為 `https://github.com/pungin/Beanfun`
- [x] A3 發送電郵拆成兩個垂直排列連結：✉ Pungin / ✉ YCC3741

### B. 最小化到通知中心（system tray）

- [x] B1 i18n: zh-TW `最小化到通知中心` / zh-CN `最小化到通知中心`
- [x] B2 Cargo.toml 加 `tray-icon` feature
- [x] B3 lib.rs 加 `.setup()` 建立 TrayIconBuilder + `on_window_event` minimize 攔截
- [x] B4 新增 `tray.rs` — `build_tray()` + `handle_minimize_to_tray()` + async config 讀取
- [x] B5 tray click：左鍵 → show + unminimize + focus + hide tray

### C. 視窗 / 登入頁微調

- [x] C1 預設視窗高度 600 → 720（消除 scroll）
- [x] C2 登入頁標題 `繽放 Next` → 空心 icon + `繽放`（三語同步）
- [x] C3 登入頁 icon 改用空心描邊版
- [x] C4 地區圖示 Flag → 灰粗圓潤 TW / HK 字樣
- [x] C5 版號 0.1.0 → 6.0.0（Cargo.toml + tauri.conf.json + package.json）

### D. PostMessageW 修正 + updater fix

- [x] D1 `windows-app-manifest.xml` + `build.rs`：release 建置加 `highestAvailable`（mirror WPF manifest）
- [x] D2 登入頁 icon 比例修正（移除固定 width/height，改 height:36px + width:auto）
- [x] D3 updater: GitHub release JSON `body: null` 導致 decode 失敗 → `null_as_empty` deserializer
- [x] D4 updater: 純 semver `6.0.0` 掉入 Path B 誤判有新版 → 加 Path C semver 比較
- [x] D5 視窗寬度 800 → 720

## 待處理（下一輪）

- [ ] CI / lint / test / 打包 exe 流程

## Smoke test

- [x] T1 About: 署名 / GitHub 連結 / 雙 email 各開一次
- [x] T2 Tray: 勾選 → minimize → tray 出現 → 左鍵還原 → tray 消失
- [x] T3 Tray: 不勾 → minimize → 正常縮到 taskbar
- [x] T4 登入頁空心 icon + 繽放 + 視窗不再有 scroll
- [x] T5 地區選擇頁：TW / HK 粗體字取代 Flag icon
- [x] T6 About 頁版號顯示 6.0.0
- [x] T7 回歸: PlayerReport / 會員中心 / 客服中心 / game images

## Commit

- [ ] Single commit（F1-F9 + A + B + C + Todo）
