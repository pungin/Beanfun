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
                    MessageBox.Show(
                        Application.Current.TryFindResource("SessionKeyFailed") as string
                    );
                    return;
                }
                App.MainWnd.bfClient = client;
                new WebBrowser(
                    $"https://login.beanfun.com/Login/GoGamaPassRequest?pSKey={skey}"
                ).Show();
            }
            catch
            {
                MessageBox.Show(Application.Current.TryFindResource("ConnectionFailed") as string);
            }
        }

        private void btn_back_Click(object sender, RoutedEventArgs e)
        {
            App.LoginMethod = (int)LoginMethod.Regular;
            App.MainWnd.loginMethodChanged();
        }
    }
}
