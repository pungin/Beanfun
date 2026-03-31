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

            string hintPrefix =
                Application.Current.TryFindResource("AdvanceCheckHintPrefix") as string ?? "提示：";
            lblHint.Text = $"{hintPrefix}{emailHint}";

            // Determine input label based on hint content (email or phone)
            bool isPhone =
                emailHint != null
                && (
                    emailHint.Contains("手機")
                    || emailHint.Contains("電話")
                    || emailHint.Contains("phone")
                );
            string labelKey = isPhone ? "AdvanceCheckInputPhone" : "AdvanceCheckInputEmail";
            lblVerifyInput.Text =
                Application.Current.TryFindResource(labelKey) as string
                ?? (isPhone ? "請輸入認證電話號碼：" : "請輸入認證 Email：");

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
                MessageBox.Show(
                    Application.Current.TryFindResource("AdvanceCheckFillEmailAndCaptcha") as string
                );
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
