using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;

namespace Beanfun
{
    /// <summary>
    /// VerifyPage.xaml 的交互逻辑
    /// </summary>
    public partial class VerifyPage : Page
    {
        public VerifyPage()
        {
            InitializeComponent();
        }

        private void Image_MouseLeftButtonDown(object sender, MouseButtonEventArgs e)
        {
            if (App.LoginMethod == (int)LoginMethod.QRCode) App.MainWnd.loginMethodChanged();
            App.MainWnd.return_page = App.MainWnd.loginPage;
        }

        private void Button_Click(object sender, RoutedEventArgs e)
        {
            if (this.t_Verify.Text.Length <= 0)
            {
                MessageBox.Show(TryFindResource("MsgAuthInfoEmpty") as string);
                return;
            }
            if (this.t_Code.Text.Length <= 0)
            {
                MessageBox.Show(TryFindResource("MsgCaptchaCodeEmpty") as string);
                return;
            }
            
            App.MainWnd.verifyWorker.RunWorkerAsync();
        }

        private void Button_Click_1(object sender, RoutedEventArgs e)
        {
            imageCaptcha.Source = App.MainWnd.bfClient.getVerifyCaptcha(App.MainWnd.samplecaptcha);
            t_Code.Text = "";
        }
    }
}
