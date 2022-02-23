# Beanfun

[![GitHub all releases](https://img.shields.io/github/downloads/pungin/Beanfun/total)](https://github.com/pungin/Beanfun/releases)

遊戲橘子數位科技旗下遊戲的啟動器

本程式**不是**遊戲橘子數位科技開發的官方客戶端程式

關於遊戲帳號使用第三方的方式登入請再三斟酌並且請確認您下載當前程式的途徑是否安全

程式使用部分BeanfunLogin的代碼

程式使用<https://github.com/InWILL/Locale_Remulator>作為區域模擬元件，支持32bit和64bit遊戲

## Download

Download available at <https://github.com/pungin/Beanfun/releases/latest>.

## Getting Started

### Prerequisites

* [.NET Framework 4.8](https://dotnet.microsoft.com/en-us/download/dotnet-framework/net48)
* [Microsoft Visual C++ Redistributable](https://docs.microsoft.com/zh-CN/cpp/windows/latest-supported-vc-redist?view=msvc-170)

### Usage

下載`Beanfun.exe`後直接運行即可

啟動遊戲時會在當前資料夾釋放`LRProc.dll`和`LRHookx32.dll`或`LRHookx64.dll`文件
* `LRProc.dll` - 將`LRHookx32.dll`或`LRHookx64.dll`載入到遊戲中
* `LRHookx32.dll`或`LRHookx64.dll` - 區域模擬元件

## Built With

* [ini-parser](https://github.com/rickyah/ini-parser) - ini元件
* [log4net](https://logging.apache.org/log4net/) - 日誌元件
* [Newtonsoft.Json](https://www.newtonsoft.com/json) - JSON元件
* [Detours](https://github.com/microsoft/Detours) - Used to hook ANSI/Unicode functions
* [Locale_Remulator](https://github.com/InWILL/Locale_Remulator) - 區域模擬元件