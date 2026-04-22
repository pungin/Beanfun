# 架構 (Architecture)

```
Beanfun/
├── src/                     # Vue 3 前端
│   ├── pages/               #   頁面元件 (Login, AccountList, Settings, About…)
│   ├── components/          #   共用元件 (TitleBar, QrForm, GamepassForm…)
│   ├── composables/         #   組合式函式 (useAuth, useGameLauncher…)
│   ├── stores/              #   Pinia 狀態管理
│   ├── services/            #   Tauri IPC 呼叫封裝
│   ├── router/              #   Vue Router + fitWindow / auth guard
│   ├── i18n/, locales/      #   vue-i18n 多語系
│   ├── styles/              #   全域樣式
│   ├── constants/           #   常數
│   ├── types/               #   共用 TypeScript 型別
│   ├── assets/              #   圖片 / icon
│   └── windows/             #   多視窗進入點 (In-App Browser, ServiceAccountInfo…)
├── src-tauri/               # Rust 後端 (Tauri v2)
│   ├── src/
│   │   ├── commands/        #     IPC command handlers (auth/account/session/
│   │   │                    #     launcher/storage/system/otp/web_browser/
│   │   │                    #     cookie_native/config/game/update/maple_cache)
│   │   ├── core/            #     核心邏輯 (parser, crypto, version)
│   │   └── services/        #     服務層
│   │       ├── beanfun/     #       登入 / 帳號 / session / ping keep-alive
│   │       ├── config/      #       Config.xml 讀寫
│   │       ├── game/        #       遊戲啟動 + LocaleRemulator
│   │       ├── maple_cache/ #       楓之谷快取讀取 / 清理
│   │       ├── process/     #       視窗操作 / PostMessage
│   │       ├── storage/     #       Users.dat 加解密 (DPAPI)
│   │       ├── registry/    #       Windows Registry
│   │       ├── system/      #       系統工具 (open URL…)
│   │       └── updater/     #       自動更新檢查
│   ├── capabilities/        #     Tauri v2 權限配置
│   ├── LocaleRemulator/     #     LR 預編譯二進位檔
│   ├── icons/               #     App icon
│   └── tests/               #     整合測試
├── tests/                   # 前端單元測試 (Vitest)
├── public/                  # Vite 靜態資源
├── scripts/                 # 建置 / 轉換腳本
├── Lang/                    # WPF 時代 XAML 語系檔 → i18n 來源
└── .github/workflows/       # CI (lint, format, test, build, release)
```
