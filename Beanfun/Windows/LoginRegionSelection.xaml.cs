using System.Windows;
using System.Windows.Input;

namespace Beanfun
{
    /// <summary>
    /// LoginRegionSelection.xaml 的交互逻辑
    /// </summary>
    public partial class LoginRegionSelection : Window
    {
        public LoginRegionSelection()
        {
            InitializeComponent();
        }

        private void Window_MouseLeftButtonDown(object sender, MouseButtonEventArgs e)
        {
            this.DragMove();
        }

        private void ButtonTW_Click(object sender, RoutedEventArgs e)
        {
            ConfigAppSettings.SetValue("loginRegion", "TW");
            this.Hide();
            App.MainWnd.Initialize();
        }

        private void ButtonHK_Click(object sender, RoutedEventArgs e)
        {
            ConfigAppSettings.SetValue("loginRegion", "HK");
            this.Hide();
            App.MainWnd.Initialize();
        }

        private void Window_Closed(object sender, System.EventArgs e)
        {
            App.Current.Shutdown();
        }
    }
}
