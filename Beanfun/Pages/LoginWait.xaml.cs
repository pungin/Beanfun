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
            App.MainWnd.bfAPPAutoLogin.IsEnabled = false;
            t_Info.Content = "正在登入,請稍等...";
            if (App.LoginMethod == (int)LoginMethod.QRCode) App.MainWnd.loginMethodChanged();
            App.MainWnd.return_page = App.MainWnd.loginPage;
        }
    }
}
