using System.Windows;
using System.Windows.Controls;
using System.Windows.Media;

namespace Beanfun
{
    /// <summary>
    /// About.xaml 的交互逻辑
    /// </summary>
    public partial class About : Page
    {
        public About()
        {
            InitializeComponent();
            initThemeColor(App.MainWnd.isLightColor());
        }

        public void initThemeColor(bool isLightMode)
        {
            Brush brush = new SolidColorBrush((Color)ColorConverter.ConvertFromString(isLightMode ? "Black" : "White"));
            t_AppName.Foreground = brush;
            t_Author.Foreground = brush;
            t_Version.Foreground = brush;
            version.Foreground = brush;
        }

        private void Donate_Click(object sender, RoutedEventArgs e)
        {
            Donate wnd = new Donate();
            wnd.ShowDialog();
        }

        private void Button_Click(object sender, RoutedEventArgs e)
        {
            if (App.MainWnd == null) return;
            if (App.MainWnd.return_page == null) App.MainWnd.return_page = App.MainWnd.loginPage;
            App.MainWnd.frame.Content = App.MainWnd.return_page;
            App.MainWnd.return_page = null;
        }

        private void UpdateCheck_Click(object sender, RoutedEventArgs e)
        {
            App.MainWnd.CheckUpdates(true);
        }

        private void MailContact_Click(object sender, RoutedEventArgs e)
        {
            string to = "pungin@msn.com ";
            string subject = "繽放 反饋/建議";
            string body = $"軟體版本: {version.Text}%0d反饋/建議訊息:%0d";
            System.Diagnostics.Process.Start($"mailto:{to}?subject={subject}&body={body}");
        }

        private void Github_Click(object sender, RoutedEventArgs e)
        {
            System.Diagnostics.Process.Start("https://github.com/pungin/Beanfun/issues/new");
        }
    }
}
