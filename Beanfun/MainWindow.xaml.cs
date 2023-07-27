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
using System.Management;
using System.Net;
using System.Runtime.InteropServices;
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
        LocaleRemulator = 2
    };
    /// <summary>
    /// MainWindow.xaml 的交互逻辑
    /// </summary>
    public partial class MainWindow : Window
    {
        public LoginPage loginPage;
        public ManageAccount manageAccPage;
        public LoginWait loginWaitPage = new LoginWait();
        public AccountList accountList = null;
        public VerifyPage verifyPage;
        public Settings settingPage;
        public About aboutPage;
        public LoginTotp loginTotp;

        public System.ComponentModel.BackgroundWorker getOtpWorker;
        public System.ComponentModel.BackgroundWorker loginWorker;
        public System.ComponentModel.BackgroundWorker totpWorker;
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
        private static readonly System.Windows.Forms.NotifyIcon _trayNotifyIcon = new System.Windows.Forms.NotifyIcon
        {
            Icon = Properties.Resources.icon,
            Text = Application.Current.TryFindResource("AppName") as string
        };

        public Dictionary<string, List<GameService>> GameList = new Dictionary<string, List<GameService>>();
        public GameService SelectedGame = null;
        public bool UnconnectedGame = false;

        public string viewstate, eventvalidation, samplecaptcha;

        public Page return_page = null;
        public IniData INIData = null;

        public WindowAccentCompositor compositor = null;

        public MainWindow()
        {
            InitializeComponent();

            this.getOtpWorker = new System.ComponentModel.BackgroundWorker();
            this.loginWorker = new System.ComponentModel.BackgroundWorker();
            this.totpWorker = new System.ComponentModel.BackgroundWorker();
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
            // totpWorker
            //
            this.totpWorker.WorkerReportsProgress = true;
            this.totpWorker.WorkerSupportsCancellation = true;
            this.totpWorker.DoWork += this.totpWorker_DoWork;
            this.totpWorker.RunWorkerCompleted += this.totpWorker_RunWorkerCompleted;
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
            manageAccPage = new ManageAccount();
            verifyPage = new VerifyPage();
            accountList = new AccountList();
            settingPage = new Settings();
            aboutPage = new About();
            loginTotp = new LoginTotp();

            Initialize();

            if ((App.OSVersion >= App.Win7 && App.OSVersion < App.Win8) || App.OSVersion >= App.Win10)
            {
                compositor = new WindowAccentCompositor(this);
                if (App.OSVersion >= App.Win7 && App.OSVersion < App.Win8)
                {
                    WinChrome.GlassFrameThickness = new Thickness(-1);

                    const int GWL_STYLE = -16;
                    const int WS_SYSMENU = 0x80000;
                    var hwnd = new System.Windows.Interop.WindowInteropHelper(this).EnsureHandle();
                    WindowsAPI.SetWindowLong(hwnd, GWL_STYLE, WindowsAPI.GetWindowLong(hwnd, GWL_STYLE) & ~WS_SYSMENU);
                }
            } else frame.Content = loginWaitPage;

            changeThemeColor(null);
        }

        protected override void OnContentRendered(EventArgs e)
        {
            frame.Content = loginPage;
            //frame.Content = loginTotp;

            if (App.LoginMethod == (int)LoginMethod.Regular && (bool)loginPage.id_pass.checkBox_AutoLogin.IsChecked)
            {
                do_Login();
            }

            base.OnContentRendered(e);
        }

        public void NavigateLoginPage()
        {
            frame.Content = loginPage;

            btn_Region.Visibility = Visibility.Visible;

            try
            {
                if (bfClient != null) bfClient.Logout();
            } catch { }
        }

        public void changeThemeColor(string sColor)
        {
            if (sColor == null) sColor = ConfigAppSettings.GetValue("ThemeColor", "#FF8201");
            Color color = (Color)ColorConverter.ConvertFromString(sColor);
            bool oldIsLightColor = isLightColor();
            Background = new SolidColorBrush(color);
            color.R = (byte)Math.Max(color.R - 50, 0);
            color.G = (byte)Math.Max(color.G - 50, 0);
            color.B = (byte)Math.Max(color.B - 50, 0);
            color.A = 0xFF;
            this.BorderBrush = new SolidColorBrush(color);
            bool isLightMode = isLightColor();
            if (compositor !=  null)
            {
                int bgA = -1;
                if (!this.IsActive)
                {
                    compositor.IsEnabled = false;
                    if (App.OSVersion >= App.Win7 && App.OSVersion < App.Win8) bgA = 0;
                }
                else
                {
                    bgA = App.OSVersion < App.Win8 ? 0x4C : App.OSVersion < App.Win11 ? 0xCC : 0x99;
                    compositor.Color = (Color)ColorConverter.ConvertFromString(isLightMode || ((SolidColorBrush)Background).Color == Colors.Black ? "#00FFFFFF" : "#00000000");
                    if (!compositor.IsEnabled) compositor.IsEnabled = true;
                }
                if (bgA != -1)
                {
                    Color bg = ((SolidColorBrush)Background).Color;
                    bg.A = (byte) bgA;
                    Background = new SolidColorBrush(bg);
                }
            }
            if (oldIsLightColor != isLightMode)
            {
                btn_About_MouseLeave(null, null);
                btn_Setting_MouseLeave(null, null);
                btn_Region_MouseLeave(null, null);
                btn_Min_MouseLeave(null, null);
                btn_Close_MouseLeave(null, null);
                LogoIcon.Fill = new SolidColorBrush(isLightMode ? Colors.Black : Colors.White);
                if (aboutPage != null) aboutPage.initThemeColor(isLightMode);
            }
        }
        
        public bool isLightColor()
        {
            Color color = ((SolidColorBrush)Background).Color;
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
                if (App.OSVersion < App.Win11)
                {
                    if (App.OSVersion >= App.Win8_1)
                        ServicePointManager.SecurityProtocol |= SecurityProtocolType.Ssl3;
                    ServicePointManager.SecurityProtocol |= SecurityProtocolType.Tls12;
                    ServicePointManager.ServerCertificateValidationCallback = (sender, certificate, chain, errors) => true;
                }
                if (settingPage.tradLogin != null && !(bool)settingPage.tradLogin.IsChecked)
                    accountList.panel_GetOtp.Visibility = Visibility.Collapsed;

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

                if ((bool)settingPage.ask_update.IsChecked)
                {
                    new Thread(() => CheckUpdates(false)).Start();
                }

                this.accountManager = new AccountManager();

                bool res = accountManager.init();
                if (res == false)
                    errexit(TryFindResource("InitAccountError") as string, 0);

                settingPage.t_GamePath.PreviewMouseLeftButtonDown += this.btn_SetGamePath_Click;
                LastLoginAccountID = ConfigAppSettings.GetValue("AccountID", LastLoginAccountID);
                int loginMethod = accountManager.getMethodByAccount(App.LoginRegion, LastLoginAccountID);
                if (loginMethod < (int)LoginMethod.Regular)
                    loginMethod = int.Parse(ConfigAppSettings.GetValue("loginMethod", "0"));
                if (loginMethod > (int)LoginMethod.QRCode)
                    loginMethod = (int)LoginMethod.QRCode;

                loginMethodInit();
                reLoadGameInfo();

                App.LoginMethod = loginMethod;
                loginMethodChanged();

                _trayNotifyIcon.MouseClick += (sender, e) =>
                {
                    if (e.Button == System.Windows.Forms.MouseButtons.Left)
                    {
                        this.Visibility = Visibility.Visible;
                        _trayNotifyIcon.Visible = false;
                    }
                };

                frame.Content = loginPage;
            }
            catch (Exception ex)
            {
                Console.WriteLine(ex.StackTrace);
                MessageBox.Show(string.Format((TryFindResource("LoadDataError") as string).Replace("\\r\\n", "\r\n"), ex.Message)/* + "\r\n\r\n" + ex.StackTrace*/);

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

            private string imageBaseUrl
            {
                get
                {
                    return App.LoginRegion == "TW" ? "https://tw.images.beanfun.com/uploaded_images/beanfun_tw/game_zone/" : "http://hk.images.beanfun.com/uploaded_images/beanfun/game_zone/";
                }
            }

            private BitmapImage xlarge_image;
            public BitmapImage XLarge_image
            {
                get
                {
                    if (xlarge_image == null)
                        xlarge_image = loadImage($"{ imageBaseUrl }{ xlarge_image_name }");
                    return xlarge_image;
                }
            }

            private BitmapImage large_image;
            public BitmapImage Large_image {
                get {
                    if (large_image == null)
                        large_image = loadImage($"{ imageBaseUrl }{ large_image_name }");
                    return large_image;
                }
            }

            private BitmapImage small_image;
            public BitmapImage Small_image
            {
                get
                {
                    if (small_image == null)
                        small_image = loadImage($"{ imageBaseUrl }{ small_image_name }");
                    return small_image;
                }
            }

            public GameService(string name, string service_code, string service_region, string website_url, string xlarge_image_name, string large_image_name, string small_image_name, string download_url)
            {
                this.name = I18n.ToSimplified(name);
                this.service_code = service_code;
                this.service_region = service_region;
                this.website_url = website_url;
                this.xlarge_image_name = xlarge_image_name;
                this.large_image_name = large_image_name;
                this.small_image_name = small_image_name;
                this.download_url = download_url;
            }

            private BitmapImage loadImage(string url)
            {
                BitmapImage image;
                try
                {
                    byte[] buffer = new WebClientEx().DownloadData(url);
                    image = new BitmapImage();
                    image.BeginInit();
                    image.StreamSource = new MemoryStream(buffer);
                    image.EndInit();
                }
                catch (Exception)
                {
                    image = null;
                }
                return image;
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

            if (gameCode == "610074_T9" || gameCode == "610075_T9")
            {
                settingPage.skipPlayWnd.Visibility = Visibility.Visible;

                if ((bool)settingPage.skipPlayWnd.IsChecked)
                    checkPlayPage.IsEnabled = true;

                settingPage.autoKillPatcher.Visibility = Visibility.Visible;

                if ((bool)settingPage.autoKillPatcher.IsChecked)
                    checkPatcher.IsEnabled = true;

                settingPage.btn_Tools.Visibility = Visibility.Visible;
            }
            else
            {
                settingPage.skipPlayWnd.Visibility = Visibility.Collapsed;
                checkPlayPage.IsEnabled = false;
                settingPage.autoKillPatcher.Visibility = Visibility.Collapsed;
                checkPatcher.IsEnabled = false;

                if (gameCode == "610096_TE")
                    settingPage.btn_Tools.Visibility = Visibility.Visible;
                else
                    settingPage.btn_Tools.Visibility = Visibility.Collapsed;
            }

            if (this.bfClient != null && !loginWorker.IsBusy && !getOtpWorker.IsBusy)
            {
                this.bfClient.GetAccounts(service_code, service_region);
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

            try
            {
                if (loginPage != null)
                {
                    foreach (GameService gs in GameList[App.LoginRegion.ToLower()])
                    {
                        if (gs.service_region == this.service_region && gs.service_code == this.service_code)
                        {
                            loginPage.id_pass.imageGame.ImageSource = gs.Large_image;
                            accountList.imageGame.Source = gs.Small_image;
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
            if (!GameList.ContainsKey(App.LoginRegion.ToLower()))
            {
                List<GameService> gameList = new List<GameService>();
                WebClient wc = new WebClientEx();

                string res = Encoding.UTF8.GetString(wc.DownloadData("https://" + (App.LoginRegion == "HK" ? "bfweb.hk" : "tw") + ".beanfun.com/beanfun_block/generic_handlers/get_service_ini.ashx"));

                StringIniParser sip = new StringIniParser();
                INIData = sip.ParseString(res);

                res = Encoding.UTF8.GetString(wc.DownloadData("https://" + (App.LoginRegion == "HK" ? "bfweb.hk" : "tw") + ".beanfun.com/game_zone/"));
                Regex reg = new Regex("Services.ServiceList = (.*);");
                if (reg.IsMatch(res))
                {
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
                GameList.Add(App.LoginRegion.ToLower(), gameList);
            }

            selectedGameChanged();
        }

        public void CheckUpdates(bool show)
        {
            Update.ApplicationUpdater.CheckApplicationUpdate(show);
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
            btn_Region_MouseLeave(null, null);
            btn_Min_MouseLeave(null, null);
            btn_Close_MouseLeave(null, null);
            if (this.IsActive)
            {
                changeThemeColor(null);
            }
            else
            {
                changeThemeColor("#F3F3F3");
                this.BorderBrush = new SolidColorBrush((Color)ColorConverter.ConvertFromString("Gray"));
            }
        }

        private void Window_StateChanged(object sender, EventArgs e)
        {
            if (settingPage != null && settingPage.minimize_to_tray != null && (bool)settingPage.minimize_to_tray.IsChecked && this.WindowState == WindowState.Minimized)
            {
                this.WindowState = WindowState.Normal;
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

        private void btn_Region_Click(object sender, RoutedEventArgs e)
        {
            App.LoginRegion = App.LoginRegion == "TW" ? "HK" : "TW";
            ConfigAppSettings.SetValue("loginRegion", App.LoginRegion);
            loginMethodInit();
            reLoadGameInfo();
        }

        private void btn_Region_MouseLeave(object sender, MouseEventArgs e)
        {
            if (this.IsActive)
                btn_Region.Foreground = new SolidColorBrush(getTitleButtonColor());
            else
                btn_Region.Foreground = new SolidColorBrush((Color)ColorConverter.ConvertFromString("Gray"));
        }

        private void btn_Region_IsKeyboardFocusedChanged(object sender, DependencyPropertyChangedEventArgs e)
        {
            if (btn_Region.IsKeyboardFocused) frame.Focus();
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
            openFileDialog.Filter = accountList.gameName.Content + string.Format(TryFindResource("FileDialog_Filter") as string, game_exe);
            openFileDialog.Title = string.Format(TryFindResource("FileDialog_Title") as string, game_exe);

            if (openFileDialog.ShowDialog() == true)
            {
                string file = openFileDialog.FileName;
                ConfigAppSettings.SetValue(dir_value_name + "." + gameCode, file);
                settingPage.t_GamePath.Text = file;
            }
        }

        public void loginMethodChanged()
        {
            qrCheckLogin.IsEnabled = false;
            
            if (App.LoginRegion == "TW")
            {
                switch (App.LoginMethod)
                {
                    case (int)LoginMethod.QRCode:
                        btn_Region.IsEnabled = false;
                        loginPage.qr.qr_image.Source = qr_default;
                        loginPage.login_form.Content = loginPage.qr;
                        qrWorker.RunWorkerAsync(loginPage == null || loginPage.qr == null ? false : true);
                        break;
                    default:
                        loginPage.login_form.Content = loginPage.id_pass;
                        break;
                }
            }
            else
            {
                loginPage.login_form.Content = loginPage.id_pass;
                App.LoginMethod = (int)LoginMethod.Regular;
            }

            if (App.LoginMethod == (int)Beanfun.LoginMethod.Regular && (loginPage.id_pass.t_Password.Password == "" || loginPage.id_pass.t_Password.Password == null))
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

        public void loginMethodInit()
        {
            try
            {
                if (App.LoginRegion == "TW")
                {
                    btn_Region.Content = "TW";
                    btn_Region.ToolTip = TryFindResource("ChangHKRegion") as string;
                    loginPage.id_pass.btn_QRCode.IsEnabled = true;

                    accountList.btn_Deposite.Visibility = Visibility.Visible;
                }
                else
                {
                    btn_Region.Content = "HK";
                    btn_Region.ToolTip = TryFindResource("ChangTWRegion") as string;
                    loginPage.id_pass.btn_QRCode.IsEnabled = false;

                    accountList.btn_Deposite.Visibility = Visibility.Collapsed;
                }
            }
            catch { }

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

                    App.LoginMethod = loginMethod;
                    loginMethodChanged();

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

                    App.LoginMethod = (int)LoginMethod.Regular;
                    loginMethodChanged();

                    verifyPage.t_Verify.Text = "";
                    verifyPage.checkBoxRememberVerify.IsChecked = false;
                }
            }
            catch { /* ignore out of range */ }
            manageAccPage.setupAccList(this);
        }

        public void do_Login()
        {
            btn_Region.Visibility = Visibility.Collapsed;
            this.loginWorker.RunWorkerAsync(App.LoginMethod);
            frame.Content = loginWaitPage;
        }

        public void do_Totp()
        {
            btn_Region.Visibility = Visibility.Collapsed;
            frame.Content = loginWaitPage;
            this.totpWorker.RunWorkerAsync();
        }

        public bool errexit(string msg, int method, string title = null)
        {
            string originalMsg = msg;

            switch (msg)
            {
                case "LoginNoResponse":
                case "LoginNoSkey":
                case "LoginNoOTP1":
                case "LoginNoSeed":
                case "LoginNoHash":
                case "LoginIntResultError":
                case "AKeyParseFailed":
                case "authkeyParseFailed":
                case "LoginUnknown":
                    msg = TryFindResource(msg) as string;
                    method = 0;
                    break;
                case "LoginNoAkey":
                    msg = $"{ TryFindResource("LoginNoAkey") as string }({ msg })";
                    break;
                case "LoginNoAccountMatch":
                case "LoginGetAccountErr":
                case "LoginUpdateAccountListErr":
                    msg = $"{ TryFindResource("LoginNoAccountMatch") as string }({ msg })";
                    break;
                case "MainAccount_Not_Exist":
                    msg = string.Format(TryFindResource("MainAccount_Not_Exist") as string, App.LoginRegion == "TW" ? TryFindResource("Taiwan") : TryFindResource("HongKong"));
                    break;
                default:
                    if (msg.StartsWith("OTPNoLongPollingKey:"))
                    {
                        msg = msg.Replace("OTPNoLongPollingKey:", "");
                        if (msg == "") msg = TryFindResource("GetOtpInitError") as string;
                        else if (msg.Contains("很抱歉，需先完成進階認證")) msg = TryFindResource("NeedAuthToPlayGame") as string;
                        else if (msg.Contains("尚未登入，請重新登入") || msg.Contains("無法認證登入狀態")) { msg = TryFindResource("DisconnectedFromServer") as string; method = 1; };
                    }
                    else
                    {
                        string res = null;
                        try
                        {
                            res = TryFindResource(msg) as string;
                        } catch {}
                        if (res != null) msg = res;
                    }
                    break;
            }

            MessageBox.Show(I18n.ToSimplified(msg).Replace("\\r\\n", "\r\n"), title);
            if (method == 0)
                App.Current.Shutdown();
            else if (method == 1)
            {
                loginMethodChanged();
                accountList.t_Password.Text = "";
                NavigateLoginPage();
            }

            return false;
        }

        private volatile bool isCancelRequested;

        public void CancelWork()
        {
            isCancelRequested = true;
        }

        public void ResumeWork()
        {
            isCancelRequested = false;
        }


        // Login do work.
        private void loginWorker_DoWork(object sender, DoWorkEventArgs e)
        {
            //if (this.pingWorker.IsBusy) this.pingWorker.CancelAsync();
            // while (this.pingWorker.IsBusy)
            //    Thread.Sleep(137);
            CancelWork();

            Console.WriteLine("loginWorker starting");
            Thread.CurrentThread.Name = "Login Worker";
            e.Result = "";
            try
            {
                loginWaitPage.Dispatcher.Invoke(
                    new Action(
                        delegate
                        {
                            if (App.LoginMethod != (int)LoginMethod.QRCode)
                                this.bfClient = new BeanfunClient();
                            this.bfClient.Login(loginPage.id_pass.t_AccountID.Text, loginPage.id_pass.t_Password.Password, App.LoginMethod, this.qrcodeClass, this.service_code, this.service_region);
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
                e.Result = TryFindResource("LoginErrorUnknown") as string + "\n\n" + ex.Message + "\n" + ex.StackTrace;
            }

            ResumeWork();
        }

        // Login completed.
        private void loginWorker_RunWorkerCompleted(object sender, RunWorkerCompletedEventArgs e)
        {
            Console.WriteLine("loginWorker end");
            if (e != null && e.Error != null)
            {

                errexit(e.Error.Message, 1);
                NavigateLoginPage();
                return;
            }
            if (e != null && (string)e.Result != null)
            {
                if ((string)e.Result == "need_totp")
                {
                    frame.Content = loginTotp;
                    loginTotp.btn_login.IsEnabled = true;
                    loginTotp.btn_cancel.IsEnabled = true;
                    loginTotp.otp1.Text = "";
                    loginTotp.otp2.Text = "";
                    loginTotp.otp3.Text = "";
                    loginTotp.otp4.Text = "";
                    loginTotp.otp5.Text = "";
                    loginTotp.otp6.Text = "";
                    loginTotp.otp1.Focus();
                    return;
                }
                else if ((string)e.Result == "LoginAdvanceCheck")
                {
                    MessageBox.Show(TryFindResource("MsgNeedAuth") as string);

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
                    { MessageBox.Show(I18n.ToSimplified(this.bfClient.errmsg)); NavigateLoginPage(); }
                    string errmsg = reLoadVerifyPage(response);
                    if (errmsg != null)
                    { MessageBox.Show(I18n.ToSimplified(errmsg)); NavigateLoginPage(); }
                }
                else if (((string)e.Result).StartsWith("bfAPPAutoLogin.ashx"))
                {
                    string[] args = Regex.Split((string)e.Result, "\",\"");
                    if (args.Length < 2)
                    {
                        errexit("LoginUnknown", 1);
                        return;
                    }
                    loginWaitPage.t_Info.Content = (TryFindResource("MsgNeedBeanfunAuth") as string).Replace("\\r\\n", "\r\n");
                    bfAPPAutoLogin.IsEnabled = true;
                }
                else
                {
                    errexit((string)e.Result, 1);
                }
                return;
            }
            
            ConfigAppSettings.SetValue("loginMethod", App.LoginMethod.ToString());
            if (App.LoginRegion != "TW" || App.LoginMethod != (int)LoginMethod.QRCode)
            {
                LastLoginAccountID = loginPage.id_pass.t_AccountID.Text;
                ConfigAppSettings.SetValue("AccountID", LastLoginAccountID);
                accountManager.addAccount(
                    App.LoginRegion,
                    loginPage.id_pass.t_AccountID.Text,
                    "",
                    loginPage.id_pass.checkBox_RememberPWD.IsEnabled && (bool)loginPage.id_pass.checkBox_RememberPWD.IsChecked ? loginPage.id_pass.t_Password.Password : "",
                    (bool)verifyPage.checkBoxRememberVerify.IsChecked ? verifyPage.t_Verify.Text : "",
                    App.LoginMethod,
                    (bool)loginPage.id_pass.checkBox_AutoLogin.IsChecked
                );

                loginMethodInit();
            }
            else ConfigAppSettings.SetValue("AccountID", null);

            try
            {
                frame.Content = accountList;
                btn_Region.Visibility = Visibility.Collapsed;

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
                errexit(TryFindResource("LoginNoAccountMatch") as string, 1);
            }
        }


        // totp do work.
        private void totpWorker_DoWork(object sender, DoWorkEventArgs e)
        {
            //if (this.pingWorker.IsBusy) this.pingWorker.CancelAsync();
            // while (this.pingWorker.IsBusy)
            //    Thread.Sleep(137);
            CancelWork();

            Console.WriteLine("loginWorker starting");
            Thread.CurrentThread.Name = "Totp Worker";
            e.Result = "";
            try
            {
                loginWaitPage.Dispatcher.Invoke(
                    new Action(
                        delegate
                        {
                            this.bfClient.TotpLogin(loginTotp.otp1.Text,loginTotp.otp2.Text,loginTotp.otp3.Text,loginTotp.otp4.Text,loginTotp.otp5.Text,loginTotp.otp6.Text,this.service_code, this.service_region);
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
                e.Result = TryFindResource("LoginErrorUnknown") as string + "\n\n" + ex.Message + "\n" + ex.StackTrace;
            }

            ResumeWork();
        }

        // Login completed.
        private void totpWorker_RunWorkerCompleted(object sender, RunWorkerCompletedEventArgs e)
        {
            Console.WriteLine("loginWorker end");
            if (e != null && e.Error != null)
            {
                errexit(e.Error.Message, 1);
                NavigateLoginPage();
                return;
            }
            if (e != null && (string)e.Result != null)
            {
                if ((string)e.Result == "LoginAdvanceCheck")
                {
                    MessageBox.Show(TryFindResource("MsgNeedAuth") as string);

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
                    { MessageBox.Show(I18n.ToSimplified(this.bfClient.errmsg)); NavigateLoginPage(); }
                    string errmsg = reLoadVerifyPage(response);
                    if (errmsg != null)
                    { MessageBox.Show(I18n.ToSimplified(errmsg)); NavigateLoginPage(); }
                }
                else if (((string)e.Result).StartsWith("bfAPPAutoLogin.ashx"))
                {
                    string[] args = Regex.Split((string)e.Result, "\",\"");
                    if (args.Length < 2)
                    {
                        errexit("LoginUnknown", 1);
                        return;
                    }
                    loginWaitPage.t_Info.Content = (TryFindResource("MsgNeedBeanfunAuth") as string).Replace("\\r\\n", "\r\n");
                    bfAPPAutoLogin.IsEnabled = true;
                }
                else
                {
                    errexit((string)e.Result, 1);
                }
                return;
            }
            
            ConfigAppSettings.SetValue("loginMethod", App.LoginMethod.ToString());
            if (App.LoginRegion != "TW" || App.LoginMethod != (int)LoginMethod.QRCode)
            {
                LastLoginAccountID = loginPage.id_pass.t_AccountID.Text;
                ConfigAppSettings.SetValue("AccountID", LastLoginAccountID);
                accountManager.addAccount(
                    App.LoginRegion,
                    loginPage.id_pass.t_AccountID.Text,
                    "",
                    loginPage.id_pass.checkBox_RememberPWD.IsEnabled && (bool)loginPage.id_pass.checkBox_RememberPWD.IsChecked ? loginPage.id_pass.t_Password.Password : "",
                    (bool)verifyPage.checkBoxRememberVerify.IsChecked ? verifyPage.t_Verify.Text : "",
                    App.LoginMethod,
                    (bool)loginPage.id_pass.checkBox_AutoLogin.IsChecked
                );

                loginMethodInit();
            }
            else ConfigAppSettings.SetValue("AccountID", null);

            try
            {
                frame.Content = accountList;
                btn_Region.Visibility = Visibility.Collapsed;

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
                errexit(TryFindResource("LoginNoAccountMatch") as string, 1);
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
                    accountList.btnAddServiceAccount.Content = TryFindResource("GoToVerify");
                    accountList.btnAddServiceAccount.IsEnabled = true;
                    accountList.btnAddServiceAccount.Visibility = Visibility.Visible;
                }
                else
                {
                    accountList.btnAddServiceAccount.Content = TryFindResource("AddServiceAccount");
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
                //accountList.m_ChangeAccName.Visibility = !UnconnectedGame || App.LoginRegion != "TW" ? Visibility.Visible : Visibility.Collapsed;
                accountList.m_ChangePassword.Visibility = UnconnectedGame ? Visibility.Visible : Visibility.Collapsed;
                //accountList.m_AccInfo.Visibility = !UnconnectedGame || App.LoginRegion != "TW" ? Visibility.Visible : Visibility.Collapsed;
                accountList.s_Account.Visibility = Visibility.Visible;
            }
            else
            {
                accountList.m_CopyAccount.Visibility = Visibility.Collapsed;
                //accountList.m_ChangeAccName.Visibility = Visibility.Collapsed;
                accountList.m_ChangePassword.Visibility = Visibility.Collapsed;
                //accountList.m_AccInfo.Visibility = Visibility.Collapsed;
                accountList.s_Account.Visibility = Visibility.Collapsed;
            }
            accountList.m_GetEmail.Visibility = visable;

            if (gameCode == "610074_T9" || gameCode == "610075_T9" || gameCode == "610096_TE")
                accountList.btn_Tools.Visibility = Visibility.Visible;
            else
                accountList.btn_Tools.Visibility = Visibility.Collapsed;
        }

        public void updateRemainPoint(int remainPoint)
        {
            accountList.m_RemainPoint.Header = string.Format(TryFindResource("GashRemain") as string, $"{ remainPoint }{(App.LoginRegion == "TW" || remainPoint == 0 ? "" : string.Format(TryFindResource("GashRemainInGame") as string, Math.Floor(remainPoint / 2.5)))}");
        }

        public void runGame(string account = null, string password = null)
        {
            string gameCode = service_code + "_" + service_region;
            string gamePath = settingPage.t_GamePath.Text;
            if (gamePath == "" || !File.Exists(gamePath))
            {
                MessageBoxResult result = MessageBox.Show(TryFindResource("MsgCantFindGame") as string, "", MessageBoxButton.YesNo);
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

            for (int i = 0; i < gamePath.Length; i++)
            {
                if (Convert.ToInt32(Convert.ToChar(gamePath.Substring(i, 1))) > Convert.ToInt32(Convert.ToChar(128)))
                {
                    MessageBox.Show(TryFindResource("MsgGamePathHaveWChar") as string);
                    break;
                }
            }

            List<int> processIds = new List<int>();

            Regex regexx = new Regex("(.*).exe");
            string gameProcessName = "";
            if (regexx.IsMatch(game_exe))
                gameProcessName = regexx.Match(game_exe).Groups[1].Value;
            if (gameProcessName != "")
            {
                foreach (Process process in Process.GetProcessesByName(gameProcessName))
                {
                    if (processIds.Contains(process.Id)) { continue; }
                    try
                    {

                        using (ManagementObjectSearcher searcher = new ManagementObjectSearcher("select * from Win32_Process where ProcessId = " + process.Id))
                        using (ManagementObjectCollection objects = searcher.Get())
                        {
                            if (gamePath == objects.Cast<ManagementBaseObject>().SingleOrDefault()?["executablepath"]?.ToString())
                            {
                                processIds.Add(process.Id);
                                continue;
                            }
                        }
                    }
                    catch { }
                    try
                    {
                        if (process.MainModule.FileName == gamePath)
                        { processIds.Add(process.Id); continue; }
                    }
                    catch { }
                }
            }

            if (processIds.Count > 0)
            {
                MessageBoxResult result = MessageBox.Show(TryFindResource("MsgGameAlreadyRun") as string, "", MessageBoxButton.YesNo);
                if (result == MessageBoxResult.Yes)
                {
                    foreach(int processId in processIds)
                    {
                        try
                        {
                            Process process = Process.GetProcessById(processId);
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
                bool is64BitGame = false;
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
                            WindowsAPI.BinaryType bt;
                            if (WindowsAPI.GetBinaryType(gamePath, out bt))
                            {
                                is64BitGame = bt == WindowsAPI.BinaryType.SCS_64BIT_BINARY;
                            }
                            if (App.OSVersion < App.WinVista)
                            {
                                errexit(TryFindResource("MsgLEDoNotSupportXP") as string, 2);
                                return;
                            }
                            else
                            {
                                runMode = (int)GameStartMode.LocaleRemulator;
                                break;
                            }
                    }
                }

                if (runMode > (int)GameStartMode.LocaleRemulator) runMode = (int)GameStartMode.LocaleRemulator;

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
                    case (int)GameStartMode.LocaleRemulator:
                        startByLR(gamePath, commandLine, is64BitGame);
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
            catch(Exception ex)
            {
                Console.WriteLine(ex.ToString());
                errexit((TryFindResource("MsgLocalePluginRunError") as string).Replace("\\r\\n", "\r\n"), 2);
            }
        }

        private void startByLR(string path, string command, bool is64BitGame)
        {
            if (App.ReleaseResource("LRProc.dll") == -1 || App.ReleaseResource("LRHookx32.dll") == -1 || App.ReleaseResource("LRHookx64.dll") == -1)
                MessageBox.Show(TryFindResource("MsgLocalePluginReleaseError") as string);
            string dllPath = string.Format("{0}\\{1}", System.Environment.CurrentDirectory, "LRHookx32.dll");

            var commandLine = string.Empty;
            commandLine = path.StartsWith("\"")
                ? $"{path} "
                : $"\"{path}\" ";
            commandLine += command;
            System.Globalization.TextInfo culInfo = System.Globalization.CultureInfo.GetCultureInfo("zh-HK").TextInfo;

            new Thread(new ThreadStart(() => {
                try
                {
                    LRInject(path, Path.GetDirectoryName(path), commandLine, dllPath, (uint)culInfo.ANSICodePage, App.OSVersion >= App.Win8 && is64BitGame);
                }
                catch (Exception ex)
                {
                    Console.WriteLine(ex.ToString());
                    errexit((TryFindResource("MsgLocalePluginRunError") as string).Replace("\\r\\n", "\r\n"), 2);
                }
            })).Start();
        }

        [DllImport("LRProc.dll", EntryPoint = "LRInject", CharSet = CharSet.Ansi ,CallingConvention = CallingConvention.Cdecl)]
        public static extern int LRInject(string application, string workpath, string commandline, string dllpath, uint CodePage, bool HookIME);

        public bool AddServiceAccount(string name)
        {
            if (this.bfClient == null)
                return false;
            if (name == null || name == "")
                return false;

            if (this.bfClient.AddServiceAccount(name, service_code, service_region))
            {
                this.bfClient.GetAccounts(service_code, service_region);
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
                this.bfClient.GetAccounts(service_code, service_region);
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
            CancelWork();
            //if (this.pingWorker.IsBusy) this.pingWorker.CancelAsync();
            /*while (this.pingWorker.IsBusy) {
                Thread.Sleep(133);
            }
            */

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

            //if (!this.pingWorker.IsBusy) this.pingWorker.RunWorkerAsync();
            //this.pingWorker.RunWorkerAsync();
            ResumeWork();
            return;
        }

        // getOTP completed.
        private void getOtpWorker_RunWorkerCompleted(object sender, RunWorkerCompletedEventArgs e)
        {
            accountList.btnGetOtp.Content = TryFindResource("GetOtp") as string;
            if (e.Error != null)
            {
                errexit(e.Error.Message, 2, TryFindResource("GetOtpFailed") as string);
            }
            else
            {
                int index = (int)e.Result;

                if (index == -1)
                {
                    errexit(this.bfClient.errmsg, 2, TryFindResource("GetOtpFailed") as string);
                }
                else
                {
                    int accIndex = accountList.list_Account.SelectedIndex;
                    string acc = this.bfClient.accountList[index].sid;
                    accountList.t_Password.Text = this.otp;
                    

                    if (!(bool)settingPage.tradLogin.IsChecked && login_action_type == 1)
                    {
                        runGame(acc, accountList.t_Password.Text);
                    }
                    else
                    {
                        IntPtr hWnd = WindowsAPI.FindWindow(win_class_name, null);
                        if ("MapleStoryClass".Equals(win_class_name) && hWnd == IntPtr.Zero)
                        {
                            hWnd = WindowsAPI.FindWindow("MapleStoryClassTW", null);
                        }
                        if ((bool)accountList.autoPaste.IsChecked && accountList.autoPaste.Visibility == Visibility.Visible)
                        {
                            if (hWnd == IntPtr.Zero)
                            {
                                try
                                {
                                    Clipboard.SetText(accountList.t_Password.Text);
                                    MessageBox.Show(TryFindResource("GetOtpSuccessAndCopy") as string);
                                }
                                catch { }
                            }
                            else
                            {
                                double dpixRatio = 1.0;
                                if (hWnd != IntPtr.Zero)
                                {
                                    System.Drawing.Graphics currentGraphics = System.Drawing.Graphics.FromHwnd(hWnd);
                                    dpixRatio = currentGraphics.DpiX / 96.0;
                                }

                                const int WM_KEYDOWN = 0X100;
                                const int WM_LBUTTONDOWN = 0x0201;
                                const byte VK_BACK = 0x0008;
                                const byte VK_TAB = 0x0009;
                                const byte VK_ENTER = 0x000D;
                                const byte VK_ESCAPE = 0x001B;
                                const byte VK_END = 0x0023;
                                WindowsAPI.SetForegroundWindow(hWnd);
                                Thread.Sleep(100);
                                if  ("610074".Equals(service_code) && "T9".Equals(service_region))
                                {
                                    // 按下ESC關閉提示框
                                    WindowsAPI.PostKey(hWnd, WM_KEYDOWN, VK_ESCAPE);
                                    Thread.Sleep(100);
                                    // 選中帳號欄
                                    System.Drawing.Point oldPoint = new System.Drawing.Point(0, 0);
                                    WindowsAPI.GetCursorPos(ref oldPoint);
                                    System.Drawing.Point point = new System.Drawing.Point(0, 0);
                                    WindowsAPI.ClientToScreen(hWnd, ref point);
                                    System.Drawing.Point textBoxPoint = new System.Drawing.Point((int)(500 * dpixRatio), (int)(338 * dpixRatio));
                                    WindowsAPI.SetCursorPos(point.X + textBoxPoint.X, point.Y + textBoxPoint.Y);
                                    int pos = (textBoxPoint.X & 0xFFFF) | (textBoxPoint.Y << 16);
                                    WindowsAPI.PostMessage(hWnd, WM_LBUTTONDOWN, 1, pos);
                                    Thread.Sleep(200);
                                    WindowsAPI.SetCursorPos(oldPoint.X, oldPoint.Y);
                                }
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

            //if (!this.pingWorker.IsBusy)  this.pingWorker.RunWorkerAsync();
            //this.pingWorker.RunWorkerAsync();

            }

        // Ping to Beanfun website.
        private void pingWorker_DoWork(object sender, DoWorkEventArgs e)
        {
            Thread.CurrentThread.Name = "ping Worker";
            Console.WriteLine("pingWorker start");
            const int WaitSecs = 60; // 1min
            


            while (!isCancelRequested)
            {
                if (this.pingWorker.CancellationPending)
                {
                    Console.WriteLine("break duo to cancel");
                    break;
                }

                if (this.getOtpWorker.IsBusy || this.loginWorker.IsBusy || this.totpWorker.IsBusy)
                {
                    Console.WriteLine("ping.busy sleep 1s");
                    System.Threading.Thread.Sleep(1000 * 1);
                    continue;
                }

                if (this.bfClient != null) { 
                    this.bfClient.Ping();
                }

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
            this.qrcodeClass = this.bfClient.GetQRCodeValue(skey);
        }

        private void qrWorker_RunWorkerCompleted(object sender, RunWorkerCompletedEventArgs e)
        {
            btn_Region.IsEnabled = true;
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
            qrWorker.RunWorkerAsync(loginPage == null || loginPage.qr == null ? false : true);
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
                    errexit(TryFindResource("MsgBeanfunRejectLogin") as string, 1);
                    break;
                case "-2":
                    Console.WriteLine("登入請求已逾時");
                    NavigateLoginPage();
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
            loginWaitPage.t_Info.Content = TryFindResource("MsgLogging") as string;
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
                            string WvsLoginServerDomain = "tw.login.maplestory.gamania.com";
                            if ("610075_T9".Equals(service_code + "_" + service_region))
                                WvsLoginServerDomain = "tw.loginT.maplestory.gamania.com";
                            tcpClient.Connect(WvsLoginServerDomain, 8484);

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
                    info += $"\r\n{ TryFindResource("ClientVersion") as string }{ ClientMapleMajor }";
                    if (SrvMapleMajor != 0 && SrvMapleMinor.Split(':')[0] != "")
                    {
                        info += $"\r\n{ TryFindResource("ServerVersion") as string }{ SrvMapleMajor }.{ SrvMapleMinor.Split(':')[0] }";
                    }
                }
                bool isCanUpdate = ClientMapleMajor != 0 && SrvMapleMajor != 0 && ClientMapleMajor >= (SrvMapleMajor - 2);
                MessageBoxResult result = MessageBox.Show(
                    string.Format((TryFindResource("MsgKillPatcher") as string).Replace("\\r\\n", "\r\n"), info,
                        isCanUpdate && ClientMapleMajor == SrvMapleMajor ? $"V{ SrvMapleMajor }.{ SrvMapleMinor.Split(':')[0] }fix" : "",
                        isCanUpdate ? TryFindResource("UpdateByPatch") : TryFindResource("UpdateByFullClient"),
                        isCanUpdate ? TryFindResource("GamePatch") : TryFindResource("GameFullClient")), TryFindResource("WarningByBeanfun") as string, MessageBoxButton.YesNo);
                if (result == MessageBoxResult.Yes)
                    Process.Start($"https://maplestory.beanfun.com/download{ (isCanUpdate ? "?download_type=2" : "") }");
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
                    MessageBox.Show(TryFindResource("WrongCaptcha") as string);
                }
                else
                {
                    MessageBox.Show(TryFindResource("WrongAuthInfo") as string);
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
                { MessageBox.Show(I18n.ToSimplified(errmsg)); }
            }
        }

        private void verifyWorker_RunWorkerCompleted(object sender, RunWorkerCompletedEventArgs e)
        {
            if (e.Result != null)
            { do_Login(); }
        }
    }
}
