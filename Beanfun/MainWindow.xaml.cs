using Amemiya.Net;
using IniParser;
using IniParser.Model;
using Microsoft.Win32;
using Newtonsoft.Json.Linq;
using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.Diagnostics;
using System.IO;
using System.Linq;
using System.Net;
using System.Text;
using System.Text.RegularExpressions;
using System.Threading;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
using System.Windows.Media;
using System.Windows.Media.Imaging;
using Utility.ModifyRegistry;

namespace Beanfun
{
    enum LoginMethod : int
    {
        Regular = 0,
        QRCode = 1
    };
    enum GameStartMode : int
    {
        Auto = 0,
        Normal = 1,
        LocaleEmulator = 2
    };
    /// <summary>
    /// MainWindow.xaml 的交互逻辑
    /// </summary>
    public partial class MainWindow : Window
    {
        public LoginPage loginPage;
        public ManagerAccount manageAccPage;
        public LoginWait loginWaitPage = new LoginWait();
        public AccountList accountList = null;
        public VerifyPage verifyPage;
        public Settings settingPage;
        public About aboutPage;

        public System.ComponentModel.BackgroundWorker getOtpWorker;
        public System.ComponentModel.BackgroundWorker loginWorker;
        public System.ComponentModel.BackgroundWorker pingWorker;
        public System.ComponentModel.BackgroundWorker qrWorker;
        public System.ComponentModel.BackgroundWorker verifyWorker;
        public System.Windows.Threading.DispatcherTimer qrCheckLogin;
        public System.Windows.Threading.DispatcherTimer checkPlayPage;
        public System.Windows.Threading.DispatcherTimer checkPatcher;
        public System.Windows.Threading.DispatcherTimer bfAPPAutoLogin;

        public AccountManager accountManager = null;

        public BeanfunClient bfClient;

        public BeanfunClient.QRCodeClass qrcodeClass;
        
        public string LastLoginAccountID = "";
        public string service_code = "610074", service_region = "T9";
        public string game_exe = "MapleStory.exe";
        public string dir_value_name = "Path";
        public string win_class_name = "MapleStoryClass";
        public short login_action_type = 1;
        public string game_commandLine = "tw.login.maplestory.gamania.com 8484 BeanFun %s %s";
        private string otp;
        private BitmapImage qr_default;
        private static readonly System.Windows.Forms.NotifyIcon _trayNotifyIcon = new System.Windows.Forms.NotifyIcon { Icon = Properties.Resources.icon };

        private Version currentVersion = System.Reflection.Assembly.GetExecutingAssembly().GetName().Version;

        public List<GameService> gameList = new List<GameService>();
        public GameService SelectedGame = null;
        public bool UnconnectedGame = false;

        public string viewstate, eventvalidation, samplecaptcha;

        public Page return_page = null;
        public IniData INIData = null;

        public MainWindow()
        {
            InitializeComponent();

            if (!App.IsWin10) SourceChord.FluentWPF.AcrylicWindow.SetTintOpacity(this, 1.0);

            this.getOtpWorker = new System.ComponentModel.BackgroundWorker();
            this.loginWorker = new System.ComponentModel.BackgroundWorker();
            this.pingWorker = new System.ComponentModel.BackgroundWorker();
            this.qrWorker = new System.ComponentModel.BackgroundWorker();
            this.verifyWorker = new System.ComponentModel.BackgroundWorker();
            this.qrCheckLogin = new System.Windows.Threading.DispatcherTimer();
            this.checkPlayPage = new System.Windows.Threading.DispatcherTimer();
            this.checkPatcher = new System.Windows.Threading.DispatcherTimer();
            this.bfAPPAutoLogin = new System.Windows.Threading.DispatcherTimer();
            // 
            // getOtpWorker
            // 
            this.getOtpWorker.WorkerReportsProgress = true;
            this.getOtpWorker.WorkerSupportsCancellation = true;
            this.getOtpWorker.DoWork += this.getOtpWorker_DoWork;
            this.getOtpWorker.RunWorkerCompleted += this.getOtpWorker_RunWorkerCompleted;
            // 
            // loginWorker
            // 
            this.loginWorker.WorkerReportsProgress = true;
            this.loginWorker.WorkerSupportsCancellation = true;
            this.loginWorker.DoWork += this.loginWorker_DoWork;
            this.loginWorker.RunWorkerCompleted += this.loginWorker_RunWorkerCompleted;
            // 
            // pingWorker
            // 
            this.pingWorker.WorkerReportsProgress = true;
            this.pingWorker.WorkerSupportsCancellation = true;
            this.pingWorker.DoWork += this.pingWorker_DoWork;
            this.pingWorker.RunWorkerCompleted += this.pingWorker_RunWorkerCompleted;
            // 
            // qrWorker
            // 
            this.qrWorker.WorkerReportsProgress = true;
            this.qrWorker.WorkerSupportsCancellation = true;
            this.qrWorker.DoWork += this.qrWorker_DoWork;
            this.qrWorker.RunWorkerCompleted += this.qrWorker_RunWorkerCompleted;
            // 
            // verifyWorker
            // 
            this.verifyWorker.WorkerReportsProgress = true;
            this.verifyWorker.WorkerSupportsCancellation = true;
            this.verifyWorker.DoWork += this.verifyWorker_DoWork;
            this.verifyWorker.RunWorkerCompleted += this.verifyWorker_RunWorkerCompleted;
            // 
            // qrCheckLogin
            // 
            this.qrCheckLogin.Interval = TimeSpan.FromSeconds(2);
            this.qrCheckLogin.Tick += this.qrCheckLogin_Tick;
            // 
            // checkPlayPage
            // 
            this.checkPlayPage.Interval = TimeSpan.FromMilliseconds(100);
            this.checkPlayPage.Tick += this.checkPlayPage_Tick;
            // 
            // checkPatcher
            // 
            this.checkPatcher.Interval = TimeSpan.FromMilliseconds(100);
            this.checkPatcher.Tick += this.checkPatcher_Tick;
            // 
            // bfAPPAutoLogin
            // 
            this.bfAPPAutoLogin.Interval = TimeSpan.FromSeconds(2);
            this.bfAPPAutoLogin.Tick += this.bfAPPAutoLogin_Tick;

            loginPage = new LoginPage();
            manageAccPage = new ManagerAccount();
            verifyPage = new VerifyPage();
            accountList = new AccountList();
            settingPage = new Settings();
            aboutPage = new About();

            Initialize();

            changeThemeColor((Color)ColorConverter.ConvertFromString(ConfigAppSettings.GetValue("ThemeColor", "#B6DE8E")));
        }

        public void changeThemeColor(Color color)
        {
            bool oldIsLightColor = isLightColor();
            SourceChord.FluentWPF.AcrylicWindow.SetTintColor(this, color);
            color.R = (byte)Math.Max(color.R - 50, 0);
            color.G = (byte)Math.Max(color.G - 50, 0);
            color.B = (byte)Math.Max(color.B - 50, 0);
            color.A = 0xFF;
            this.BorderBrush = new SolidColorBrush(color);
            bool isLightMode = isLightColor();
            if (oldIsLightColor != isLightMode)
            {
                btn_About_MouseLeave(null, null);
                btn_Setting_MouseLeave(null, null);
                btn_Min_MouseLeave(null, null);
                btn_Close_MouseLeave(null, null);
                BitmapImage logo = new BitmapImage(new Uri("pack://application:,,,/Resources/logo" + (isLightMode ? "" : "_darkmode") + ".png"));
                if (loginPage != null) loginPage.Logo.Source = logo;
                if (verifyPage != null) verifyPage.Logo.Source = logo;
                if (accountList != null) accountList.Logo.Source = logo;
                if (aboutPage != null) aboutPage.initThemeColor(isLightMode);
            }
        }
        
        public bool isLightColor()
        {
            Color color = SourceChord.FluentWPF.AcrylicWindow.GetTintColor(this);
            return (0.299 * color.R + 0.587 * color.G + 0.114 * color.B) / 255 > 0.5;
        }

        public Color getTitleButtonColor()
        {
            return (Color)ColorConverter.ConvertFromString(isLightColor() ? "Black" : "White");
        }

        public void Initialize()
        {
            try
            {
                App.LoginRegion = loginPage.Beanfun_TW.IsEnabled ? "HK" : "TW";
                if (loginPage.Beanfun_TW.IsEnabled)
                {
                    ServicePointManager.SecurityProtocol &= ~SecurityProtocolType.Ssl3;
                }
                else
                {
                    ServicePointManager.SecurityProtocol |= SecurityProtocolType.Ssl3;
                }
                if (settingPage.tradLogin != null && !(bool)settingPage.tradLogin.IsChecked)
                    accountList.panel_GetOtp.Visibility = Visibility.Collapsed;

                aboutPage.version.Text = $"{currentVersion.Major}.{currentVersion.Minor}.{currentVersion.Build}({currentVersion.Revision})";

                qr_default = new BitmapImage();
                qr_default.BeginInit();
                qr_default.UriSource = new Uri("pack://application:,,,/Resources/refresh.png");
                qr_default.EndInit();
                loginPage.qr.qr_image.Source = qr_default;

                string loginGame = ConfigAppSettings.GetValue("loginGame", "");
                if (loginGame != "")
                {
                    string[] arr = loginGame.Split('_');
                    if (arr != null && arr.Length > 1)
                    {
                        service_code = arr[0];
                        service_region = arr[1];
                    }
                }

                if ((bool)settingPage.ask_update.IsChecked) CheckUpdates(false);

                this.accountManager = new AccountManager();

                bool res = accountManager.init();
                if (res == false)
                    errexit("帳號記錄初始化失敗，未知的錯誤。", 0);

                loginPage.ddlAuthType.SelectionChanged += this.ddlAuthType_SelectionChanged;
                settingPage.t_GamePath.PreviewMouseLeftButtonDown += this.btn_SetGamePath_Click;
                LastLoginAccountID = ConfigAppSettings.GetValue("AccountID", LastLoginAccountID);
                int loginMethod = accountManager.getMethodByAccount(App.LoginRegion, LastLoginAccountID);
                if (loginMethod < (int)LoginMethod.Regular)
                    loginMethod = int.Parse(ConfigAppSettings.GetValue("loginMethod", "0"));
                if (loginMethod > (int)LoginMethod.QRCode)
                    loginMethod = (int)LoginMethod.QRCode;

                ddlAuthTypeItemsInit();
                reLoadGameInfo();

                loginPage.ddlAuthType.SelectedIndex = loginMethod;

                _trayNotifyIcon.MouseClick += (sender, e) =>
                {
                    if (e.Button == System.Windows.Forms.MouseButtons.Left)
                    {
                        if (this.Visibility == Visibility.Visible)
                        {
                            this.Visibility = Visibility.Hidden;
                        }
                        else
                        {
                            this.Visibility = Visibility.Visible;
                            this.Activate();
                        }

                        _trayNotifyIcon.Visible = false;
                    }
                };

                frame.Content = loginPage;

                if (loginPage.ddlAuthType.SelectedIndex == (int)LoginMethod.Regular && (bool)loginPage.id_pass.checkBox_AutoLogin.IsChecked)
                {
                    do_Login();
                }
            }
            catch (Exception ex)
            {
                MessageBox.Show("從樂豆官網加載訊息出錯，錯誤訊息：" + ex.Message + "\r\n\r\n\r\n可能有如下原因：\r\n1.樂豆網站暫時維修中無法訪問\r\n2.本機無法直接與樂豆連線\r\n3.如果有用遊戲代理然後瀏覽器能訪問樂豆網站，那麼可能就是代理沒有對本應用進行代理"/* + "\r\n\r\n" + ex.StackTrace*/);

                new LoginRegionSelection().ShowDialog();
            }
        }

        public class GameService
        {
            public string name { get; set; }
            public string service_code { get; set; }
            public string service_region { get; set; }
            public string website_url { get; set; }
            public string xlarge_image_name { get; set; }
            public string large_image_name { get; set; }
            public string small_image_name { get; set; }
            public string download_url { get; set; }

            public GameService(string name, string service_code, string service_region, string website_url, string xlarge_image_name, string large_image_name, string small_image_name, string download_url)
            {
                this.name = name;
                this.service_code = service_code;
                this.service_region = service_region;
                this.website_url = website_url;
                this.xlarge_image_name = xlarge_image_name;
                this.large_image_name = large_image_name;
                this.small_image_name = small_image_name;
                this.download_url = download_url;
            }
        }

        public void selectedGameChanged()
        {
            string gameCode = service_code + "_" + service_region;
            ConfigAppSettings.SetValue("loginGame", gameCode);

            if (INIData == null)
            {
                reLoadGameInfo();
                return;
            }
            string exe = INIData[gameCode]["exe"];
            if (exe == null)
            {
                new GameList().ShowDialog();
                return;
            }
            Regex regex = new Regex("(.*).exe");
            if (regex.IsMatch(exe))
                game_exe = regex.Match(exe).Groups[1].Value + ".exe";
            else
                game_exe = "";
            regex = new Regex(".exe (.*)");
            if (regex.IsMatch(exe))
                game_commandLine = regex.Match(exe).Groups[1].Value;
            else
                game_commandLine = "";

            login_action_type = 8;
            string sLoginActionType = INIData[gameCode]["login_action_type"];
            if (sLoginActionType != "")
                login_action_type = short.Parse(sLoginActionType);
            if (login_action_type == 1)
            {
                settingPage.tradLogin.Visibility = Visibility.Visible;
                if ((bool)settingPage.tradLogin.IsChecked)
                    accountList.panel_GetOtp.Visibility = Visibility.Visible;
                else
                    accountList.panel_GetOtp.Visibility = Visibility.Collapsed;
            }
            else
            {
                settingPage.tradLogin.Visibility = Visibility.Collapsed;
                accountList.panel_GetOtp.Visibility = Visibility.Visible;
            }

            win_class_name = INIData[gameCode]["win_class_name"];
            if ("MapleStoryClass".Equals(win_class_name))
            {
                accountList.autoPaste.Visibility = Visibility.Visible;
            }
            else
            {
                accountList.autoPaste.Visibility = Visibility.Collapsed;
            }
            dir_value_name = INIData[gameCode]["dir_value_name"];
            if (ConfigAppSettings.GetValue(dir_value_name + "." + gameCode, "") == "")
            {
                string dir_reg = INIData[gameCode]["dir_reg"];
                if (dir_reg != "")
                {
                    dir_reg = dir_reg.Replace("HKEY_LOCAL_MACHINE\\", "");

                    try
                    {
                        ModifyRegistry myRegistry = new ModifyRegistry();
                        myRegistry.BaseRegistryKey = Registry.CurrentUser;
                        myRegistry.SubKey = dir_reg;
                        if (myRegistry.Read(dir_value_name) != "")
                        {
                            ConfigAppSettings.SetValue(dir_value_name + "." + gameCode, myRegistry.Read(dir_value_name));
                            settingPage.t_GamePath.Text = myRegistry.Read(dir_value_name);
                        }
                    }
                    catch
                    {
                        settingPage.t_GamePath.Text = "";
                    }
                }
            }
            else
            {
                settingPage.t_GamePath.Text = ConfigAppSettings.GetValue(dir_value_name + "." + gameCode);
            }

            if (gameCode == "610074_T9")
            {
                settingPage.skipPlayWnd.Visibility = Visibility.Visible;

                if ((bool)settingPage.skipPlayWnd.IsChecked)
                    checkPlayPage.IsEnabled = true;

                settingPage.autoKillPatcher.Visibility = Visibility.Visible;

                if ((bool)settingPage.autoKillPatcher.IsChecked)
                    checkPatcher.IsEnabled = true;
            }
            else
            {
                settingPage.skipPlayWnd.Visibility = Visibility.Collapsed;
                checkPlayPage.IsEnabled = false;
                settingPage.autoKillPatcher.Visibility = Visibility.Collapsed;
                checkPatcher.IsEnabled = false;
            }

            if (this.bfClient != null && !loginWorker.IsBusy && !getOtpWorker.IsBusy)
            {
                if (App.LoginRegion == "TW")
                    this.bfClient.GetAccounts(service_code, service_region);
                else
                    this.bfClient.GetAccounts_HK(service_code, service_region);
                redrawSAccountList();
                if (this.bfClient.errmsg != null)
                {
                    errexit(this.bfClient.errmsg, 2);
                    this.bfClient.errmsg = null;
                }
            }
            switch (gameCode)
            {
                case "610153_TN":
                case "610085_TC":
                    UnconnectedGame = true;
                    break;
                default:
                    UnconnectedGame = false;
                    break;
            }

           WebClient wc = new WebClientEx();

            try
            {
                if (loginPage != null)
                {
                    foreach (GameService gs in gameList)
                    {
                        if (gs.service_region == this.service_region && gs.service_code == this.service_code)
                        {
                            BitmapImage large_image;
                            BitmapImage small_image;
                            try
                            {

                                string baseUrl = App.LoginRegion == "TW" ? "https://tw.images.beanfun.com/uploaded_images/beanfun_tw/game_zone/" : "http://hk.images.beanfun.com/uploaded_images/beanfun/game_zone/";

                                byte[] buffer = wc.DownloadData(baseUrl + gs.large_image_name);
                                large_image = new BitmapImage();
                                large_image.BeginInit();
                                large_image.StreamSource = new MemoryStream(buffer);
                                large_image.EndInit();

                                buffer = wc.DownloadData(baseUrl + gs.small_image_name);
                                small_image = new BitmapImage();
                                small_image.BeginInit();
                                small_image.StreamSource = new MemoryStream(buffer);
                                small_image.EndInit();
                            }
                            catch (Exception)
                            {
                                large_image = null;
                                small_image = null;
                            }
                            loginPage.id_pass.imageGame.Source = large_image;
                            loginPage.qr.gameName.Content = gs.name;
                            accountList.imageGame.Source = small_image;
                            accountList.gameName.Content = gs.name;
                            SelectedGame = gs;
                            break;
                        }
                    }
                }
            }
            catch { /* ignore out of range */ }
        }

        public void reLoadGameInfo()
        {
            WebClient wc = new WebClientEx();

            string res = Encoding.UTF8.GetString(wc.DownloadData("https://" + App.LoginRegion.ToLower() + ".beanfun.com/beanfun_block/generic_handlers/get_service_ini.ashx"));

            StringIniParser sip = new StringIniParser();
            INIData = sip.ParseString(res);

            res = Encoding.UTF8.GetString(wc.DownloadData("https://" + App.LoginRegion.ToLower() + ".beanfun.com/game_zone/"));
            Regex reg = new Regex("Services.ServiceList = (.*);");
            if (reg.IsMatch(res))
            {
                gameList.Clear();
                string json = reg.Match(res).Groups[1].Value;
                JObject o = JObject.Parse(json);
                foreach (var game in o["Rows"])
                {
                    GameService gs = new GameService((string)game["ServiceFamilyName"], (string)game["ServiceCode"], (string)game["ServiceRegion"], (string)game["ServiceWebsiteURL"], (string)game["ServiceXLargeImageName"], (string)game["ServiceLargeImageName"], (string)game["ServiceSmallImageName"], (string)game["ServiceDownloadURL"]);
                    gameList.Add(gs);
                    if (gs.service_code == service_code && gs.service_region == service_region)
                        SelectedGame = gs;
                }
            }

            selectedGameChanged();
        }

        public void CheckUpdates(bool show)
        {
            Update.ApplicationUpdater.CheckApplicationUpdate(currentVersion, show);
        }

        private string reLoadVerifyPage(string response)
        {
            Regex regex = new Regex("id=\"__VIEWSTATE\" value=\"(.*)\"");
            if (!regex.IsMatch(response))
            { return "VerifyNoViewstate"; }
            this.viewstate = regex.Match(response).Groups[1].Value;
            regex = new Regex("id=\"__EVENTVALIDATION\" value=\"(.*)\"");
            if (!regex.IsMatch(response))
            { return "VerifyNoEventvalidation"; }
            this.eventvalidation = regex.Match(response).Groups[1].Value;
            regex = new Regex("id=\"LBD_VCID_c_logincheck_advancecheck_samplecaptcha\" value=\"(.*)\"");
            if (!regex.IsMatch(response))
            { return "VerifyNoSamplecaptcha"; }
            this.samplecaptcha = regex.Match(response).Groups[1].Value;
            /*regex = new Regex("\\<span id=\"lblVerify\"\\>(.*)\\<\\/span\\>");
            if (!regex.IsMatch(response))
            { return "VerifyNoLblVerify"; }
            verifyPage.t_Verify.MaskText = regex.Match(response).Groups[1].Value;*/
            regex = new Regex("\\<span id=\"lblAuthType\"\\>(.*)\\<\\/span\\>");
            if (!regex.IsMatch(response))
            { return "VerifyNoLblAuthType"; }
            verifyPage.labelAuthType.Content = regex.Match(response).Groups[1].Value;
            regex = new Regex("alert\\('(.*)'\\);");
            if (regex.IsMatch(response))
            { return regex.Match(response).Groups[1].Value; }
            verifyPage.imageCaptcha.Source = this.bfClient.getVerifyCaptcha(this.samplecaptcha);
            return null;
        }

        private void Window_MouseLeftButtonDown(object sender, MouseButtonEventArgs e)
        {
            try
            {
                this.DragMove();
            } catch {}
        }

        private void Window_Activated(object sender, EventArgs e)
        {
            btn_About_MouseLeave(null, null);
            btn_Setting_MouseLeave(null, null);
            btn_Min_MouseLeave(null, null);
            btn_Close_MouseLeave(null, null);
            if (this.IsActive)
            {
                Color color = SourceChord.FluentWPF.AcrylicWindow.GetTintColor(this);
                color.R = (byte)Math.Max(color.R - 50, 0);
                color.G = (byte)Math.Max(color.G - 50, 0);
                color.B = (byte)Math.Max(color.B - 50, 0);
                color.A = 0xFF;
                this.BorderBrush = new SolidColorBrush(color);
            }
            else
            {
                this.BorderBrush = new SolidColorBrush((Color)ColorConverter.ConvertFromString("Gray"));
            }
        }

        private void Window_StateChanged(object sender, EventArgs e)
        {
            if (settingPage != null && settingPage.minimize_to_tray != null && (bool)settingPage.minimize_to_tray.IsChecked && this.WindowState == WindowState.Minimized)
            {
                this.Visibility = Visibility.Hidden;
                _trayNotifyIcon.Visible = true;
            }
        }

        private void btn_About_MouseLeave(object sender, MouseEventArgs e)
        {
            if (this.IsActive)
                btn_About.Foreground = new SolidColorBrush(getTitleButtonColor());
            else
                btn_About.Foreground = new SolidColorBrush((Color)ColorConverter.ConvertFromString("Gray"));
        }

        private void btn_About_IsKeyboardFocusedChanged(object sender, DependencyPropertyChangedEventArgs e)
        {
            if (btn_About.IsKeyboardFocused) frame.Focus();
        }

        private void btn_Setting_MouseLeave(object sender, MouseEventArgs e)
        {
            if (this.IsActive)
                btn_Setting.Foreground = new SolidColorBrush(getTitleButtonColor());
            else
                btn_Setting.Foreground = new SolidColorBrush((Color)ColorConverter.ConvertFromString("Gray"));
        }

        private void btn_Setting_IsKeyboardFocusedChanged(object sender, DependencyPropertyChangedEventArgs e)
        {
            if (btn_Setting.IsKeyboardFocused) frame.Focus();
        }

        private void btn_Min_MouseLeave(object sender, MouseEventArgs e)
        {
            if (this.IsActive)
                btn_Min.Foreground = new SolidColorBrush(getTitleButtonColor());
            else
                btn_Min.Foreground = new SolidColorBrush((Color)ColorConverter.ConvertFromString("Gray"));
        }

        private void btn_Min_IsKeyboardFocusedChanged(object sender, DependencyPropertyChangedEventArgs e)
        {
            if (btn_Min.IsKeyboardFocused) frame.Focus();
        }

        private void btn_Close_MouseEnter(object sender, MouseEventArgs e)
        {
            btn_Close.Foreground = new SolidColorBrush((Color)ColorConverter.ConvertFromString("White"));
        }

        private void btn_Close_MouseLeave(object sender, MouseEventArgs e)
        {
            if (this.IsActive)
                btn_Close.Foreground = new SolidColorBrush(getTitleButtonColor());
            else
                btn_Close.Foreground = new SolidColorBrush((Color)ColorConverter.ConvertFromString("Gray"));
        }

        private void btn_Close_IsKeyboardFocusedChanged(object sender, DependencyPropertyChangedEventArgs e)
        {
            if (btn_Close.IsKeyboardFocused) frame.Focus();
        }

        private void btn_About_Click(object sender, RoutedEventArgs e)
        {
            frame.Content = aboutPage;
            if (return_page != null) return;
            return_page = (Page)frame.Content;
        }

        private void btn_Setting_Click(object sender, RoutedEventArgs e)
        {
            frame.Content = settingPage;
            if (return_page != null) return;
            return_page = (Page)frame.Content;
        }

        private void btn_Min_Click(object sender, RoutedEventArgs e)
        {
            base.WindowState = WindowState.Minimized;
        }

        private void btn_Close_Click(object sender, RoutedEventArgs e)
        {
            System.Windows.Application.Current.Shutdown();
        }

        private void btn_SetGamePath_Click(object sender, RoutedEventArgs e)
        {
            string gameCode = service_code + "_" + service_region;
            OpenFileDialog openFileDialog = new OpenFileDialog();
            openFileDialog.Filter = accountList.gameName.Content + "主程式|" + game_exe + "|All files (*.*)|*.*";
            openFileDialog.Title = "請選擇 " + game_exe + " 檔案";

            if (openFileDialog.ShowDialog() == true)
            {
                string file = openFileDialog.FileName;
                ConfigAppSettings.SetValue(dir_value_name + "." + gameCode, file);
                settingPage.t_GamePath.Text = file;
            }
        }

        public void ddlAuthType_SelectionChanged(object sender, SelectionChangedEventArgs e)
        {
            qrCheckLogin.IsEnabled = false;
            
            if (App.LoginRegion == "TW")
            {
                switch (loginPage.ddlAuthType.SelectedIndex)
                {
                    case (int)Beanfun.LoginMethod.QRCode:
                        loginPage.ddlAuthType.IsEnabled = false;
                        loginPage.qr.qr_image.Source = qr_default;
                        loginPage.login_form.Content = loginPage.qr;
                        qrWorker.RunWorkerAsync(loginPage == null || loginPage.qr == null || loginPage.qr.useNewQRCode == null || (bool)loginPage.qr.useNewQRCode.IsChecked ? false : true);
                        break;
                    default:
                        loginPage.login_form.Content = loginPage.id_pass;
                        if (loginPage.ddlAuthType.SelectedIndex == (int)Beanfun.LoginMethod.Regular)
                        {
                            loginPage.id_pass.lb_pwd.Text = "密碼";
                        }
                        else
                        {
                            loginPage.id_pass.lb_pwd.Text = "PIN碼";
                        }
                        break;
                }
            }
            else
            {
                loginPage.login_form.Content = loginPage.id_pass;
                loginPage.ddlAuthType.SelectedIndex = (int)Beanfun.LoginMethod.Regular;
                loginPage.id_pass.lb_pwd.Text = "密碼";
            }

            if (loginPage.ddlAuthType.SelectedIndex == (int)Beanfun.LoginMethod.Regular && (loginPage.id_pass.t_Password.Password == "" || loginPage.id_pass.t_Password.Password == null))
            {
                string pwd = accountManager.getPasswordByAccount(App.LoginRegion, loginPage.id_pass.t_AccountID.Text);

                if (pwd != null && pwd != "")
                {
                    loginPage.id_pass.t_Password.Password = pwd;
                    loginPage.id_pass.checkBox_RememberPWD.IsChecked = true;
                    loginPage.id_pass.checkBox_AutoLogin.IsChecked = accountManager.getAutoLoginByAccount(App.LoginRegion, loginPage.id_pass.t_AccountID.Text);
                }

                string verify = accountManager.getVerifyByAccount(App.LoginRegion, loginPage.id_pass.t_AccountID.Text);
                if (verify != null && verify != "")
                {
                    verifyPage.t_Verify.Text = verify;
                    verifyPage.checkBoxRememberVerify.IsChecked = true;
                }
                else
                {
                    verifyPage.t_Verify.Text = "";
                    verifyPage.checkBoxRememberVerify.IsChecked = false;
                }
            }
        }

        public void ddlAuthTypeItemsInit()
        {
            try
            {
                loginPage.ddlAuthType.Items.Clear();
                if (App.LoginRegion == "TW")
                {
                    foreach (string type in loginPage.item_TW)
                    {
                        loginPage.ddlAuthType.Items.Add(type);
                    }

                    accountList.btn_Deposite.Visibility = Visibility.Visible;
                }
                else
                {
                    foreach (string type in loginPage.item_HK)
                    {
                        loginPage.ddlAuthType.Items.Add(type);
                    }

                    accountList.btn_Deposite.Visibility = Visibility.Collapsed;
                }
            } catch { }

            try
            {
                string accId = LastLoginAccountID;
                int selectedIndex = -1;
                string[] accountArrays = accountManager.getAccountList(App.LoginRegion);
                List<string> accList = new List<string>();

                int i = 0;
                foreach (string s in accountArrays)
                {
                    if (s == accId) selectedIndex = i;
                    string name = accountManager.getNameByAccount(App.LoginRegion, s);
                    if (name != null && name != "")
                    {
                        accList.Add(name + "(" + s + ")");
                    }
                    else
                    {
                        accList.Add(s);
                    }
                    i++;
                }
                loginPage.id_pass.t_AccountID.ItemsSource = null;
                loginPage.id_pass.t_AccountID.ItemsSource = accList;

                if (accountManager.getAccountList().Length > 0)
                    loginPage.id_pass.ManageAcc.Visibility = Visibility.Visible;
                else
                    loginPage.id_pass.ManageAcc.Visibility = Visibility.Collapsed;

                int loginMethod = accountManager.getMethodByAccount(App.LoginRegion, accId);
                if (loginMethod < (int)LoginMethod.Regular)
                {
                    if (accountArrays.Length > 0)
                    { accId = accList[0]; selectedIndex = 0; }
                    loginMethod = accountManager.getMethodByAccount(App.LoginRegion, accId);
                }

                if (loginMethod > -1)
                {
                    loginPage.id_pass.t_AccountID.SelectedIndex = selectedIndex;

                    loginPage.ddlAuthType.SelectedIndex = loginMethod;

                    string pwd = accountManager.getPasswordByAccount(App.LoginRegion, accId);
                    if (loginMethod != (int)LoginMethod.Regular)
                        pwd = "";

                    if (pwd == null || pwd == "")
                    {
                        loginPage.id_pass.t_Password.Password = "";
                        loginPage.id_pass.checkBox_RememberPWD.IsChecked = false;
                        loginPage.id_pass.checkBox_AutoLogin.IsChecked = false;
                    }
                }
                else
                {
                    loginPage.id_pass.t_AccountID.Text = "";
                    loginPage.id_pass.t_Password.Password = "";
                    loginPage.id_pass.checkBox_RememberPWD.IsChecked = false;
                    loginPage.id_pass.checkBox_AutoLogin.IsChecked = false;

                    loginPage.ddlAuthType.SelectedIndex = (int)LoginMethod.Regular;

                    verifyPage.t_Verify.Text = "";
                    verifyPage.checkBoxRememberVerify.IsChecked = false;
                }
            }
            catch { /* ignore out of range */ }
            manageAccPage.setupAccList(this);
        }

        public void do_Login()
        {
            frame.Content = loginWaitPage;
            this.loginWorker.RunWorkerAsync(loginPage.ddlAuthType.SelectedIndex);
        }

        public bool errexit(string msg, int method, string title = null)
        {
            string originalMsg = msg;

            switch (msg)
            {
                case "LoginNoResponse":
                    msg = "初始化失敗，請檢查網路連線。";
                    method = 0;
                    break;
                case "LoginNoSkey":
                    msg = "獲取Skey失敗。";
                    method = 0;
                    break;
                case "LoginNoOTP1":
                    msg = "獲取OTP1失敗。";
                    method = 0;
                    break;
                case "LoginNoSeed":
                    msg = "獲取Seed失敗。";
                    method = 0;
                    break;
                case "LoginNoHash":
                    msg = "獲取QRcode失敗。";
                    method = 0;
                    break;
                case "LoginIntResultError":
                    msg = "獲取QRcode失敗，返回的初始化訊息不正確。";
                    method = 0;
                    break;
                case "AKeyParseFailed":
                    msg = "獲取AKey失敗。";
                    method = 0;
                    break;
                case "authkeyParseFailed":
                    msg = "獲取authkey失敗。";
                    method = 0;
                    break;
                case "LoginJsonParseFailed":
                    msg = "偵測登入結果失敗，未找到返回的Json訊息。";
                    break;
                case "LoginNoViewstate":
                    msg = "登入失敗，未找到Viewstate訊息，請檢查網絡連線。";
                    break;
                case "LoginNoEventvalidation":
                    msg = "登入失敗，未找到Eventvalidation訊息，請檢查網絡連線。";
                    break;
                case "LoginNoViewstateGenerator":
                    msg = "登入失敗，未找到ViewstateGenerator訊息，請檢查網絡連線。";
                    break;
                case "LoginNoSamplecaptcha":
                    msg = "登入失敗，未找到Samplecaptcha訊息，請檢查網絡連線。";
                    break;
                case "LoginNoAkey":
                case "LoginNoProcessLoginV2JSON":
                    msg = "登入失敗，帳號或密碼錯誤。(" + msg + ")";
                    break;
                case "LoginNoAccountMatch":
                case "LoginGetAccountErr":
                case "LoginUpdateAccountListErr":
                case "LoginAuthErr":
                    msg = "登入失敗，無法取得帳號列表。(" + msg + ")";
                    break;
                case "LoginNoWebtoken":
                    msg = "登入失敗，登入後無法取得bfWebToken Cookie。";
                    break;
                case "LoginUnknown":
                    msg = "登入失敗，請稍後再試";
                    method = 0;
                    break;
                case "BFServiceXNotFound":
                    MessageBoxResult result = MessageBox.Show("調用或初始化BFService元件失敗，有如下可能：\n1.未安裝BFService元件\n2.BFService元件默認會安裝到「文檔」資料夾，請確認真實路徑是否為與當前語係的字元集支援的文字並且能正常訪問\n\n是否前往下載元件？", "", MessageBoxButton.YesNo);

                    if (result == MessageBoxResult.Yes)
                        Process.Start("http://hk.download.beanfun.com/beanfun20/beanfun_2_0_93_170_hk.exe");

                    frame.Content = loginPage;
                    return false;
                case "LoginNoMethod":
                    msg = "登入出錯，選擇了不存在的登入方式。";
                    break;
                case "OTPUnknown":
                    msg = "獲取密碼失敗，請嘗試重新登入。";
                    break;
                case "OTPNoCreateTime":
                    msg = "獲取帳號創建時間失敗。";
                    break;
                case "OTPNoSecretCode":
                    msg = "獲取密碼失敗，未找到SecretCode訊息，請檢查網絡連線。";
                    break;
                case "OTPNoMyAccountData":
                    msg = "獲取密碼失敗，未找到MyAccountData訊息，請檢查網絡連線。";
                    break;
                case "DecryptOTPError":
                    msg = "解密密碼失敗。";
                    break;
                case "MainAccount_Not_Exist":
                    msg = "此Beanfun帳號不存在，請確認您的Beanfun帳號是否成功註冊，或者確認帳號所在區域為" + (App.LoginRegion == "TW" ? "台灣" : "香港") + "的Beanfun帳號。";
                    break;
                case "LoginInitCaptcha":
                    msg = "載入圖形驗證碼失敗。";
                    break;
                default:
                    if (msg.StartsWith("OTPNoLongPollingKey:"))
                    {
                        msg = msg.Replace("OTPNoLongPollingKey:", "");
                        if (msg == "") msg = "獲取密碼時初始化失敗，請檢查網路連線。";
                        else if (msg.Contains("很抱歉，需先完成進階認證")) msg = "很抱歉，需先完成進階認證，才可啟動此款遊戲";
                        else if (msg.Contains("尚未登入，請重新登入") || msg.Contains("無法認證登入狀態")) { msg = "已從伺服器斷線，請重新登入"; method = 1; };
                    }
                    break;
            }

            MessageBox.Show(msg, title);
            if (method == 0)
                App.Current.Shutdown();
            else if (method == 1)
            {
                ddlAuthType_SelectionChanged(null, null);
                accountList.t_Password.Text = "";
                frame.Content = loginPage;
            }

            return false;
        }

        // Login do work.
        private void loginWorker_DoWork(object sender, DoWorkEventArgs e)
        {
            if (this.pingWorker.IsBusy) this.pingWorker.CancelAsync();
            //while (this.pingWorker.IsBusy) Thread.Sleep(137);
            Console.WriteLine("loginWorker starting");
            Thread.CurrentThread.Name = "Login Worker";
            e.Result = "";
            try
            {
                loginPage.Dispatcher.Invoke(
                    new Action(
                        delegate
                        {
                            if (loginPage.ddlAuthType.SelectedIndex != (int)LoginMethod.QRCode)
                                this.bfClient = new BeanfunClient();
                            this.bfClient.Login(loginPage.id_pass.t_AccountID.Text, loginPage.id_pass.t_Password.Password, loginPage.ddlAuthType.SelectedIndex, this.qrcodeClass, this.service_code, this.service_region);
                        }
                    )
                );
                if (this.bfClient.errmsg != null)
                    e.Result = this.bfClient.errmsg;
                else
                    e.Result = null;
            }
            catch (Exception ex)
            {
                e.Result = "登入失敗，未知的錯誤。\n\n" + ex.Message + "\n" + ex.StackTrace;
            }
        }

        // Login completed.
        private void loginWorker_RunWorkerCompleted(object sender, RunWorkerCompletedEventArgs e)
        {
            Console.WriteLine("loginWorker end");
            if (e != null && e.Error != null)
            {
                errexit(e.Error.Message, 1);
                frame.Content = loginPage;
                return;
            }
            if (e != null && (string)e.Result != null)
            {
                if ((string)e.Result == "LoginAdvanceCheck")
                {
                    MessageBox.Show("為確保您的帳號安全，需請您協助進行資料驗證，以利保障您的權益。");

                    // Handle panel switching.
                    frame.Content = verifyPage;
                    if ((bool)verifyPage.checkBoxRememberVerify.IsChecked)
                        verifyPage.t_Code.Focus();
                    else
                        verifyPage.t_Verify.Focus();
                    verifyPage.t_Verify.Text = accountManager.getVerifyByAccount(App.LoginRegion, loginPage.id_pass.t_AccountID.Text);
                    verifyPage.checkBoxRememberVerify.IsChecked = verifyPage.t_Verify.Text != null && verifyPage.t_Verify.Text != "";
                    verifyPage.t_Code.Text = "";
                    string response = this.bfClient.getVerifyPageInfo();
                    if (response == null)
                    { MessageBox.Show(this.bfClient.errmsg); frame.Content = loginPage; }
                    string errmsg = reLoadVerifyPage(response);
                    if (errmsg != null)
                    { MessageBox.Show(errmsg); frame.Content = loginPage; }
                }
                else if (((string)e.Result).StartsWith("bfAPPAutoLogin.ashx"))
                {
                    string[] args = Regex.Split((string)e.Result, "\",\"");
                    if (args.Length < 2)
                    {
                        errexit("LoginUnknown", 1);
                        return;
                    }
                    loginWaitPage.t_Info.Content = "此帳號需透過beanfun! 遊戲授權登入。\r\n請使用beanfun! 遊戲授權登入。";
                    bfAPPAutoLogin.IsEnabled = true;
                    beanfunApp.Main(new string[] { args[1] });
                }
                else
                {
                    errexit((string)e.Result, 1);
                }
                return;
            }
            
            ConfigAppSettings.SetValue("loginMethod", loginPage.ddlAuthType.SelectedIndex.ToString());
            if (App.LoginRegion != "TW" || loginPage.ddlAuthType.SelectedIndex != (int)LoginMethod.QRCode)
            {
                LastLoginAccountID = loginPage.id_pass.t_AccountID.Text;
                ConfigAppSettings.SetValue("AccountID", LastLoginAccountID);
                accountManager.addAccount(
                    App.LoginRegion,
                    loginPage.id_pass.t_AccountID.Text,
                    "",
                    loginPage.id_pass.checkBox_RememberPWD.IsEnabled && (bool)loginPage.id_pass.checkBox_RememberPWD.IsChecked ? loginPage.id_pass.t_Password.Password : "",
                    (bool)verifyPage.checkBoxRememberVerify.IsChecked ? verifyPage.t_Verify.Text : "",
                    loginPage.ddlAuthType.SelectedIndex,
                    (bool)loginPage.id_pass.checkBox_AutoLogin.IsChecked
                );

                ddlAuthTypeItemsInit();
            }
            else ConfigAppSettings.SetValue("AccountID", null);

            try
            {
                frame.Content = accountList;

                redrawSAccountList();

                if (!this.pingWorker.IsBusy) this.pingWorker.RunWorkerAsync();

                updateRemainPoint(this.bfClient.remainPoint);

                accountList.list_Account.Focus();
                if ((bool)settingPage.autoStartGame.IsChecked && this.bfClient.accountList.Count() > 0)
                {
                    if (((bool)settingPage.tradLogin.IsChecked && login_action_type == 1) || login_action_type == 0)
                        runGame();
                    accountList.btnGetOtp_Click(null, null);
                }
            }
            catch
            {
                errexit("登入失敗，無法取得帳號列表。", 1);
            }
        }

        private void redrawSAccountList()
        {
            if (this.bfClient.accountAmountLimitNotice != "")
            {
                accountList.lbl_AccountAmountLimitNotice.Content = this.bfClient.accountAmountLimitNotice;
                accountList.accLimit.Visibility = Visibility.Visible;
                int accLimit;
                try{ accLimit = int.Parse(this.bfClient.accountAmountLimitNotice.Substring(this.bfClient.accountAmountLimitNotice.Length - 1, 1)); }
                catch { accLimit = -1; }
                if (accLimit == -1)
                {
                    accountList.btnAddServiceAccount.Content = "前往認證";
                    accountList.btnAddServiceAccount.IsEnabled = true;
                    accountList.btnAddServiceAccount.Visibility = Visibility.Visible;
                }
                else
                {
                    accountList.btnAddServiceAccount.Content = "新增帳號";
                    accountList.btnAddServiceAccount.IsEnabled = this.bfClient.accountList.Count < accLimit;
                    accountList.btnAddServiceAccount.Visibility = this.bfClient.accountList.Count < accLimit ? Visibility.Visible : Visibility.Hidden;
                }
            }
            else
            {
                accountList.lbl_AccountAmountLimitNotice.Content = "";
                accountList.accLimit.Visibility = Visibility.Collapsed;
            }

            accountList.list_Account.ItemsSource = null;
            accountList.list_Account.ItemsSource = this.bfClient.accountList;

            string gameCode = service_code + "_" + service_region;
            Visibility visable = App.LoginRegion == "TW" ? Visibility.Visible : Visibility.Collapsed;
            if (accountList.list_Account.Items.Count > 0)
            {
                accountList.list_Account.SelectedIndex = 0;
                
                accountList.m_CopyAccount.Visibility = Visibility.Visible;
                accountList.m_ChangeAccName.Visibility = !UnconnectedGame || App.LoginRegion != "TW" ? Visibility.Visible : Visibility.Collapsed;
                accountList.m_ChangePassword.Visibility = UnconnectedGame ? Visibility.Visible : Visibility.Collapsed;
                accountList.m_AccInfo.Visibility = !UnconnectedGame || App.LoginRegion != "TW" ? Visibility.Visible : Visibility.Collapsed;
                accountList.s_Account.Visibility = Visibility.Visible;
            }
            else
            {
                accountList.m_CopyAccount.Visibility = Visibility.Collapsed;
                accountList.m_ChangeAccName.Visibility = Visibility.Collapsed;
                accountList.m_ChangePassword.Visibility = Visibility.Collapsed;
                accountList.m_AccInfo.Visibility = Visibility.Collapsed;
                accountList.s_Account.Visibility = Visibility.Collapsed;
            }
            accountList.m_GetEmail.Visibility = visable;

            if (gameCode == "610074_T9" || gameCode == "610096_TE")
                accountList.btn_Tools.Visibility = Visibility.Visible;
            else
                accountList.btn_Tools.Visibility = Visibility.Collapsed;
        }

        public void updateRemainPoint(int remainPoint)
        {
            accountList.m_RemainPoint.Header = $"樂豆: { remainPoint }{(App.LoginRegion == "TW" || remainPoint == 0 ? "" : $" (遊戲內 { Math.Floor(remainPoint / 2.5) })")} 點";
        }

        public void runGame(string account = null, string password = null)
        {
            string gameCode = service_code + "_" + service_region;
            string gamePath = settingPage.t_GamePath.Text;
            if (gamePath == "" || !File.Exists(gamePath))
            {
                MessageBoxResult result = MessageBox.Show("無法正確偵測遊戲安裝狀態。請按一下<是>來重新偵測。若未安裝遊戲，請按一下<否>開始下載。", "", MessageBoxButton.YesNo);
                if (result == MessageBoxResult.Yes || SelectedGame == null)
                {
                    btn_SetGamePath_Click(null, null);
                }
                else
                {
                    Process.Start(SelectedGame.download_url);
                }
                return;
            }
            gamePath = settingPage.t_GamePath.Text;
            if (gamePath == "" || !File.Exists(gamePath))
            {
                return;
            }
            bool findGame = false;

            Regex regexx = new Regex("(.*).exe");
            string gameProcessName = "";
            if (regexx.IsMatch(game_exe))
                gameProcessName = regexx.Match(game_exe).Groups[1].Value;
            if (gameProcessName != "")
            {
                foreach (Process process in Process.GetProcessesByName(gameProcessName))
                {
                    try
                    {
                        if (process.MainModule.FileName == gamePath)
                        { findGame = true; break; }
                    }
                    catch { }
                }
            }

            if (findGame)
            {
                MessageBoxResult result = MessageBox.Show("遊戲已經運行,可能是客戶端問題導致未完全結束程序,是否要結束遊戲?", "", MessageBoxButton.YesNo);
                if (result == MessageBoxResult.Yes)
                {
                    foreach (Process process in Process.GetProcessesByName(gameProcessName))
                    {
                        try
                        {
                            if (process.MainModule.FileName == gamePath)
                                process.Kill();
                        }
                        catch { }
                    }
                }
            }
            try
            {
                Console.WriteLine("try open game");
                int runMode = int.Parse(ConfigAppSettings.GetValue("startGameMode", "0"));
                if (runMode == (int)GameStartMode.Auto)
                {
                    switch (WindowsAPI.GetSystemDefaultLocaleName())
                    {
                        case "zh-Hant":
                        case "zh-CHT":
                        case "zh-TW":
                        case "zh-HK":
                        case "zh-MO":
                            runMode = (int)GameStartMode.Normal;
                            break;
                        default:
                            runMode = (int)GameStartMode.LocaleEmulator;
                            break;
                    }
                }

                if (runMode > (int)GameStartMode.LocaleEmulator) runMode = (int)GameStartMode.LocaleEmulator;

                string commandLine = "";
                if (account != null && password != null && account != "" && password != "" && game_commandLine != "")
                {
                    commandLine = game_commandLine;
                    Regex regex = new Regex("%s");
                    commandLine = regex.Replace(commandLine, account, 1);
                    commandLine = regex.Replace(commandLine, password, 1);
                }

                switch (runMode)
                {
                    case (int)GameStartMode.LocaleEmulator:
                        OperatingSystem os = System.Environment.OSVersion;
                        if (os.Platform == PlatformID.Win32NT && os.Version.Major < 6 && runMode != (int)GameStartMode.Normal)
                        {
                            errexit("以非繁體語係系統啟動遊戲的方式不支援Windows XP。", 2);
                            return;
                        }
                        startByLE(gamePath, commandLine);
                        break;
                    case (int)GameStartMode.Normal:
                        ProcessStartInfo startInfo = new ProcessStartInfo(gamePath);
                        startInfo.WorkingDirectory = Path.GetDirectoryName(gamePath);
                        startInfo.Arguments = commandLine;
                        Process.Start(startInfo);
                        break;
                }
                Console.WriteLine("try open game done");
            }
            catch
            {
                errexit("啟動失敗，請嘗試從桌面捷徑直接啟動遊戲。\r\n\r\n若您系統為非繁體語係系統，可能是Locale Emulator元件不支援您的系統或是遊戲自動更新導致遊戲損壞了(可以嘗試重新安裝遊戲)。", 2);
            }
        }

        private void releaseResource(byte[] data, string path)
        {
            if (!File.Exists(path))
            {
                FileStream fsObj = new FileStream(path, FileMode.CreateNew);
                fsObj.Write(data, 0, data.Length);
                fsObj.Close();
            }
        }

        private void startByLE(string path, string command)
        {
            if (!File.Exists(string.Format("{0}\\LocaleEmulator.dll", System.Environment.CurrentDirectory)) || !File.Exists(string.Format("{0}\\LoaderDll.dll", System.Environment.CurrentDirectory)))
            {
                /*
                MessageBoxResult result = MessageBox.Show("程式檢測到您當前運行的系統非繁體語係，若要在非繁體語係之系統下運行遊戲則需要在登入器目錄下釋出額外檔案，是否確認需要釋出額外檔案？", "", MessageBoxButton.YesNo);
                if (result == MessageBoxResult.No) return;
                */
                releaseResource(global::Beanfun.Properties.Resources.LocaleEmulator, string.Format("{0}\\LocaleEmulator.dll", System.Environment.CurrentDirectory));
                releaseResource(global::Beanfun.Properties.Resources.LoaderDll, string.Format("{0}\\LoaderDll.dll", System.Environment.CurrentDirectory));
            }

            int CHINESEBIG5_CHARSET = 136;
            var commandLine = string.Empty;
            commandLine = path.StartsWith("\"")
                ? $"{path} "
                : $"\"{path}\" ";
            commandLine += command;
            System.Globalization.TextInfo culInfo = System.Globalization.CultureInfo.GetCultureInfo("zh-HK").TextInfo;

            var l = new LEProc.LoaderWrapper
            {
                ApplicationName = path,
                CommandLine = commandLine,
                CurrentDirectory = Path.GetDirectoryName(path),
                AnsiCodePage = (uint)culInfo.ANSICodePage,
                OemCodePage = (uint)culInfo.OEMCodePage,
                LocaleID = (uint)culInfo.LCID,
                DefaultCharset = (uint) CHINESEBIG5_CHARSET,
                HookUILanguageAPI = (uint)0,
                Timezone = "China Standard Time",
                NumberOfRegistryRedirectionEntries = 0,
                DebugMode = false
            };

            uint ret;
            if ((ret = l.Start()) != 0)
            {
                if (ret == 0xC00700C1)
                {
                    errexit($"非繁體語係系統啟動遊戲失敗\r\n"
                                + $"錯誤碼: {Convert.ToString(ret, 16).ToUpper()}\r\n"
                                + "導致這個錯誤的原因可能是因為使用了遊戲的自動更新，導致遊戲損壞了。\r\n"
                                + "\r\n解決方案:\r\n"
                                + "- 如果官方有fix更新檔請下載下來手動更新一下遊戲\r\n"
                                + "- 如果官方沒有fix更新檔或上面方法無法解決請嘗試重新安裝遊戲\r\n"
                    , 2);
                }
                else
                {
                    errexit($"非繁體語係系統啟動遊戲失敗\r\n"
                                + $"錯誤碼: {Convert.ToString(ret, 16).ToUpper()}\r\n"
                                + $"{string.Format($"{Environment.OSVersion} {(Is64BitOS() ? "x64" : "x86")}", Environment.OSVersion, Is64BitOS() ? "x64" : "x86")}\r\n"
                                + $"{GenerateSystemDllVersionList()}\r\n"
                                + "如果你有運行任何防毒軟體, 請關閉後再次嘗試。\r\n"
                                + "如果仍然顯示此視窗, 請嘗試以「安全模式」啟動程式。"
                                + "如果你進行了以上的嘗試仍然沒有一個有效，請隨時在後面的連結提交問題\r\n"
                                + "https://github.com/xupefei/Locale-Emulator/issues\r\n" + "\r\n" + "\r\n"
                                + "你可以按 CTRL+C 將此訊息複製到你的剪貼板。\r\n"
                    , 2);
                }
            }
        }

        public static bool Is64BitOS()
        {
            if (IntPtr.Size == 8) // 64-bit programs run only on Win64
            {
                return true;
            }
            // Detect whether the current process is a 32-bit process 
            // running on a 64-bit system.
            bool flag;
            return DoesWin32MethodExist("kernel32.dll", "IsWow64Process") &&
                   WindowsAPI.IsWow64Process(WindowsAPI.GetCurrentProcess(), out flag) && flag;
        }

        private static bool DoesWin32MethodExist(string moduleName, string methodName)
        {
            var moduleHandle = WindowsAPI.GetModuleHandle(moduleName);
            if (moduleHandle == IntPtr.Zero)
            {
                return false;
            }
            return WindowsAPI.GetProcAddress(moduleHandle, methodName) != IntPtr.Zero;
        }

        private static string GenerateSystemDllVersionList()
        {
            string[] dlls = { "NTDLL.DLL", "KERNELBASE.DLL", "KERNEL32.DLL", "USER32.DLL", "GDI32.DLL" };

            var result = new StringBuilder();

            foreach (var dll in dlls)
            {
                var version = FileVersionInfo.GetVersionInfo(
                                                             Path.Combine(
                                                                          Path.GetPathRoot(Environment.SystemDirectory),
                                                                          Is64BitOS()
                                                                              ? @"Windows\SysWOW64\"
                                                                              : @"Windows\System32\",
                                                                          dll));

                result.Append(dll);
                result.Append(": ");
                result.Append(
                              $"{version.FileMajorPart}.{version.FileMinorPart}.{version.FileBuildPart}.{version.FilePrivatePart}");
                result.Append("\r\n");
            }

            return result.ToString();
        }

        public bool AddServiceAccount(string name)
        {
            if (this.bfClient == null)
                return false;
            if (name == null || name == "")
                return false;

            if (this.bfClient.AddServiceAccount(name, service_code, service_region))
            {
                if (App.LoginRegion == "TW") this.bfClient.GetAccounts(service_code, service_region);
                else this.bfClient.GetAccounts_HK(service_code, service_region);
                int index = accountList.list_Account.SelectedIndex;
                redrawSAccountList();
                accountList.list_Account.SelectedIndex = index;
                return true;
            }
            return false;
        }

        public string UnconnectedGame_AddAccount(string name, string txtNewPwd, string txtNewPwd2, string txtServiceAccountDN, System.Collections.Specialized.NameValueCollection payload)
        {
            if (this.bfClient == null)
                return null;
            if (name == null || name == "")
                return null;
            if (txtNewPwd == null || txtNewPwd == "")
                return null;
            if (txtNewPwd2 == null || txtNewPwd2 == "")
                return null;

            string result = this.bfClient.UnconnectedGame_AddAccount(service_code, service_region, name, txtNewPwd, txtNewPwd2, txtServiceAccountDN, payload);
            if (result == "")
            {
                if (App.LoginRegion == "TW") this.bfClient.GetAccounts(service_code, service_region);
                else this.bfClient.GetAccounts_HK(service_code, service_region);
                int index = accountList.list_Account.SelectedIndex;
                redrawSAccountList();
                accountList.list_Account.SelectedIndex = index;
            }
            return result;
        }

        public string UnconnectedGame_ChangePassword(string txtEmail)
        {
            if (this.bfClient == null)
                return null;
            if (txtEmail == null)
                return null;

            return this.bfClient.UnconnectedGame_ChangePassword(service_code, service_region, accountList.list_Account.SelectedIndex, txtEmail);
        }

        public System.Collections.Specialized.NameValueCollection UnconnectedGame_AddAccountInit()
        {
            if (this.bfClient == null)
                return null;
            return this.bfClient.UnconnectedGame_InitAddAccountPayload(service_code, service_region);
        }

        public System.Collections.Specialized.NameValueCollection UnconnectedGame_AddUnconnectedCheck(string name, string txtServiceAccountDN, System.Collections.Specialized.NameValueCollection payload)
        {
            if (this.bfClient == null)
                return null;
            return this.bfClient.UnconnectedGame_AddAccountCheck(service_code, service_region, name, txtServiceAccountDN, payload);
        }

        public System.Collections.Specialized.NameValueCollection UnconnectedGame_AddAccountCheckNickName(string txtServiceAccountDN, System.Collections.Specialized.NameValueCollection payload)
        {
            if (this.bfClient == null)
                return null;
            return this.bfClient.UnconnectedGame_AddAccountCheckNickName(service_code, service_region, txtServiceAccountDN, payload);
        }

        public bool ChangeServiceAccountDisplayName(string newName)
        {
            if (this.bfClient == null)
                return false;
            BeanfunClient.ServiceAccount account = (BeanfunClient.ServiceAccount)accountList.list_Account.SelectedItem;
            if (newName == null || newName == "" || account == null)
                return false;
            if (newName == account.sname)
                return true;

            string gameCode = service_code + "_" + service_region;
            if (this.bfClient.ChangeServiceAccountDisplayName(newName, gameCode, account))
            {
                account.sname = newName;
                int index = accountList.list_Account.SelectedIndex;
                redrawSAccountList();
                accountList.list_Account.SelectedIndex = index;
                return true;
            }
            return false;
        }

        public string GetServiceContract()
        {
            if (this.bfClient == null)
                return "";

            return this.bfClient.GetServiceContract(service_code, service_region);
        }

        // getOTP do work.
        private void getOtpWorker_DoWork(object sender, DoWorkEventArgs e)
        {
            if (this.pingWorker.IsBusy) this.pingWorker.CancelAsync();
            //while (this.pingWorker.IsBusy) Thread.Sleep(133);
            Console.WriteLine("getOtpWorker start");
            Thread.CurrentThread.Name = "GetOTP Worker";
            int index = (int)e.Argument;
            Console.WriteLine("Count = " + this.bfClient.accountList.Count + " | index = " + index);
            if (this.bfClient.accountList.Count <= index)
            {
                return;
            }
            Console.WriteLine("call GetOTP");
            this.otp = this.bfClient.GetOTP(this.bfClient.accountList[index], this.service_code, this.service_region);
            Console.WriteLine("call GetOTP done");
            if (this.otp == null)
                e.Result = -1;
            else
            {
                e.Result = index;
            }

            return;
        }

        // getOTP completed.
        private void getOtpWorker_RunWorkerCompleted(object sender, RunWorkerCompletedEventArgs e)
        {
            accountList.btnGetOtp.Content = "獲取密碼";
            if (e.Error != null)
            {
                errexit(e.Error.Message, 2, "獲取密碼失敗");
            }
            else
            {
                int index = (int)e.Result;

                if (index == -1)
                {
                    errexit(this.bfClient.errmsg, 2, "獲取密碼失敗");
                }
                else
                {
                    int accIndex = accountList.list_Account.SelectedIndex;
                    string acc = this.bfClient.accountList[index].sid;
                    accountList.t_Password.Text = this.otp;

                    if ((!(bool)settingPage.tradLogin.IsChecked && login_action_type == 1))
                    {
                        runGame(acc, accountList.t_Password.Text);
                    }
                    else
                    {
                        IntPtr hWnd = WindowsAPI.FindWindow(win_class_name, null);
                        double dpixRatio = 1.0;
                        if (hWnd != IntPtr.Zero)
                        {
                            System.Drawing.Graphics currentGraphics = System.Drawing.Graphics.FromHwnd(hWnd);
                            dpixRatio = currentGraphics.DpiX / 96.0;
                        }
                        if ((bool)accountList.autoPaste.IsChecked && accountList.autoPaste.Visibility == Visibility.Visible)
                        {
                            if (hWnd == IntPtr.Zero)
                            {
                                try
                                {
                                    Clipboard.SetText(accountList.t_Password.Text);
                                    MessageBox.Show("密碼獲取成功，已複製。");
                                }
                                catch { }
                            }
                            else
                            {
                                const int WM_KEYDOWN = 0X100;
                                const int WM_LBUTTONDOWN = 0x0201;
                                const byte VK_BACK = 0x0008;
                                const byte VK_TAB = 0x0009;
                                const byte VK_ENTER = 0x000D;
                                const byte VK_ESCAPE = 0x001B;
                                const byte VK_END = 0x0023;
                                WindowsAPI.SetForegroundWindow(hWnd);
                                Thread.Sleep(100);
                                // 按下ESC關閉提示框
                                WindowsAPI.PostKey(hWnd, WM_KEYDOWN, VK_ESCAPE);
                                Thread.Sleep(100);
                                // 選中帳號欄
                                System.Drawing.Point oldPoint = new System.Drawing.Point(0, 0);
                                WindowsAPI.GetCursorPos(ref oldPoint);
                                System.Drawing.Point point = new System.Drawing.Point(0, 0);
                                WindowsAPI.ClientToScreen(hWnd, ref point);
                                System.Drawing.Point textBoxPoint = new System.Drawing.Point((int)(370 * dpixRatio), (int)(295 * dpixRatio));
                                WindowsAPI.SetCursorPos(point.X + textBoxPoint.X, point.Y + textBoxPoint.Y);
                                int pos = (textBoxPoint.X & 0xFFFF) | (textBoxPoint.Y << 16);
                                WindowsAPI.PostMessage(hWnd, WM_LBUTTONDOWN, 1, pos);
                                Thread.Sleep(100);
                                WindowsAPI.SetCursorPos(oldPoint.X, oldPoint.Y);
                                // 清空帳號欄內容
                                WindowsAPI.PostKey(hWnd, WM_KEYDOWN, VK_END);
                                for (int i = 0; i < 64; i++)
                                {
                                    WindowsAPI.PostKey(hWnd, WM_KEYDOWN, VK_BACK);
                                }
                                // 輸入帳號
                                WindowsAPI.PostString(hWnd, acc);
                                // 切換到密碼欄
                                WindowsAPI.PostKey(hWnd, WM_KEYDOWN, VK_TAB);
                                // 清空密碼欄內容
                                WindowsAPI.PostKey(hWnd, WM_KEYDOWN, VK_END);
                                for (int i = 0; i < 20; i++)
                                {
                                    WindowsAPI.PostKey(hWnd, WM_KEYDOWN, VK_BACK);
                                }
                                // 輸入密碼
                                WindowsAPI.PostString(hWnd, accountList.t_Password.Text);
                                // 按登入
                                WindowsAPI.PostKey(hWnd, WM_KEYDOWN, VK_ENTER);
                            }
                        }
                    }
                }
            }

            Console.WriteLine("getOtpWorker end");

            accountList.list_Account.IsEnabled = true;
            accountList.btnGetOtp.IsEnabled = true;
            accountList.btn_Logout.IsEnabled = true;
            accountList.btn_ChangeGame.IsEnabled = true;
            accountList.gameName.IsEnabled = true;
            accountList.btn_StartGame.IsEnabled = true;
            accountList.m_MenuList.IsEnabled = true;
            if (this.bfClient.accountAmountLimitNotice != "")
                accountList.btnAddServiceAccount.IsEnabled = this.bfClient.accountList.Count < int.Parse(this.bfClient.accountAmountLimitNotice.Substring(this.bfClient.accountAmountLimitNotice.Length - 1, 1));

            if (!this.pingWorker.IsBusy) this.pingWorker.RunWorkerAsync();
        }

        // Ping to Beanfun website.
        private void pingWorker_DoWork(object sender, DoWorkEventArgs e)
        {
            Thread.CurrentThread.Name = "ping Worker";
            Console.WriteLine("pingWorker start");
            const int WaitSecs = 60; // 1min

            while (true)
            {
                if (this.pingWorker.CancellationPending)
                {
                    Console.WriteLine("break duo to cancel");
                    break;
                }

                if (this.getOtpWorker.IsBusy || this.loginWorker.IsBusy)
                {
                    Console.WriteLine("ping.busy sleep 1s");
                    System.Threading.Thread.Sleep(1000 * 1);
                    continue;
                }

                if (this.bfClient != null)
                    this.bfClient.Ping(); 

                for (int i = 0; i < WaitSecs; ++i)
                {
                    if (this.pingWorker.CancellationPending)
                        break;
                    System.Threading.Thread.Sleep(1000 * 1);
                }
            }
        }

        private void pingWorker_RunWorkerCompleted(object sender, RunWorkerCompletedEventArgs e)
        {
            Console.WriteLine("ping.done");
        }

        private void qrWorker_DoWork(object sender, DoWorkEventArgs e)
        {
            this.bfClient = new BeanfunClient();
            string skey = this.bfClient.GetSessionkey();
            this.qrcodeClass = this.bfClient.GetQRCodeValue(skey, (bool)e.Argument);
        }

        private void qrWorker_RunWorkerCompleted(object sender, RunWorkerCompletedEventArgs e)
        {
            loginPage.ddlAuthType.IsEnabled = true;
            if (updateQRCodeImage()) qrCheckLogin.IsEnabled = true;
        }
        
        private void qrCheckLogin_Tick(object sender, EventArgs e)
        {
            if (this.qrcodeClass == null)
            {
                MessageBox.Show("QRCode not get yet");
                return;
            }
            int res = this.bfClient.QRCodeCheckLoginStatus(this.qrcodeClass);
            if (res != 0)
                this.qrCheckLogin.IsEnabled = false;
            if (res == 1)
            {
                do_Login();
            }
            if (res == -2)
            {
                refreshQRCode();
            }
        }

        public void refreshQRCode()
        {
            qrWorker.RunWorkerAsync(loginPage == null || loginPage.qr == null || loginPage.qr.useNewQRCode == null || (bool)loginPage.qr.useNewQRCode.IsChecked ? false : true);
        }

        public bool updateQRCodeImage()
        {
            loginPage.qr.btn_Refresh_QRCode.IsEnabled = false;

            BitmapImage qrCodeImage;
            bool result;
            if (this.qrcodeClass == null || (qrCodeImage = this.bfClient.getQRCodeImage(qrcodeClass)) == null)
            {
                result = false;
                loginPage.qr.qr_image.Source = qr_default;
            }
            else
            {
                result = true;
                loginPage.qr.qr_image.Source = qrCodeImage;
            }
            loginPage.qr.btn_Refresh_QRCode.IsEnabled = true;

            return result;
        }

        private void bfAPPAutoLogin_Tick(object sender, EventArgs e)
        {
            JObject resultJson = this.bfClient.CheckIsRegisteDevice(service_code, service_region);
            if (resultJson == null || resultJson["IntResult"] == null)
                return;
            if ((string)resultJson["IntResult"] != "1" && (string)resultJson["IntResult"] != "0")
                this.bfAPPAutoLogin.IsEnabled = false;

            switch ((string)resultJson["IntResult"])
            {
                case "-3":
                    Console.WriteLine("登入請求被拒絕");
                    errexit("您的登入要求已被beanfun! App拒絕。", 1);
                    break;
                case "-2":
                    Console.WriteLine("登入請求已逾時");
                    frame.Content = loginPage;
                    break;
                case "-1":
                    errexit((string)resultJson["StrReslut"], 1);
                    break;
                case "0":
                    return;
                case "1":
                    Console.WriteLine("尚未授權本次登入");
                    return;
                case "2":
                    loginWorker_RunWorkerCompleted(null, null);
                    break;
            }
            loginWaitPage.t_Info.Content = "正在登入,請稍等...";
        }

        private void checkPlayPage_Tick(object sender, EventArgs e)
        {
            try
            {
                const uint WM_CLOSE = 0x10;
                IntPtr hWnd;
                if ((hWnd = WindowsAPI.FindWindow("StartUpDlgClass", "MapleStory")) != IntPtr.Zero)
                    WindowsAPI.PostMessage(hWnd, WM_CLOSE, 0, 0);
            }
            catch { }
        }

        private void checkPatcher_Tick(object sender, EventArgs e)
        {
            if (settingPage == null || settingPage.t_GamePath == null) return;
            bool found = false;
            try
            {
                string patherPath = Path.GetDirectoryName(settingPage.t_GamePath.Text) + "\\Patcher.exe";
                foreach (Process process in Process.GetProcessesByName("Patcher"))
                {
                    try
                    {
                        if (process.MainModule.FileName == patherPath)
                        {
                            process.Kill();
                            found = true;
                        }
                    }
                    catch { }
                }
            }
            catch { }

            if (found)
            {
                short ClientMapleMajor = 0;
                short SrvMapleMajor = 0;
                string SrvMapleMinor = "";
                try
                {
                    // 獲取客戶端版本
                    FileVersionInfo fileVerInfo = FileVersionInfo.GetVersionInfo(settingPage.t_GamePath.Text);
                    ClientMapleMajor = (short) fileVerInfo.ProductPrivatePart;

                    // 獲取伺服器版本
                    CancellationTokenSource c = new CancellationTokenSource();
                    CancellationToken token = c.Token;
                    byte[] Data = null;
                    System.Threading.Tasks.Task task = new System.Threading.Tasks.Task(() =>
                    {
                        try
                        {
                            var tcpClient = new System.Net.Sockets.TcpClient();
                            tcpClient.SendTimeout = 6000;
                            tcpClient.ReceiveTimeout = 6000;
                            tcpClient.Connect("tw.login.maplestory.gamania.com", 8484);

                            if (tcpClient.Connected)
                            {
                                Data = new Byte[1024];
                                System.Net.Sockets.NetworkStream nsData = tcpClient.GetStream();
                                Int32 bytes = nsData.Read(Data, 0, Data.Length);
                            }

                            tcpClient.Close();
                            while (true)
                            {
                                if (token.IsCancellationRequested)
                                {
                                    throw new OperationCanceledException();
                                }
                            }
                        }
                        catch { };
                    }, token);

                    task.Start();
                    task.Wait(3000, token);
                    c.Cancel();
                    if (Data != null)
                    {
                        if (accountList != null)
                        {
                            MapleLib.PacketLib.PacketReader packet = new MapleLib.PacketLib.PacketReader(Data);
                            packet.ReadShort();
                            SrvMapleMajor = packet.ReadShort();
                            SrvMapleMinor = packet.ReadMapleString();
                            packet.Skip(4);
                            packet.Skip(4);
                            byte MapleRegion = packet.ReadByte();
                            Console.WriteLine("伺服器版本: " + SrvMapleMajor + "\r\n小版本: " + SrvMapleMinor.Split(':')[0] + "\r\n區域號: " + MapleRegion);

                            if (SrvMapleMajor == 0 || SrvMapleMinor.Split(':')[0] == "" || MapleRegion == 0) Data = null;
                        }
                    }
                    if (Data == null)
                    {
                        SrvMapleMajor = 0;
                        SrvMapleMinor = "";
                    }
                } catch {}
                string info = "";
                if (ClientMapleMajor != 0)
                {
                    info += $"\r\n客戶端版本:{ ClientMapleMajor }";
                    if (SrvMapleMajor != 0 && SrvMapleMinor.Split(':')[0] != "")
                    {
                        info += $"\r\n伺服器版本:{ SrvMapleMajor }.{ SrvMapleMinor.Split(':')[0] }";
                    }
                }
                bool isCanUpdate = ClientMapleMajor != 0 && SrvMapleMajor != 0 && ClientMapleMajor >= (SrvMapleMajor - 2);
                MessageBoxResult result = MessageBox.Show($"遊戲自動更新有可能會造成遊戲程式損毀，已被阻止。{ info }\r\n建議下載{ (isCanUpdate && ClientMapleMajor == SrvMapleMajor ? $"V{ SrvMapleMajor }.{ SrvMapleMinor.Split(':')[0] }fix" : "") }{ (isCanUpdate ? "手動更新檔來更新" : "完整檔重新安裝遊戲") }，如需要使用自動更新功能請到設定頁面取消阻止。\r\n是否前往下載{ (isCanUpdate ? "手動更新檔" : "完整檔主程式") }頁面？", "來自繽放的警告", MessageBoxButton.YesNo);
                if (result == MessageBoxResult.Yes)
                {
                    if (isCanUpdate) Process.Start("https://tw.beanfun.com/MapleStory/download?download_type=2");
                    else Process.Start("https://tw.beanfun.com/MapleStory/download");
                }
            }
        }

        private void verifyWorker_DoWork(object sender, DoWorkEventArgs e)
        {
            e.Result = null;
            string response = "";
            verifyPage.Dispatcher.Invoke(
                new Action(
                    delegate
                    {
                        response = this.bfClient.verify(viewstate, eventvalidation, samplecaptcha, verifyPage.t_Verify.Text, verifyPage.t_Code.Text);
                    }
                )
            );
            Regex regex = new Regex("alert\\('(.*)'\\);");
            string msg = null;
            if (regex.IsMatch(response))
            { msg = regex.Match(response).Groups[1].Value; }
            if (msg == null)
            {
                if (response.Contains("圖形驗證碼輸入錯誤"))
                {
                    MessageBox.Show("圖形驗證碼輸入錯誤");
                }
                else
                {
                    MessageBox.Show("資料錯誤，請重新輸入");
                }
            }
            else
            {
                if (msg.Contains("資料已驗證成功"))
                { e.Result = true; }
                else
                { MessageBox.Show(msg.Replace("\\n", "\n").Replace("\\r", "\r")); }
            }
            if (e.Result == null)
            {
                string errmsg = "Error Load Verify Page";
                verifyPage.Dispatcher.Invoke(
                    new Action(
                        delegate
                        {
                            errmsg = reLoadVerifyPage(response);
                            verifyPage.t_Code.Text = "";
                        }
                    )
                );
                if (errmsg != null)
                { MessageBox.Show(errmsg); }
            }
        }

        private void verifyWorker_RunWorkerCompleted(object sender, RunWorkerCompletedEventArgs e)
        {
            if (e.Result != null)
            { do_Login(); }
        }
    }
}
