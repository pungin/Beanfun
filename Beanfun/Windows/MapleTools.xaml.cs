using System.IO;
using System.Windows;

namespace Beanfun
{
    /// <summary>
    /// MapleTools.xaml 的交互逻辑
    /// </summary>
    public partial class MapleTools : Window
    {
        public MapleTools()
        {
            InitializeComponent();
        }

        private void Window_MouseLeftButtonDown(object sender, System.Windows.Input.MouseButtonEventArgs e)
        {
            this.DragMove();
        }

        private void btn_PlayerReport_Click(object sender, RoutedEventArgs e)
        {
            if (App.LoginRegion == "HK") MessageBox.Show(TryFindResource("MsgPlayerReport") as string);
            //new WebBrowser("https://event.beanfun.com/customerservice/PluginReporting/PluginBoard/PluginBoardJQ.aspx").Show();
            new WebBrowser("https://event.beanfun.com/customerservice/PluginReporting/PlayerReport.aspx").Show();
        }

        private void btn_VideoReport_Click(object sender, RoutedEventArgs e)
        {
            new WebBrowser("https://event.beanfun.com/MapleStory/eventad/EventAD.aspx?EventADID=3453").Show();
        }

        private void btn_EquipCalculator_Click(object sender, RoutedEventArgs e)
        {
            new EquipCalculator().Show();
        }

        private void btn_CoreCaculator_Click(object sender, RoutedEventArgs e)
        {
            new CoreCalculator().Show();
        }

        private void btn_Recycling_Click(object sender, RoutedEventArgs e)
        {
            MessageBoxResult result = MessageBox.Show(TryFindResource("MsgRecycling") as string, "", MessageBoxButton.YesNo);

            if (result != MessageBoxResult.Yes) return;

            DirectoryInfo gameDir = new DirectoryInfo(Path.GetDirectoryName(App.MainWnd.settingPage.t_GamePath.Text));

            string[] dirList = new string[] {
                "blob_storage",
                "GPUCache",
                "VideoDecodeStats",
                "XignCode",
            };

            foreach (string dir in dirList)
            {
                if (!Directory.Exists($"{ gameDir.FullName }\\{ dir }")) continue;
                try
                {
                    Directory.Delete($"{ gameDir.FullName }\\{ dir }", true);
                }
                catch { }
            }

            // 清理更新失敗的緩存
            foreach (DirectoryInfo di in gameDir.GetDirectories())
            {
                try
                {
                    if (di.Name.EndsWith(".$$$")) di.Delete(true);
                }
                catch { }
            }

            // 清理報錯的檔案和多餘dll
            foreach (FileInfo fi in gameDir.GetFiles())
            {
                try
                {
                    if (fi.Name.ToLower().EndsWith(".dmp") || fi.Name.ToLower().Equals("localeemulator.dll") || fi.Name.ToLower().Equals("loaderdll.dll")) fi.Delete();
                } catch { }
            }

            MessageBox.Show(TryFindResource("MsgRecyclingDone") as string);
        }
    }
}
