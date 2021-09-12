using System.Collections.Generic;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;

namespace Beanfun
{
    /// <summary>
    /// LoginPage.xaml 的交互逻辑
    /// </summary>
    public partial class LoginPage : Page
    {
        public id_pass_form id_pass = new id_pass_form();
        public qr_form qr = new qr_form();

        public LoginPage()
        {
            InitializeComponent();
        }
    }
}
