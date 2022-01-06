using System;
using System.Windows;
using System.Windows.Input;
using System.Windows.Media;

namespace Beanfun
{
    /// <summary>
    /// ServiceAccountInfo.xaml 的交互逻辑
    /// </summary>
    public partial class ServiceAccountInfo : Window
    {
        public ServiceAccountInfo(BeanfunClient.ServiceAccount account)
        {
            InitializeComponent();
            t_sn.Text = account.ssn;
            t_sname.Text = account.sname;
            t_id.Text = account.sid;
            t_status.Content = account.isEnable ? TryFindResource("Normal") : TryFindResource("Banned");
            t_status.Foreground = new SolidColorBrush((Color)ColorConverter.ConvertFromString(account.isEnable ? "Green" : "Red"));
            if (account.sauthtype == null)
            {
                p_sauthtype.Visibility = Visibility.Collapsed;
            }
            else
            {
                t_sauthtype.Text = account.sauthtype;
            }
            if (account.screatetime == null)
            {
                p_screatetime.Visibility = Visibility.Collapsed;
            }
            else
            {
                t_screatetime.Content = string.Format(TryFindResource("CreateDate") as string, account.screatetime);
                t_screatedays.Content = getDays(account.screatetime);
            }
            if (account.slastusedtime == null)
            {
                p_slastusedtime.Visibility = Visibility.Collapsed;
            }
            else
            {
                t_slastusedtime.Content = string.Format(TryFindResource("LastLoginDate") as string, account.slastusedtime);
            }
        }

        private void Window_MouseLeftButtonDown(object sender, MouseButtonEventArgs e)
        {
            this.DragMove();
        }

        private string getDays(string time)
        {
            DateTime start = Convert.ToDateTime(time);
            DateTime end = Convert.ToDateTime(DateTime.Now);
            TimeSpan sp = end.Subtract(start);
            return Convert.ToString(sp.Days);
        }
    }
}
