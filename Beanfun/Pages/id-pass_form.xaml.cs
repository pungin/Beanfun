using System.Collections.Generic;
using System.Text.RegularExpressions;
using System.Windows;
using System.Windows.Controls;

namespace Beanfun
{
    /// <summary>
    /// id_pass_form.xaml 的交互逻辑
    /// </summary>
    public partial class id_pass_form : Page
    {
        public id_pass_form()
        {
            InitializeComponent();

            this.Loaded += (sender, e) =>
            {
                var tb = t_AccountID.Template.FindName("PART_EditableTextBox", t_AccountID) as TextBox;
                if (tb != null) System.Windows.Input.InputMethod.SetPreferredImeState(tb, System.Windows.Input.InputMethodState.Off);
            };
        }

        private void checkBox_RememberPWD_Unchecked(object sender, RoutedEventArgs e)
        {
            checkBox_AutoLogin.IsChecked = false;
        }

        private void checkBox_AutoLogin_Checked(object sender, RoutedEventArgs e)
        {
            checkBox_RememberPWD.IsChecked = true;
        }

        private void RegAcc_Click(object sender, RoutedEventArgs e)
        {
            string url;
            if (App.LoginRegion == "TW")
            {
                url = "https://tw.beanfun.com/TW/signup/Join_beanfun_signup.aspx?service=999999_T0";
            }
            else
            {
                url = "https://bfweb.hk.beanfun.com/beanfun_web_ap/signup/preregistration.aspx?service=999999_T0";
            }
            new WebBrowser(url).Show();
        }

        private void FindPwd_Click(object sender, RoutedEventArgs e)
        {
            string url;
            if (App.LoginRegion == "TW")
            {
                url = "https://tw.beanfun.com/member/forgot_pwd.aspx";
            }
            else
            {
                url = "https://bfweb.hk.beanfun.com/member/forgot_pwd.aspx";
            }
            new WebBrowser(url).Show();
        }

        private void btn_login_Click(object sender, RoutedEventArgs e)
        {
            if (t_AccountID.Text == null || t_AccountID.Text == "")
            {
                MessageBox.Show(TryFindResource("AccountNeed") as string);
                return;
            }
            if (t_Password.Password == null || t_Password.Password == "")
            {
                MessageBox.Show(TryFindResource("PasswordNeed") as string);
                return;
            }
            //System.Console.WriteLine("PW" + t_Password.Password);
            App.MainWnd.do_Login();
        }

        private void Button_Click(object sender, RoutedEventArgs e)
        {
            new GameList().ShowDialog();
        }

        private void t_AccountID_TextChanged(object sender, System.EventArgs e)
        {
            TextBox tb = t_AccountID.Template.FindName("PART_EditableTextBox", t_AccountID) as TextBox;
            int caretIndex = 0;
            if (tb != null) caretIndex = tb.CaretIndex;

            string tbAccount = t_AccountID.Text;

            Regex regex = new Regex(@"\((.*)\)");
            if (regex.IsMatch(tbAccount))
            {
                t_AccountID.Text = regex.Match(tbAccount).Groups[1].Value;
                tb.Text = t_AccountID.Text;
            }

            List<string> searches = new List<string>();
            List<string> accList = new List<string>();
            string[] accArr = App.MainWnd.accountManager.getAccountList(App.LoginRegion);

            bool IsFind = false;
            foreach (string s in accArr)
            {
                if (s == t_AccountID.Text)
                {
                    IsFind = true;
                }
                accList.Add(s);
            }

            searches = accList.FindAll(delegate (string s) { return s.Contains(t_AccountID.Text.Trim()); });

            for (int i = 0; i < accList.Count; i++)
            {
                string name = App.MainWnd.accountManager.getNameByAccount(App.LoginRegion, accList[i]);
                if (name != null && name != "")
                {
                    if (tbAccount == accList[i]) tbAccount = name + "(" + accList[i] + ")";
                    accList[i] = name + "(" + accList[i] + ")";
                }
            }

            for (int i = 0; i < searches.Count; i++)
            {
                string name = App.MainWnd.accountManager.getNameByAccount(App.LoginRegion, searches[i]);
                if (name != null && name != "")
                {
                    searches[i] = name + "(" + searches[i] + ")";
                }
            }

            if (!IsFind && t_AccountID.Text != "" && searches.Count > 0)
            {
                t_AccountID.IsDropDownOpen = true;
                t_AccountID.ItemsSource = null;
                t_AccountID.ItemsSource = searches;
                t_AccountID.SelectedIndex = -1;
                t_AccountID.Text = tbAccount;
            }
            else
            {
                t_AccountID.ItemsSource = null;
                t_AccountID.ItemsSource = accList;
                if (!IsFind)
                {
                    t_AccountID.SelectedIndex = -1;
                    t_AccountID.Text = tbAccount;
                }
                t_AccountID.IsDropDownOpen = false;

                if (IsFind)
                {
                    if (accList.Count > 0) t_AccountID.SelectedItem = tbAccount;

                    t_Password.Password = "";
                    checkBox_RememberPWD.IsChecked = false;

                    int loginMethod = App.MainWnd.accountManager.getMethodByAccount(App.LoginRegion, t_AccountID.Text);
                    if (loginMethod > -1)
                        App.LoginMethod = loginMethod;
                    App.MainWnd.loginMethodChanged();
                }
            }

            if (tb != null) tb.CaretIndex = caretIndex;
        }

        private void t_AccountID_GotFocus(object sender, RoutedEventArgs e)
        {
            var tb = t_AccountID.Template.FindName("PART_EditableTextBox", t_AccountID) as TextBox;
            if (tb != null)
            {
                tb.CaretIndex = tb.Text.Length;
            }
        }

        private void t_AccountID_DropDown(object sender, System.EventArgs e)
        {
            var tb = t_AccountID.Template.FindName("PART_EditableTextBox", t_AccountID) as TextBox;
            if (tb != null)
            {
                string tbAccount = t_AccountID.Text;
                string name = App.MainWnd.accountManager.getNameByAccount(App.LoginRegion, tbAccount);
                if (name != null && name != "") tbAccount = name + "(" + tbAccount + ")";
                if ((t_AccountID.ItemsSource as List<string>).FindAll(delegate (string s) { return s.Equals(tbAccount.Trim()); }).Count <= 0)
                    tb.SelectionLength = 0;
                else
                    tb.CaretIndex = tb.Text.Length;
            }
        }

        private void DeleteButton_Click(object sender, RoutedEventArgs e)
        {
            string t_AccID = t_AccountID.Text;
            Button closeButton = sender as Button;
            string t_AccID_toDelete = (string)closeButton.Tag;

            Regex regex = new Regex(@"\((.*)\)");
            if (regex.IsMatch(t_AccID_toDelete)) t_AccID_toDelete = regex.Match(t_AccID_toDelete).Groups[1].Value;

            MessageBoxResult result = MessageBox.Show(string.Format(TryFindResource("MsgDeleteAccount") as string, t_AccID_toDelete), TryFindResource("DeleteAccount") as string, MessageBoxButton.YesNo);

            if (result == MessageBoxResult.Yes)
            {
                App.MainWnd.accountManager.removeAccount(App.LoginRegion, t_AccID_toDelete);
                App.MainWnd.loginMethodInit();

                foreach (string s in t_AccountID.Items)
                {
                    string str = s;
                    if (regex.IsMatch(str)) str = regex.Match(str).Groups[1].Value;
                    if (t_AccID == str)
                    {
                        t_AccountID.SelectedItem = str;
                        break;
                    }
                }
            }
        }

        private void btn_QRCode_Click(object sender, RoutedEventArgs e)
        {
            App.LoginMethod = (int)LoginMethod.QRCode;
            App.MainWnd.loginMethodChanged();
        }
    }
}
