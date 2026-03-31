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

        private async void btn_OpenGamePass_Click(object sender, RoutedEventArgs e)
        {
            btn_OpenGamePass.IsEnabled = false;
            try
            {
                var client = new BeanfunClient();
                string skey = await System.Threading.Tasks.Task.Run(() => client.GetSessionkey());
                if (string.IsNullOrEmpty(skey))
                {
                    MessageBox.Show(
                        Application.Current.TryFindResource("SessionKeyFailed") as string
                    );
                    return;
                }
                App.MainWnd.bfClient = client;
                new GamePassBrowser(skey).Show();
            }
            catch
            {
                MessageBox.Show(Application.Current.TryFindResource("ConnectionFailed") as string);
            }
            finally
            {
                btn_OpenGamePass.IsEnabled = true;
            }
        }

        private void btn_back_Click(object sender, RoutedEventArgs e)
        {
            App.LoginMethod = (int)LoginMethod.Regular;
            App.MainWnd.loginMethodChanged();
        }
    }
}
