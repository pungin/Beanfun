using System;
using System.IO;
using System.Net;
using System.Windows;
using Microsoft.Web.WebView2.Core;

namespace Beanfun
{
    /// <summary>
    /// WebBrowser.xaml 的交互逻辑
    /// </summary>
    public partial class WebBrowser : Window
    {
        private readonly string _initialUri;

        public WebBrowser(string uri)
        {
            InitializeComponent();
            _initialUri = uri;
            Environment.SetEnvironmentVariable(
                "WEBVIEW2_USER_DATA_FOLDER",
                Path.GetTempPath() + "\\Beanfun\\WebView2\\"
            );
            Loaded += WebBrowser_Loaded;
        }

        private async void WebBrowser_Loaded(object sender, RoutedEventArgs e)
        {
            Loaded -= WebBrowser_Loaded;
            wb_Main.CoreWebView2InitializationCompleted +=
                Wb_Main_CoreWebView2InitializationCompleted;

            if (bool.Parse(ConfigAppSettings.GetValue("disableHardwareAcceleration", "false")))
            {
                string userDataFolder = Path.Combine(Path.GetTempPath(), "Beanfun", "WebView2");
                var options = new CoreWebView2EnvironmentOptions();
                options.AdditionalBrowserArguments = "--disable-gpu --disable-gpu-compositing";
                CoreWebView2Environment env = await CoreWebView2Environment.CreateAsync(
                    null,
                    userDataFolder,
                    options
                );
                await wb_Main.EnsureCoreWebView2Async(env);
            }

            wb_Main.Source = new Uri(_initialUri);
        }

        private void Wb_Main_CoreWebView2InitializationCompleted(
            object sender,
            CoreWebView2InitializationCompletedEventArgs e
        )
        {
            wb_Main.CoreWebView2.NewWindowRequested += CoreWebView2_NewWindowRequested;
            wb_Main.CoreWebView2.NavigationCompleted += CoreWebView2_NavigationCompleted;
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

        private void CoreWebView2_NavigationCompleted(
            object sender,
            CoreWebView2NavigationCompletedEventArgs e
        )
        {
            this.Title = wb_Main.CoreWebView2.DocumentTitle;
        }

        private void CoreWebView2_NewWindowRequested(
            object sender,
            CoreWebView2NewWindowRequestedEventArgs e
        )
        {
            wb_Main.CoreWebView2.Navigate(e.Uri);
            e.Handled = true;
        }

        private void Window_MouseLeftButtonDown(
            object sender,
            System.Windows.Input.MouseButtonEventArgs e
        )
        {
            this.DragMove();
        }

        private void wb_Main_NavigationStarting(
            object sender,
            CoreWebView2NavigationStartingEventArgs e
        )
        {
            t_URI.Text = e.Uri;
        }
    }
}
