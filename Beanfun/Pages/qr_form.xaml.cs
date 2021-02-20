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
    /// qr_form.xaml 的交互逻辑
    /// </summary>
    public partial class qr_form : Page
    {
        public qr_form()
        {
            InitializeComponent();

            useNewQRCode.IsChecked = bool.Parse(ConfigAppSettings.GetValue("useNewQRCode", "true"));
        }

        private void Button_Click(object sender, RoutedEventArgs e)
        {
            new GameList().ShowDialog();
        }

        private void btn_Refresh_QRCode_Click(object sender, RoutedEventArgs e)
        {
            App.MainWnd.refreshQRCode();
        }

        private void useNewQRCode_CheckedChanged(object sender, RoutedEventArgs e)
        {
            if (App.MainWnd == null || App.MainWnd.loginPage == null || App.MainWnd.loginPage.qr == null || useNewQRCode.IsChecked == bool.Parse(ConfigAppSettings.GetValue("useNewQRCode", "true")))
                return;
            ConfigAppSettings.SetValue("useNewQRCode", Convert.ToString((bool)useNewQRCode.IsChecked));

            App.MainWnd.refreshQRCode();
        }
    }
}
