using System;
using System.IO;
using System.Net;
using System.Windows;
using Microsoft.Web.WebView2.Core;

namespace Beanfun
{
    public partial class GamePassBrowser : Window
    {
        private readonly string _skey;
        private bool _hasClickedGamePass = false;
        private bool _loginCompleted = false;

        public GamePassBrowser(string skey)
        {
            InitializeComponent();
            _skey = skey;
            Environment.SetEnvironmentVariable(
                "WEBVIEW2_USER_DATA_FOLDER",
                Path.GetTempPath() + "\\Beanfun\\WebView2\\"
            );
            Loaded += OnLoaded;
            Closed += OnClosed;
        }

        private void OnClosed(object sender, EventArgs e)
        {
            if (!_loginCompleted)
            {
                // User closed window without completing login, reset bfClient
                App.MainWnd.bfClient = null;
            }
        }

        private async void OnLoaded(object sender, RoutedEventArgs e)
        {
            Loaded -= OnLoaded;
            this.Opacity = 0;

            wb_Main.CoreWebView2InitializationCompleted += OnWebViewReady;

            if (bool.Parse(ConfigAppSettings.GetValue("disableHardwareAcceleration", "false")))
            {
                string userDataFolder = Path.Combine(Path.GetTempPath(), "Beanfun", "WebView2");
                var options = new CoreWebView2EnvironmentOptions();
                options.AdditionalBrowserArguments = "--disable-gpu --disable-gpu-compositing";
                var env = await CoreWebView2Environment.CreateAsync(null, userDataFolder, options);
                await wb_Main.EnsureCoreWebView2Async(env);
            }

            wb_Main.Source = new Uri($"https://login.beanfun.com/Login/Index?pSKey={_skey}");
        }

        private void OnWebViewReady(object sender, CoreWebView2InitializationCompletedEventArgs e)
        {
            wb_Main.CoreWebView2.NewWindowRequested += (s, args) =>
            {
                wb_Main.CoreWebView2.Navigate(args.Uri);
                args.Handled = true;
            };

            wb_Main.CoreWebView2.NavigationCompleted += OnNavigationCompleted;

            if (App.MainWnd.bfClient != null)
            {
                foreach (Cookie cookie in App.MainWnd.bfClient.GetCookies())
                    wb_Main.CoreWebView2.CookieManager.AddOrUpdateCookie(
                        wb_Main.CoreWebView2.CookieManager.CreateCookie(
                            cookie.Name,
                            cookie.Value,
                            cookie.Domain,
                            cookie.Path
                        )
                    );
            }
        }

        private async void OnNavigationCompleted(
            object sender,
            CoreWebView2NavigationCompletedEventArgs e
        )
        {
            string url = wb_Main.Source?.ToString() ?? "";

            // Auto-click GamePass button on login page
            if (!_hasClickedGamePass && url.Contains("Login/Index"))
            {
                _hasClickedGamePass = true;
                await wb_Main.CoreWebView2.ExecuteScriptAsync(
                    @"(function() {
                        var btn = document.querySelector('a.use-gama-pass');
                        if (btn) btn.click();
                    })()"
                );
                return;
            }

            // Show window once past login page
            if (_hasClickedGamePass && !url.Contains("Login/Index"))
                this.Opacity = 1;

            // After callback completes, beanfun redirects to SendLogin then return.aspx
            // Check if we've landed back on beanfun with bfWebToken cookie set
            if (
                url.Contains("beanfun.com")
                && (
                    url.Contains("return.aspx")
                    || url.Contains("index.aspx")
                    || url.Contains("SendLogin")
                )
            )
            {
                await TryCompleteLogin();
            }
        }

        private async System.Threading.Tasks.Task TryCompleteLogin()
        {
            try
            {
                var twCookies = await wb_Main.CoreWebView2.CookieManager.GetCookiesAsync(
                    "https://tw.beanfun.com"
                );
                var loginCookies = await wb_Main.CoreWebView2.CookieManager.GetCookiesAsync(
                    "https://login.beanfun.com"
                );
                var newLoginCookies = await wb_Main.CoreWebView2.CookieManager.GetCookiesAsync(
                    "https://tw.newlogin.beanfun.com"
                );

                string webToken = null;
                foreach (var cookie in twCookies)
                {
                    if (cookie.Name == "bfWebToken")
                    {
                        webToken = cookie.Value;
                        break;
                    }
                }

                if (string.IsNullOrEmpty(webToken))
                    return;

                // Convert WebView2 cookies to System.Net.Cookie for BeanfunClient
                var allCookies = new System.Collections.Generic.List<Cookie>();
                ConvertCookies(twCookies, allCookies);
                ConvertCookies(loginCookies, allCookies);
                ConvertCookies(newLoginCookies, allCookies);

                Dispatcher.Invoke(() =>
                {
                    _loginCompleted = true;
                    this.Close();
                    App.MainWnd.GamePassLoginCompleted(webToken, allCookies);
                });
            }
            catch { }
        }

        private void ConvertCookies(
            System.Collections.Generic.IReadOnlyList<CoreWebView2Cookie> source,
            System.Collections.Generic.List<Cookie> target
        )
        {
            foreach (var wv2Cookie in source)
            {
                try
                {
                    target.Add(
                        new Cookie(
                            wv2Cookie.Name,
                            wv2Cookie.Value,
                            string.IsNullOrEmpty(wv2Cookie.Path) ? "/" : wv2Cookie.Path,
                            wv2Cookie.Domain.TrimStart('.')
                        )
                    );
                }
                catch { }
            }
        }

        private void wb_Main_NavigationStarting(
            object sender,
            CoreWebView2NavigationStartingEventArgs e
        )
        {
            this.Title =
                Application.Current.TryFindResource("GamePassLogin") as string ?? "GamePass Login";
        }
    }
}
