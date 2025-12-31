using System;
using System.Collections.Generic;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
using System.Windows.Media;

namespace Beanfun
{
    /// <summary>
    /// AccountList.xaml 的交互逻辑
    /// </summary>
    public partial class AccountList : Page
    {
        // drag and drop related variables
        private Point _dragStartPoint;
        private bool _isHandlePressed = false;
        private ListBoxItem _draggedItem = null;
        private int _draggedIndex = -1;

        public AccountList()
        {
            InitializeComponent();

            autoPaste.IsChecked = bool.Parse(ConfigAppSettings.GetValue("autoPaste", "false"));
        }

        private void btn_Logout_Click(object sender, RoutedEventArgs e)
        {
            MessageBoxResult result = MessageBox.Show(TryFindResource("LogoutConfirm") as string, TryFindResource("Logout") as string, MessageBoxButton.YesNo);
            if (result == MessageBoxResult.No) return;
            if (App.LoginMethod == (int)LoginMethod.QRCode) App.MainWnd.loginMethodChanged();
            App.MainWnd.NavigateLoginPage();
        }

        private void list_Account_MouseDoubleClick(object sender, RoutedEventArgs e)
        {
            BeanfunClient.ServiceAccount account = (BeanfunClient.ServiceAccount)list_Account.SelectedItem;
            if (account == null)
                return;
            try
            {
                Clipboard.SetText(account.sid);
            }
            catch { }
        }

        private void Button_Click(object sender, RoutedEventArgs e)
        {
            if (((bool)App.MainWnd.settingPage.tradLogin.IsChecked && App.MainWnd.login_action_type == 1) || App.MainWnd.login_action_type == 0)
            {
                btn_StartGame.IsEnabled = false;
                App.MainWnd.runGame();
                btn_StartGame.IsEnabled = true;
            }
            else
                btnGetOtp_Click(null, null);
        }

        private void autoPaste_CheckedChanged(object sender, RoutedEventArgs e)
        {
            if (bool.Parse(ConfigAppSettings.GetValue("autoPaste", "false")) == autoPaste.IsChecked)
                return;
            if (ConfigAppSettings.GetValue("autoPaste", "") == "")
                MessageBox.Show(TryFindResource("AutoPasteTip") as string);
            ConfigAppSettings.SetValue("autoPaste", Convert.ToString(autoPaste.IsChecked));
        }

        public void btnGetOtp_Click(object sender, RoutedEventArgs e)
        {
            if (list_Account.SelectedIndex < 0 || App.MainWnd.loginWorker.IsBusy)
            {
                MessageBox.Show(TryFindResource("MsgSelectAccount") as string);
                return;
            }

            this.btnGetOtp.Content = TryFindResource("GettingOtp") as string;
            this.t_Password.Text = "";
            this.list_Account.IsEnabled = false;
            this.btnGetOtp.IsEnabled = false;
            this.btn_Logout.IsEnabled = false;
            this.btn_ChangeGame.IsEnabled = false;
            this.gameName.IsEnabled = false;
            this.btn_StartGame.IsEnabled = false;
            this.btnAddServiceAccount.IsEnabled = false;
            this.m_MenuList.IsEnabled = false;
            App.MainWnd.getOtpWorker.RunWorkerAsync(list_Account.SelectedIndex);
        }

        private void t_Password_PreviewMouseLeftButtonDown(object sender, MouseButtonEventArgs e)
        {
            if (t_Password.Text == "" || (string)btnGetOtp.Content == TryFindResource("GettingOtp") as string) return;
            try
            {
                Clipboard.SetText(t_Password.Text);
            }
            catch { }
        }

        private void btnAddServiceAccount_Click(object sender, RoutedEventArgs e)
        {
            if (!btnAddServiceAccount.IsEnabled)
                return;
            if ((string) btnAddServiceAccount.Content == TryFindResource("GoToVerify") as string)
            {
                new WebBrowser("https://tw.beanfun.com/TW/member/verify_index.aspx").Show();
            }
            else
            {
                if (App.MainWnd.service_code == "610153" && App.MainWnd.service_region == "TN" || App.MainWnd.service_code == "610085" && App.MainWnd.service_region == "TC") new UnconnectedGame_AddAccount().ShowDialog();
                else new AddServiceAccount().ShowDialog();
            }
        }

        private void m_UpdatePoint_Click(object sender, RoutedEventArgs e)
        {
            App.MainWnd.updateRemainPoint(App.MainWnd.bfClient.getRemainPoint());
        }

        private void m_ChangeAccName_Click(object sender, RoutedEventArgs e)
        {
            BeanfunClient.ServiceAccount account = (BeanfunClient.ServiceAccount)list_Account.SelectedItem;
            if (account == null)
                return;
            new ChangeServiceAccountDisplayName(account.sname).ShowDialog();
        }

        private void bfb_Gash_Click(object sender, RoutedEventArgs e)
        {
            string url;
            if (App.LoginRegion == "TW")
            {
                url = "https://tw.beanfun.com/TW/";
            }
            else
            {
                url = "https://bfweb.hk.beanfun.com/HK/";
            }
            url += $"auth.aspx?channel=gash&page_and_query=default.aspx%3Fservice_code%3D999999%26service_region%3DT0&web_token={App.MainWnd.bfClient.WebToken}";
            new WebBrowser(url).Show();
        }

        private void BF_btnMember_Click(object sender, RoutedEventArgs e)
        {
            string url;
            if (App.LoginRegion == "TW")
            {
                url = "https://tw.beanfun.com/TW/";
            }
            else
            {
                url = "https://bfweb.hk.beanfun.com/HK/";
            }
            url += $"auth.aspx?channel=member&page_and_query=default.aspx%3Fservice_code%3D999999%26service_region%3DT0&web_token={App.MainWnd.bfClient.WebToken}";
            new WebBrowser(url).Show();
        }

        private void btn_Customerservice_Click(object sender, RoutedEventArgs e)
        {
            string url;
            if (App.LoginRegion == "TW")
            {
                url = "https://tw.beanfun.com/customerservice/www/main.aspx";
            }
            else
            {
                url = "https://bfweb.hk.beanfun.com/newfaq/service_newBF.aspx";
            }
            new WebBrowser(url).Show();
        }

        private void m_GetEmail_Click(object sender, RoutedEventArgs e)
        {
            new CopyBox(TryFindResource("AuthEmail") as string, App.MainWnd.bfClient.getEmail()).ShowDialog();
        }

        private void m_AccInfo_Click(object sender, RoutedEventArgs e)
        {
            BeanfunClient.ServiceAccount account = (BeanfunClient.ServiceAccount)list_Account.SelectedItem;
            if (account == null)
                return;
            new ServiceAccountInfo(account).ShowDialog();
        }

        private void btn_HomePage_Click(object sender, RoutedEventArgs e)
        {
            if (App.MainWnd != null && App.MainWnd.SelectedGame != null)
                new WebBrowser(App.MainWnd.SelectedGame.website_url).Show();
        }

        private void gameName_Click(object sender, RoutedEventArgs e)
        {
            new GameList().ShowDialog();
        }

        private void m_ChangePassword_Click(object sender, RoutedEventArgs e)
        {
            new UnconnectedGame_ChangePassword().ShowDialog();
        }

        public void btn_Tools_Click(object sender, RoutedEventArgs e)
        {
            string gameCode = App.MainWnd.service_code + "_" + App.MainWnd.service_region;
            switch (gameCode)
            {
                case "610074_T9":
                case "610075_T9":
                    new MapleTools().Show();
                    break;
                case "610096_TE":
                    new KartTools().Show();
                    break;
            }
        }

        private void btn_Deposite_Click(object sender, RoutedEventArgs e)
        {
            new WebBrowser("https://m.beanfun.com/Deposite").Show();
        }

        #region Drag and Drop Reorder

        private void ListBox_PreviewMouseLeftButtonDown(object sender, MouseButtonEventArgs e)
        {
            _dragStartPoint = e.GetPosition(null);
            _isHandlePressed = false;
            
            // get the clicked ListBoxItem
            var listBox = sender as ListBox;
            var item = FindAncestor<ListBoxItem>((DependencyObject)e.OriginalSource);
            
            if (item != null)
            {
                _draggedItem = item;
                _draggedIndex = listBox.ItemContainerGenerator.IndexFromContainer(item);
            }
        }

        private void DragHandle_MouseLeftButtonDown(object sender, MouseButtonEventArgs e)
        {
            _isHandlePressed = true;
            _dragStartPoint = e.GetPosition(null);
            
            var element = sender as FrameworkElement;
            var item = FindAncestor<ListBoxItem>(element);
            
            if (item != null)
            {
                _draggedItem = item;
                _draggedIndex = list_Account.ItemContainerGenerator.IndexFromContainer(item);
            }
        }

        private void ListBox_PreviewMouseMove(object sender, MouseEventArgs e)
        {
            if (e.LeftButton != MouseButtonState.Pressed || _draggedItem == null || !_isHandlePressed)
                return;

            Point currentPosition = e.GetPosition(null);
            Vector diff = _dragStartPoint - currentPosition;

            // check if the distance is enough to start dragging
            if (Math.Abs(diff.X) > SystemParameters.MinimumHorizontalDragDistance ||
                Math.Abs(diff.Y) > SystemParameters.MinimumVerticalDragDistance)
            {
                var listBox = sender as ListBox;
                var data = listBox.ItemContainerGenerator.ItemFromContainer(_draggedItem);
                
                if (data != null)
                {
                    DataObject dragData = new DataObject("ServiceAccount", data);
                    DragDrop.DoDragDrop(_draggedItem, dragData, DragDropEffects.Move);
                }
                
                _isHandlePressed = false;
                _draggedItem = null;
            }
        }

        private void ListBox_DragOver(object sender, DragEventArgs e)
        {
            if (!e.Data.GetDataPresent("ServiceAccount"))
            {
                e.Effects = DragDropEffects.None;
                return;
            }

            e.Effects = DragDropEffects.Move;
            
            var listBox = sender as ListBox;
            Point position = e.GetPosition(listBox);
            
            var targetItem = GetItemAtPosition(listBox, position);
            bool isLowerHalf = false;
            
            if (targetItem != null)
            {
                Point itemPosition = e.GetPosition(targetItem);
                isLowerHalf = itemPosition.Y > targetItem.ActualHeight / 2;
            }
            
            for (int i = 0; i < listBox.Items.Count; i++)
            {
                var container = listBox.ItemContainerGenerator.ContainerFromIndex(i) as ListBoxItem;
                if (container != null)
                {
                    if (container == targetItem && container != _draggedItem)
                    {
                        container.BorderBrush = new SolidColorBrush(Colors.DodgerBlue);
                        // Show border on top or bottom based on cursor position
                        container.BorderThickness = isLowerHalf ? new Thickness(0, 0, 0, 2) : new Thickness(0, 2, 0, 0);
                    }
                    else
                    {
                        container.BorderBrush = null;
                        container.BorderThickness = new Thickness(0);
                    }
                }
            }
        }

        private void ListBox_Drop(object sender, DragEventArgs e)
        {
            if (!e.Data.GetDataPresent("ServiceAccount"))
                return;

            var listBox = sender as ListBox;
            var droppedData = e.Data.GetData("ServiceAccount") as BeanfunClient.ServiceAccount;
            
            if (droppedData == null)
                return;

            Point position = e.GetPosition(listBox);
            var targetItem = GetItemAtPosition(listBox, position);
            
            int targetIndex = -1;
            bool insertAfter = false;
            
            if (targetItem != null)
            {
                targetIndex = listBox.ItemContainerGenerator.IndexFromContainer(targetItem);
                
                // Check if dropping on the lower half of the item
                Point itemPosition = e.GetPosition(targetItem);
                if (itemPosition.Y > targetItem.ActualHeight / 2)
                {
                    insertAfter = true;
                }
            }
            else
            {
                // if there is no target item, put it at the end
                targetIndex = listBox.Items.Count;
                insertAfter = false;
            }

            for (int i = 0; i < listBox.Items.Count; i++)
            {
                var container = listBox.ItemContainerGenerator.ContainerFromIndex(i) as ListBoxItem;
                if (container != null)
                {
                    container.BorderBrush = null;
                    container.BorderThickness = new Thickness(0);
                }
            }

            // perform reordering
            if (_draggedIndex != -1 && targetIndex != -1)
            {
                int finalIndex = targetIndex;
                if (insertAfter)
                    finalIndex++;
                
                // Skip if dropping at the same position
                if (_draggedIndex == finalIndex || _draggedIndex == finalIndex - 1 && insertAfter)
                {
                    _draggedItem = null;
                    _draggedIndex = -1;
                    return;
                }
                
                var accountList = App.MainWnd.bfClient.accountList;
                var item = accountList[_draggedIndex];
                accountList.RemoveAt(_draggedIndex);
                
                // Adjust index after removal
                if (_draggedIndex < finalIndex)
                    finalIndex--;
                
                // Clamp to valid range
                if (finalIndex > accountList.Count)
                    finalIndex = accountList.Count;
                if (finalIndex < 0)
                    finalIndex = 0;
                
                accountList.Insert(finalIndex, item);
                
                listBox.ItemsSource = null;
                listBox.ItemsSource = accountList;
                listBox.SelectedIndex = finalIndex;
                
                SaveAccountOrder();
            }

            _draggedItem = null;
            _draggedIndex = -1;
        }

        private ListBoxItem GetItemAtPosition(ListBox listBox, Point position)
        {
            HitTestResult hitTestResult = VisualTreeHelper.HitTest(listBox, position);
            if (hitTestResult == null)
                return null;
            
            return FindAncestor<ListBoxItem>(hitTestResult.VisualHit);
        }

        private static T FindAncestor<T>(DependencyObject current) where T : DependencyObject
        {
            while (current != null)
            {
                if (current is T)
                    return (T)current;
                current = VisualTreeHelper.GetParent(current);
            }
            return null;
        }

        /// <summary>
        /// Save the current game's account sorting order
        /// </summary>
        private void SaveAccountOrder()
        {
            if (App.MainWnd?.bfClient?.accountList == null)
                return;

            string gameCode = App.MainWnd.service_code + "_" + App.MainWnd.service_region;
            var accountList = App.MainWnd.bfClient.accountList;
            
            string orderString = string.Join(",", accountList.ConvertAll(a => a.sid));
            ConfigAppSettings.SetValue("AccountOrder_" + gameCode, orderString);
        }

        /// <summary>
        /// Reorder the account list based on the saved order
        /// </summary>
        public static void ApplyAccountOrder(List<BeanfunClient.ServiceAccount> accountList, string gameCode)
        {
            string orderString = ConfigAppSettings.GetValue("AccountOrder_" + gameCode, "");
            if (string.IsNullOrEmpty(orderString))
                return;

            string[] orderArray = orderString.Split(',');
            
            // create a dictionary to quickly find the account
            var accountDict = new Dictionary<string, BeanfunClient.ServiceAccount>();
            foreach (var account in accountList)
            {
                if (!accountDict.ContainsKey(account.sid))
                    accountDict[account.sid] = account;
            }

            // reorder the account list based on the saved order
            var orderedList = new List<BeanfunClient.ServiceAccount>();
            foreach (string sid in orderArray)
            {
                if (accountDict.ContainsKey(sid))
                {
                    orderedList.Add(accountDict[sid]);
                    accountDict.Remove(sid);
                }
            }

            // add the accounts that are not in the order to the end
            foreach (var account in accountDict.Values)
            {
                orderedList.Add(account);
            }

            // update the original list
            accountList.Clear();
            accountList.AddRange(orderedList);
        }

        #endregion
    }
}
