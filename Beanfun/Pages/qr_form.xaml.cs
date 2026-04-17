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
                    ShowToast(
                        Application.Current.TryFindResource("CopyQRCodeSuccess") as string
                            ?? "QR Code copied!"
                    );
                }
                catch { }
            }
        }

        private Window _enlargeWnd;

        public void CloseEnlargeWindow()
        {
            if (_enlargeWnd != null)
            {
                _enlargeWnd.Close();
                _enlargeWnd = null;
            }
        }

        private void EnlargeQRCode_Click(object sender, RoutedEventArgs e)
        {
            if (qr_image.Source == null)
                return;

            CloseEnlargeWindow();

            _enlargeWnd = new Window
            {
                Title = "QR Code",
                Width = 350,
                Height = 350,
                WindowStartupLocation = WindowStartupLocation.CenterOwner,
                Owner = Window.GetWindow(this),
                ResizeMode = ResizeMode.CanResize,
                Content = new Image
                {
                    Source = qr_image.Source,
                    Stretch = System.Windows.Media.Stretch.Uniform,
                },
            };
            _enlargeWnd.Closed += (s, _) => _enlargeWnd = null;
            _enlargeWnd.Show();
        }

        private void ShowToast(string message)
        {
            toastText.Text = message;
            toastBorder.Visibility = Visibility.Visible;
            var timer = new System.Windows.Threading.DispatcherTimer
            {
                Interval = TimeSpan.FromSeconds(2),
            };
            timer.Tick += (s, _) =>
            {
                timer.Stop();
                toastBorder.Visibility = Visibility.Collapsed;
            };
            timer.Start();
        }
    }
}
