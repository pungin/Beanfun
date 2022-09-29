using Microsoft.Web.WebView2.Core;
using System;
using System.Net;
using System.Windows;

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
            Environment.SetEnvironmentVariable("WEBVIEW2_USER_DATA_FOLDER", System.IO.Path.GetTempPath() + "\\Beanfun\\WebView2\\");
            wb_Main.CoreWebView2InitializationCompleted += Wb_Main_CoreWebView2InitializationCompleted;
            wb_Main.Source = new Uri(uri);
        }

        private void Wb_Main_CoreWebView2InitializationCompleted(object sender, CoreWebView2InitializationCompletedEventArgs e)
        {
            wb_Main.CoreWebView2.NewWindowRequested += CoreWebView2_NewWindowRequested;
            wb_Main.CoreWebView2.NavigationCompleted += CoreWebView2_NavigationCompleted;
            if (App.MainWnd.bfClient != null)
            {
                foreach (Cookie cookie in App.MainWnd.bfClient.GetCookies())
                    wb_Main.CoreWebView2.CookieManager.AddOrUpdateCookie(wb_Main.CoreWebView2.CookieManager.CreateCookie(cookie.Name, cookie.Value, cookie.Domain, cookie.Path));
            }
        }

        private void CoreWebView2_NavigationCompleted(object sender, CoreWebView2NavigationCompletedEventArgs e)
        {
            this.Title = wb_Main.CoreWebView2.DocumentTitle;
        }

        private void CoreWebView2_NewWindowRequested(object sender, CoreWebView2NewWindowRequestedEventArgs e)
        {
            wb_Main.CoreWebView2.Navigate(e.Uri);
            e.Handled = true;
        }

        private void Window_MouseLeftButtonDown(object sender, System.Windows.Input.MouseButtonEventArgs e)
        {
            this.DragMove();
        }

        private void wb_Main_NavigationStarting(object sender, CoreWebView2NavigationStartingEventArgs e)
        {
            t_URI.Text = e.Uri;
        }
    }
}
