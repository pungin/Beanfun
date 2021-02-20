using System;
using System.Collections.Generic;
using System.Net;
using System.Runtime.InteropServices;
using System.Windows;
using Utility.ModifyRegistry;

namespace Beanfun
{
    /// <summary>
    /// WebBrowser.xaml 的交互逻辑
    /// </summary>
    public partial class WebBrowser : Window
    {
        public WebBrowser(string uri)
        {
            InitializeComponent();
            if (!App.IsWin10) SourceChord.FluentWPF.AcrylicWindow.SetTintOpacity(this, 1.0);
            ChangeUserAgent();
            var webBrowserHelper = new WebBrowserHelper(wb_Main);
            //webBrowserHelper.BeforeNavigate += new EventHandler<WebBrowserExtendedNavigatingEventArgs>(wb_Main_BeforeNavigate);
            webBrowserHelper.BeforeNewWindow += new EventHandler<WebBrowserExtendedNavigatingEventArgs>(wb_Main_BeforeNewWindow);
            if (App.MainWnd.bfClient != null)
            {
                foreach (Cookie cookie in App.MainWnd.bfClient.GetCookies())
                    InternetSetCookie("https://beanfun.com/", cookie.Name, cookie.Value);
            }
            if (App.LoginRegion == "HK")
            {
                configBFSX();
            }
            wb_Main.Navigate(uri);
        }

        private void Window_MouseLeftButtonDown(object sender, System.Windows.Input.MouseButtonEventArgs e)
        {
            this.DragMove();
        }

        [DllImport("wininet.dll", CharSet = CharSet.Auto, SetLastError = true)]
        public static extern bool InternetSetCookie(string lpszUrlName, string lbszCookieName, string lpszCookieData);

        [DllImport("urlmon.dll", CharSet = CharSet.Ansi)]
        private static extern int UrlMkSetSessionOption(int dwOption, string pBuffer, int dwBufferLength, int dwReserved);

        const int URLMON_OPTION_USERAGENT = 0x10000001;

        public void ChangeUserAgent()
        {
            List<string> userAgent = new List<string>();
            string ua = "Mozilla/5.0 (Windows NT 10.0; WOW64; Trident/7.0; rv:11.0) like Gecko";

            UrlMkSetSessionOption(URLMON_OPTION_USERAGENT, ua, ua.Length, 0);
        }

        private void wb_Main_Navigated(object sender, System.Windows.Navigation.NavigationEventArgs e)
        {
            dynamic browser = sender;
            dynamic document = browser.Document;

            t_URI.Text = document.url;
            this.Title = document.title;
        }

        private void wb_Main_BeforeNewWindow(object sender, WebBrowserExtendedNavigatingEventArgs e)
        {
            e.Cancel = true;

            dynamic browser = sender;
            browser.Navigate(e.Url);
        }

        private void configBFSX()
        {
            ModifyRegistry myRegistry = new ModifyRegistry();
            myRegistry.BaseRegistryKey = Microsoft.Win32.Registry.CurrentUser;

            // 允許運行BFServiceX的域名
            myRegistry.SubKey = "Software\\Microsoft\\Windows\\CurrentVersion\\Ext\\Stats\\{8AFB38D0-67A4-49D3-8822-401755FC6573}\\iexplore\\AllowedDomains\\beanfun.com";
            myRegistry.CreateSubKey();
            myRegistry.SubKey = "Software\\Microsoft\\Windows\\CurrentVersion\\Ext\\Stats\\{8AFB38D0-67A4-49D3-8822-401755FC6573}\\iexplore";
            myRegistry.DeleteKey("Blocked");
            myRegistry.DeleteKey("Flags");

            // 啟用BFServiceX元件
            myRegistry.SubKey = "Software\\Microsoft\\Windows\\CurrentVersion\\Ext\\Settings\\{8AFB38D0-67A4-49D3-8822-401755FC6573}";
            myRegistry.DeleteSubKeyTree();

            // 相容性視圖的域名
            myRegistry.SubKey = "Software\\Policies\\Microsoft\\Internet Explorer\\BrowserEmulation\\PolicyList";
            myRegistry.Write("beanfun.com", "beanfun.com");
        }
    }
}
