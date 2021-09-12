using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Data;
using System.Windows.Documents;
using System.Windows.Input;
using System.Windows.Media;
using System.Windows.Media.Imaging;
using System.Windows.Navigation;
using System.Windows.Shapes;

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
                MessageBox.Show("驗證資訊不能為空。");
                return;
            }
            if (this.t_Code.Text.Length <= 0)
            {
                MessageBox.Show("圖形驗證碼不能為空。");
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
