using System.Diagnostics;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
using System.Windows.Media.Imaging;

namespace Beanfun
{
    /// <summary>
    /// qr_form.xaml 的交互逻辑
    /// </summary>
    public partial class qr_form : Page
    {
        public qr_form()
        {
            InitializeComponent();
        }

        private void btn_Refresh_QRCode_Click(object sender, RoutedEventArgs e)
        {
            App.MainWnd.refreshQRCode();
        }

        private void btn_Refresh_QRCode_MouseEnter(object sender, MouseEventArgs e)
        {
            if (qr_Tip.Visibility == Visibility.Collapsed)
            {
                DockPanel.SetDock(btn_Refresh_QRCode, Dock.Left);
                qr_Tip.Visibility = Visibility.Visible;
            }
        }

        private void qr_Tip_Click(object sender, RoutedEventArgs e)
        {
            Process.Start(
                new ProcessStartInfo(
                    "https://tw.beanfun.com/bfevent/bfApp/Page20160930/PC/index.html"
                )
                {
                    UseShellExecute = true,
                }
            );
        }

        private void TextBlock_MouseLeave(object sender, MouseEventArgs e)
        {
            if (qr_Tip.Visibility == Visibility.Visible)
            {
                DockPanel.SetDock(btn_Refresh_QRCode, Dock.Top);
                qr_Tip.Visibility = Visibility.Collapsed;
            }
        }

        private void btn_CopyDeeplink_Click(object sender, RoutedEventArgs e)
        {
            var qrcodeClass = App.MainWnd.qrcodeClass;
            if (qrcodeClass != null && !string.IsNullOrEmpty(qrcodeClass.deeplink))
            {
                try
                {
                    Clipboard.SetText(qrcodeClass.deeplink);
                    MessageBox.Show(
                        Application.Current.TryFindResource("CopyDeeplinkSuccess") as string
                    );
                }
                catch
                {
                    MessageBox.Show(Application.Current.TryFindResource("CopyFailed") as string);
                }
            }
            else
            {
                MessageBox.Show(
                    Application.Current.TryFindResource("CopyDeeplinkNotReady") as string
                );
            }
        }

        private void btn_back_Click(object sender, RoutedEventArgs e)
        {
            App.LoginMethod = (int)LoginMethod.Regular;
            App.MainWnd.loginMethodChanged();
        }

        private void btn_StartGame_Click(object sender, RoutedEventArgs e)
        {
            App.MainWnd.runGame();
        }

        private void CopyQRCode_Click(object sender, RoutedEventArgs e)
        {
            if (qr_image.Source is BitmapSource bmp)
            {
                try
                {
                    Clipboard.SetImage(bmp);
                }
                catch { }
            }
        }

        private void EnlargeQRCode_Click(object sender, RoutedEventArgs e)
        {
            if (qr_image.Source == null)
                return;

            var wnd = new Window
            {
                Title = "QR Code",
                SizeToContent = SizeToContent.WidthAndHeight,
                WindowStartupLocation = WindowStartupLocation.CenterOwner,
                Owner = Window.GetWindow(this),
                ResizeMode = ResizeMode.NoResize,
                Content = new Image
                {
                    Source = qr_image.Source,
                    Width = 300,
                    Height = 300,
                },
            };
            wnd.ShowDialog();
        }
    }
}
