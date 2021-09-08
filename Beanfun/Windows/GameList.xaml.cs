using Amemiya.Net;
using System.IO;
using System.Net;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
using System.Windows.Media.Imaging;

namespace Beanfun
{
    /// <summary>
    /// GameList.xaml 的交互逻辑
    /// </summary>
    public partial class GameList : Window
    {
        public class Game
        {
            public Image image { get; set; }
            public string name { get; set; }
            public string service_code { get; set; }
            public string service_region { get; set; }

            public Game(BitmapImage source, string name, string service_code, string service_region)
            {
                this.image = new Image();
                image.Source = source;
                this.name = name;
                this.service_code = service_code;
                this.service_region = service_region;
            }
        }

        public GameList()
        {
            InitializeComponent();

            string baseUrl = App.LoginRegion == "TW" ? "https://tw.images.beanfun.com/uploaded_images/beanfun_tw/game_zone/" : "http://hk.images.beanfun.com/uploaded_images/beanfun/game_zone/";
            WebClient wc = new WebClientEx();
            foreach (MainWindow.GameService game in App.MainWnd.gameList)
            {
                byte[] buffer = wc.DownloadData(baseUrl + game.large_image_name);
                BitmapImage large_image = new BitmapImage();
                large_image.BeginInit();
                large_image.StreamSource = new MemoryStream(buffer);
                large_image.EndInit();
                l_GameList.Items.Add(new Game(large_image, game.name, game.service_code, game.service_region));
            }
        }

        private void Window_MouseLeftButtonDown(object sender, MouseButtonEventArgs e)
        {
            this.DragMove();
        }

        private void l_GameList_SelectionChanged(object sender, SelectionChangedEventArgs e)
        {
            if (l_GameList.SelectedIndex < 0)
                return;
            if (App.MainWnd.service_code != ((Game)l_GameList.SelectedItem).service_code || App.MainWnd.service_region != ((Game)l_GameList.SelectedItem).service_region)
            {
                App.MainWnd.service_code = ((Game)l_GameList.SelectedItem).service_code;
                App.MainWnd.service_region = ((Game)l_GameList.SelectedItem).service_region;
                App.MainWnd.selectedGameChanged();
            }
            this.Close();
        }
    }
}
