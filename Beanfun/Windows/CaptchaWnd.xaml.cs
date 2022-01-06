using System;
using System.IO;
using System.Windows;
using System.Windows.Input;
using System.Windows.Media.Imaging;

namespace Beanfun
{
    /// <summary>
    /// CaptchaWnd.xaml 的互動邏輯
    /// </summary>
    public partial class CaptchaWnd : Window
    {
        public string Captcha { get { return CodeTextBox.Text; } }
        private string samplecaptcha;
        private BeanfunClient Client;

        public CaptchaWnd(BeanfunClient client, string samplecaptcha)
        {
            InitializeComponent();
            this.samplecaptcha = samplecaptcha;
            Client = client;
            Button_Click(null, null);
        }

        private void Window_MouseLeftButtonDown(object sender, MouseButtonEventArgs e)
        {
            this.DragMove();
        }

        private void Button_Click(object sender, RoutedEventArgs e)
        {
            BitmapImage result;
            try
            {
                byte[] buffer = Client.DownloadData("https://tw.newlogin.beanfun.com/login/BotDetectCaptcha.ashx?get=image&c=c_login_idpass_form_samplecaptcha&t=" + samplecaptcha);
                result = new BitmapImage();
                result.BeginInit();
                result.StreamSource = new MemoryStream(buffer);
                result.EndInit();
            }
            catch (Exception)
            {
                MessageBox.Show(TryFindResource("LoadCaptchaFailed") as string);
                return;
            }
            c_login_idpass_form_samplecaptcha_CaptchaImage.Source = result;
        }

        private void Button_Click_1(object sender, RoutedEventArgs e)
        {
            this.Close();
        }
    }
}
