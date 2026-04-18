using System;
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
                    WindowsAPI.CopyText(qrcodeClass.deeplink);
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
                    var encoder = new PngBitmapEncoder();
                    encoder.Frames.Add(BitmapFrame.Create(bmp));
                    var stream = new System.IO.MemoryStream();
                    encoder.Save(stream);

                    var data = new DataObject();
                    data.SetData("PNG", stream);
                    stream.Position = 0;
                    data.SetData(DataFormats.Bitmap, bmp);
                    Clipboard.SetDataObject(data, true);

                    ShowToast(
                        Application.Current.TryFindResource("CopyQRCodeSuccess") as string
                            ?? "QR Code copied!"
                    );
                }
                catch
                {
                    ShowToast(
                        Application.Current.TryFindResource("CopyFailed") as string
                            ?? "Copy failed",
                        false
                    );
                }
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

        private void ShowToast(string message, bool success = true)
        {
            toastText.Text = (success ? "✓ " : "") + message;
            toastBorder.Background = new System.Windows.Media.SolidColorBrush(
                (System.Windows.Media.Color)
                    System.Windows.Media.ColorConverter.ConvertFromString(
                        success ? "#CC2E7D32" : "#CC333333"
                    )
            );
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
