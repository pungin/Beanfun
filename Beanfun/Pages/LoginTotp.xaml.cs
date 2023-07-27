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
    /// LoginTotp.xaml 的交互逻辑
    /// </summary>
    public partial class LoginTotp : Page
    {
        public LoginTotp()
        {
            InitializeComponent();
        }

        private void btn_login_Click(object sender, RoutedEventArgs e)
        {
            btn_login.IsEnabled = false;
            btn_cancel.IsEnabled = false;
            App.MainWnd.do_Totp();
        }

        private void btn_back_Click(object sender, RoutedEventArgs e)
        {
            App.MainWnd.frame.Content = App.MainWnd.loginPage;
        }

        private void otp1_PreviewKeyUp(object sender, KeyEventArgs e)
        {
            if(otp1.Text.Length > 0)
            {
                otp2.Focus();
            }
        }

        private void otp2_PreviewKeyUp(object sender, KeyEventArgs e)
        {
            if (otp2.Text.Length > 0)
            {
                otp3.Focus();
            }
        }

        private void otp3_PreviewKeyUp(object sender, KeyEventArgs e)
        {
            if (otp3.Text.Length > 0)
            {
                otp4.Focus();
            }
        }

        private void otp4_PreviewKeyUp(object sender, KeyEventArgs e)
        {
            if (otp4.Text.Length > 0)
            {
                otp5.Focus();
            }
        }

        private void otp5_PreviewKeyUp(object sender, KeyEventArgs e)
        {
            if (otp5.Text.Length > 0)
            {
                otp6.Focus();
            }
        }

        private void otp6_PreviewKeyUp(object sender, KeyEventArgs e)
        {
            if (otp1.Text.Length > 0 && otp2.Text.Length > 0 && otp3.Text.Length > 0 && otp4.Text.Length > 0 && otp5.Text.Length > 0 && otp6.Text.Length > 0)
            {
                btn_login.RaiseEvent(new RoutedEventArgs(Button.ClickEvent));
            }
        }

        private void otp_GotFocus(object sender, RoutedEventArgs e)
        {
            TextBox box = sender as TextBox;
            box.SelectAll();
        }
    }
}
