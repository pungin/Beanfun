using System.IO;
using System.Windows;
using System.Windows.Media.Imaging;

namespace Beanfun
{
    /// <summary>
    /// AdvanceCheckDialog.xaml 的交互逻辑
    /// </summary>
    public partial class AdvanceCheckDialog : Window
    {
        public string Email { get; private set; }
        public string CaptchaCode { get; private set; }

        public AdvanceCheckDialog(string emailHint, byte[] captchaImageBytes)
        {
            InitializeComponent();

            lblHint.Text = $"提示：{emailHint}";

            if (captchaImageBytes != null)
            {
                using (var ms = new MemoryStream(captchaImageBytes))
                {
                    var bitmap = new BitmapImage();
                    bitmap.BeginInit();
                    bitmap.CacheOption = BitmapCacheOption.OnLoad;
                    bitmap.StreamSource = ms;
                    bitmap.EndInit();
                    imgCaptcha.Source = bitmap;
                }
            }
        }

        private void btnOK_Click(object sender, RoutedEventArgs e)
        {
            Email = txtEmail.Text.Trim();
            CaptchaCode = txtCaptcha.Text.Trim();

            if (string.IsNullOrEmpty(Email) || string.IsNullOrEmpty(CaptchaCode))
            {
                MessageBox.Show("請填寫 Email 同驗證碼");
                return;
            }
            DialogResult = true;
        }

        private void btnCancel_Click(object sender, RoutedEventArgs e)
        {
            DialogResult = false;
        }
    }
}
