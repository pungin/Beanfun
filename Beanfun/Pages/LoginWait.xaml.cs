using System.Windows;
using System.Windows.Controls;

namespace Beanfun
{
    /// <summary>
    /// LoginWait.xaml 的交互逻辑
    /// </summary>
    public partial class LoginWait : Page
    {
        public LoginWait()
        {
            InitializeComponent();
        }

        private void Button_Click(object sender, RoutedEventArgs e)
        {
            App.MainWnd.loginWorker.CancelAsync();
            App.MainWnd.totpWorker.CancelAsync();
            App.MainWnd.bfAPPAutoLogin.IsEnabled = false;
            t_Info.Content = TryFindResource("MsgLogging") as string;
            if (App.LoginMethod == (int)LoginMethod.QRCode) App.MainWnd.loginMethodChanged();
            App.MainWnd.return_page = App.MainWnd.loginPage;
        }
    }
}
