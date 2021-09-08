using System.Windows;

namespace Beanfun
{
    /// <summary>
    /// KartTools.xaml 的交互逻辑
    /// </summary>
    public partial class KartTools : Window
    {
        public KartTools()
        {
            InitializeComponent();
        }

        private void Window_MouseLeftButtonDown(object sender, System.Windows.Input.MouseButtonEventArgs e)
        {
            this.DragMove();
        }

        private void btn_KartManageData_Click(object sender, RoutedEventArgs e)
        {
            new WebBrowser("https://tw.beanfun.com/KartRider/guild/maneger_data.aspx").Show();
        }

        private void btn_KartRank_Click(object sender, RoutedEventArgs e)
        {
            new WebBrowser("https://tw.beanfun.com/kartrider/guild/rank.aspx").Show();
        }

        private void btn_KartCreate_Click(object sender, RoutedEventArgs e)
        {
            new WebBrowser("https://tw.beanfun.com/KartRider/guild/create.aspx").Show();
        }

        private void btn_KartRank_TeamIn_Click(object sender, RoutedEventArgs e)
        {
            new WebBrowser("https://tw.beanfun.com/KartRider/guild/rank_team_in.aspx").Show();
        }

        private void btn_KartSearchMember_Click(object sender, RoutedEventArgs e)
        {
            new WebBrowser("https://tw.beanfun.com/KartRider/guild/search_member.aspx").Show();
        }

        private void btn_KartLeaveGuildMember_Click(object sender, RoutedEventArgs e)
        {
            new WebBrowser("https://tw.beanfun.com/KartRider/guild/leave_guild_Member.aspx").Show();
        }
    }
}
