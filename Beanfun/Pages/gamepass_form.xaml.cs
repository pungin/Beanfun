using System.Windows;
using System.Windows.Controls;

namespace Beanfun
{
    /// <summary>
    /// gamepass_form.xaml 的交互逻辑
    /// </summary>
    public partial class gamepass_form : Page
    {
        public gamepass_form()
        {
            InitializeComponent();
        }

        private void btn_OpenGamePass_Click(object sender, RoutedEventArgs e)
        {
            try
            {
                var client = new BeanfunClient();
                string skey = client.GetSessionkey();
                if (string.IsNullOrEmpty(skey))
                {
                    MessageBox.Show("無法取得 SessionKey，請稍後再試");
                    return;
                }
                App.MainWnd.bfClient = client;
                new WebBrowser($"https://login.beanfun.com/Login/GoGamaPassRequest?pSKey={skey}").Show();
            }
            catch
            {
                MessageBox.Show("連線失敗，請檢查網路連線");
            }
        }

        private void btn_back_Click(object sender, RoutedEventArgs e)
        {
            App.LoginMethod = (int)LoginMethod.Regular;
            App.MainWnd.loginMethodChanged();
        }
    }
}
