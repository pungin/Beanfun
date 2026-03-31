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

        public GamePassBrowser(string skey)
        {
            InitializeComponent();
            _skey = skey;
            Environment.SetEnvironmentVariable(
                "WEBVIEW2_USER_DATA_FOLDER",
                Path.GetTempPath() + "\\Beanfun\\WebView2\\"
            );
            Loaded += OnLoaded;
        }

        private async void OnLoaded(object sender, RoutedEventArgs e)
        {
            Loaded -= OnLoaded;

            // Hide window until GamePass page loads
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
                            cookie.Name, cookie.Value, cookie.Domain, cookie.Path
                        )
                    );
            }
        }

        private async void OnNavigationCompleted(object sender, CoreWebView2NavigationCompletedEventArgs e)
        {
            string url = wb_Main.Source?.ToString() ?? "";

            // On Login/Index page, auto-click the GamePass button
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

            // Once we've left Login/Index, show the window
            if (_hasClickedGamePass && !url.Contains("Login/Index"))
            {
                this.Opacity = 1;
            }

            // TODO: intercept callback URL with akey after GamePass login completes
        }

        private void wb_Main_NavigationStarting(object sender, CoreWebView2NavigationStartingEventArgs e)
        {
            this.Title = "GamePass Login";
        }
    }
}
