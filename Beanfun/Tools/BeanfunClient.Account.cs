using Newtonsoft.Json.Linq;
using System.Collections.Specialized;
using System.Net;
using System.Text.RegularExpressions;

namespace Beanfun
{
    partial class BeanfunClient : WebClient
    {
        public class ServiceAccount
        {
            public bool isEnable { get; set; }
            public bool visible { get; set; }
            public bool isinherited { get; set; }
            public string sid { get; set; }
            public string ssn { get; set; }
            public string sname { get; set; }
            public string screatetime { get; set; }
            public string slastusedtime { get; set; }
            public string sauthtype { get; set; }
            
            public ServiceAccount(bool isEnable, string sid, string ssn, string sname, string screatetime)
            {
                this.isEnable = isEnable;
                this.visible = true;
                this.isinherited = false;
                this.sid = sid;
                this.ssn = ssn;
                this.sname = sname;
                this.screatetime = screatetime;
                this.slastusedtime = null;
                this.sauthtype = null;
            }

            public ServiceAccount(bool isEnable, bool visible, bool isinherited, string sid, string ssn, string sname, string screatetime, string slastusedtime, string sauthtype)
            {
                this.isEnable = isEnable;
                this.visible = visible;
                this.isinherited = isinherited;
                this.sid = sid;
                this.ssn = ssn;
                this.sname = sname;
                this.screatetime = screatetime;
                this.slastusedtime = slastusedtime;
                this.sauthtype = sauthtype;
            }
        }

        public void GetAccounts(string service_code, string service_region, bool fatal = true)
        {
            if (this.webtoken == null) return;

            Regex regex;

            this.DownloadString("https://tw.beanfun.com/beanfun_block/auth.aspx?channel=game_zone&page_and_query=game_start.aspx%3Fservice_code_and_region%3D" + service_code + "_" + service_region + "&web_token=" + webtoken);

            string response = this.DownloadString("https://tw.beanfun.com/beanfun_block/game_zone/game_server_account_list.aspx?sc=" + service_code + "&sr=" + service_region + "&dt=" + GetCurrentTime(2));

            // Add account list to ListView.
            regex = new Regex("onclick=\"([^\"]*)\"><div id=\"(\\w+)\" sn=\"(\\d+)\" name=\"([^\"]+)\"");
            this.accountList.Clear();
            foreach (Match match in regex.Matches(response))
            {
                if (match.Groups[2].Value == "" || match.Groups[3].Value == "" || match.Groups[4].Value == "")
                { continue; }
                this.accountList.Add(new ServiceAccount(match.Groups[1].Value != "", match.Groups[2].Value, match.Groups[3].Value, WebUtility.HtmlDecode(match.Groups[4].Value), GetCreateTime(service_code, service_region, match.Groups[3].Value)));
            }

            regex = new Regex("<div id=\"divServiceAccountAmountLimitNotice\" class=\"InnerContent\">(.*)</div>");
            if (regex.IsMatch(response))
            {
                accountAmountLimitNotice = regex.Match(response).Groups[1].Value;
                if (accountAmountLimitNotice.Contains("進階認證"))
                    accountAmountLimitNotice = System.Windows.Application.Current.TryFindResource("AuthReLogin") as string;
                else
                    accountAmountLimitNotice = I18n.ToSimplified(accountAmountLimitNotice);
            }
            else accountAmountLimitNotice = "";

            if (this.accountList.Count > 0) this.accountList.Sort((x, y) => { return x.ssn.CompareTo(y.ssn); });

            this.errmsg = null;
        }

        private string GetCreateTime(string service_code, string service_region, string sn)
        {
            try
            {
                string response = this.DownloadString("https://tw.beanfun.com/beanfun_block/game_zone/game_start_step2.aspx?service_code=" + service_code + "&service_region=" + service_region + "&sotp=" + sn + "&dt=" + GetCurrentTime(2));
                Regex regex = new Regex("ServiceAccountCreateTime: \"([^\"]+)\"");
                if (!regex.IsMatch(response))
                { return null; }
                return regex.Match(response).Groups[1].Value;
            }
            catch
            {
                return null;
            }
        }

        public void GetAccounts_HK(string service_code, string service_region, bool fatal = true)
        {
            if (this.bfServ == null) return;

            this.DownloadString("https://hk.beanfun.com/beanfun_block/auth.aspx?channel=game_zone&page_and_query=game_start.aspx%3Fservice_code_and_region%3D" + service_code + "_" + service_region + "&token=" + this.bfServ.Token);

            string response = this.DownloadString("https://hk.beanfun.com/beanfun_block/game_zone/game_server_account_list.aspx?service_code=" + service_code + "&service_region=" + service_region);

            // Add account list to ListView.
            Regex regex = new Regex(@"<li class=""([^""]*)"" title=""([^""]*)"" onclick=""StartGame\('" + service_code + "', '" + service_region + @"', '(\w+)', '(\d+)', '([^""]*)', '(\w+)', '([^""]+)'\)");
            this.accountList.Clear();
            foreach (Match match in regex.Matches(response))
            {
                if (match.Groups[3].Value == "" || match.Groups[4].Value == "" || match.Groups[5].Value == "" || match.Groups[7].Value == "")
                { continue; }
                Match mTime = new Regex(@"(\d+)/(\d+)/(\d+) (.*)").Match(match.Groups[7].Value);
                this.accountList.Add(new ServiceAccount(!match.Groups[1].Value.ToLower().Equals("stop"), match.Groups[3].Value, match.Groups[4].Value, WebUtility.HtmlDecode(match.Groups[5].Value), $"{mTime.Groups[3].Value}-{mTime.Groups[2].Value}-{mTime.Groups[1].Value} {mTime.Groups[4].Value}"));
            }

            regex = new Regex("<div id=\"divServiceAccountAmountLimitNotice\" class=\"InnerContent\">(.*)</div>");
            if (regex.IsMatch(response))
                accountAmountLimitNotice = I18n.ToSimplified(regex.Match(response).Groups[1].Value);
            else
                accountAmountLimitNotice = "";

            if (this.accountList.Count > 0) this.accountList.Sort((x, y) => { return x.ssn.CompareTo(y.ssn); });

            this.errmsg = null;
        }

        private NameValueCollection UnconnectedGame_InitAccountPayload(string service_code, string service_region)
        {
            string strUrl;
            string response;
            if (App.LoginRegion == "TW")
            {
                strUrl = "https://tw.beanfun.com/TW/auth.aspx?channel=accounts_management&page_and_query=01.aspx%3FServiceCode%3D" + service_code + "%26ServiceRegion%3D" + service_region + "&web_token=" + webtoken;
                response = this.DownloadString(strUrl);
            }
            else
            {
                strUrl = "http://hk.beanfun.com/beanfun_web_ap/auth.aspx?channel=accounts_management&page_and_query=01.aspx%3FServiceCode%3D" + service_code + "%26ServiceRegion%3D" + service_region + "&token=" + this.bfServ.Token;
                response = this.DownloadString(strUrl);
            }

            Regex regex = new Regex("id=\"__VIEWSTATE\" value=\"(.*)\" />");
            if (!regex.IsMatch(response))
            { this.errmsg = "LoginNoViewstate"; return null; }
            string viewstate = regex.Match(response).Groups[1].Value;
            regex = new Regex("id=\"__VIEWSTATEGENERATOR\" value=\"(.*)\" />");
            if (!regex.IsMatch(response))
            { this.errmsg = "LoginNoViewstategenerator"; return null; }
            string viewstategenerator = regex.Match(response).Groups[1].Value;

            NameValueCollection payload = new NameValueCollection();
            payload.Add("__VIEWSTATE", viewstate);
            payload.Add("__VIEWSTATEGENERATOR", viewstategenerator);

            return payload;
        }

        public NameValueCollection UnconnectedGame_InitAddAccountPayload(string service_code, string service_region)
        {
            NameValueCollection payload = UnconnectedGame_InitAccountPayload(service_code, service_region);
            payload.Add("__EVENTTARGET", "");
            payload.Add("__EVENTARGUMENT", "");
            payload.Add("imgbtn_AddAccount.x", "0");
            payload.Add("imgbtn_AddAccount.y", "0");

            string response;
            if (App.LoginRegion == "TW")
                response = this.UploadString("https://tw.beanfun.com/TW/accounts_management/02.aspx", payload);
            else
                response = this.UploadString("http://hk.beanfun.com/beanfun_web_ap/accounts_management/02.aspx", payload);

            Regex regex = new Regex("id=\"__VIEWSTATE\" value=\"(.*)\" />");
            if (!regex.IsMatch(response))
            { this.errmsg = "LoginNoViewstate"; return null; }
            string viewstate = regex.Match(response).Groups[1].Value;
            regex = new Regex("id=\"__VIEWSTATEGENERATOR\" value=\"(.*)\" />");
            if (!regex.IsMatch(response))
            { this.errmsg = "LoginNoViewstategenerator"; return null; }
            string viewstategenerator = regex.Match(response).Groups[1].Value;
            regex = new Regex("id=\"__EVENTVALIDATION\" value=\"(.*)\" />");
            if (!regex.IsMatch(response))
            { this.errmsg = "LoginNoEventvalidation"; return null; }
            string eventvalidation = regex.Match(response).Groups[1].Value;

            payload.Clear();
            payload.Add("__VIEWSTATE", viewstate);
            payload.Add("__VIEWSTATEGENERATOR", viewstategenerator);
            payload.Add("__EVENTVALIDATION", eventvalidation);

            regex = new Regex("<span id=\"lblGameName\">(.*)</span>");
            if (!regex.IsMatch(response))
            { this.errmsg = "LoginNoGameName"; return null; }
            payload.Add("GameName", regex.Match(response).Groups[1].Value);

            regex = new Regex("<span id=\"lblAccountLen\">(.*)</span>");
            if (!regex.IsMatch(response))
            { this.errmsg = "LoginNoAccountLen"; return null; }
            payload.Add("AccountLen", regex.Match(response).Groups[1].Value);
            payload.Add("CheckNickName", response.Contains("<a id=\"lbtnCheckNickName\"") ? "1" : "");

            return payload;
        }

        public NameValueCollection UnconnectedGame_AddAccountCheck(string service_code, string service_region, string name, string txtServiceAccountDN, NameValueCollection payload)
        {
            if (payload == null) return null;
            payload.Add("__EVENTTARGET", "lbtnCheckAccount");
            payload.Add("__EVENTARGUMENT", "");
            payload.Add("txtServiceAccountID", name);
            if (txtServiceAccountDN != null)
            {
                if (App.LoginRegion == "TW") payload.Add("t1", txtServiceAccountDN);
                else payload.Add("txtServiceAccountDN", txtServiceAccountDN);
            }
            payload.Add("txtNewPwd", "");
            payload.Add("txtNewPwd2", "");
            string response;
            if (App.LoginRegion == "TW") response = this.UploadString("https://tw.beanfun.com/TW/accounts_management/02.aspx", payload);
            else response = this.UploadString("http://hk.beanfun.com/beanfun_web_ap/accounts_management/02.aspx", payload);

            Regex regex = new Regex("id=\"__VIEWSTATE\" value=\"(.*)\" />");
            if (!regex.IsMatch(response))
            { this.errmsg = "LoginNoViewstate"; return null; }
            string viewstate = regex.Match(response).Groups[1].Value;
            regex = new Regex("id=\"__VIEWSTATEGENERATOR\" value=\"(.*)\" />");
            if (!regex.IsMatch(response))
            { this.errmsg = "LoginNoViewstategenerator"; return null; }
            string viewstategenerator = regex.Match(response).Groups[1].Value;
            regex = new Regex("id=\"__EVENTVALIDATION\" value=\"(.*)\" />");
            if (!regex.IsMatch(response))
            { this.errmsg = "LoginNoEventvalidation"; return null; }
            string eventvalidation = regex.Match(response).Groups[1].Value;

            payload.Clear();
            payload.Add("__VIEWSTATE", viewstate);
            payload.Add("__VIEWSTATEGENERATOR", viewstategenerator);
            payload.Add("__EVENTVALIDATION", eventvalidation);

            regex = new Regex("<span id=\"lblErrorMessage\" style=\"color:Red;\">(.*)</span>");
            payload.Add("lblErrorMessage", regex.IsMatch(response) ? regex.Match(response).Groups[1].Value : "");

            return payload;
        }

        public NameValueCollection UnconnectedGame_AddAccountCheckNickName(string service_code, string service_region, string txtServiceAccountDN, NameValueCollection payload)
        {
            if (payload == null) return null;
            payload.Add("__EVENTTARGET", "lbtnCheckNickName");
            payload.Add("__EVENTARGUMENT", "");
            payload.Add("txtServiceAccountID", "");
            if (txtServiceAccountDN != null)
            {
                if (App.LoginRegion == "TW") payload.Add("t1", txtServiceAccountDN);
                else payload.Add("txtServiceAccountDN", txtServiceAccountDN);
            }
            payload.Add("txtNewPwd", "");
            payload.Add("txtNewPwd2", "");
            string response;
            if (App.LoginRegion == "TW") response = this.UploadString("https://tw.beanfun.com/TW/accounts_management/02.aspx", payload);
            else response = this.UploadString("http://hk.beanfun.com/beanfun_web_ap/accounts_management/02.aspx", payload);

            Regex regex = new Regex("id=\"__VIEWSTATE\" value=\"(.*)\" />");
            if (!regex.IsMatch(response))
            { this.errmsg = "LoginNoViewstate"; return null; }
            string viewstate = regex.Match(response).Groups[1].Value;
            regex = new Regex("id=\"__VIEWSTATEGENERATOR\" value=\"(.*)\" />");
            if (!regex.IsMatch(response))
            { this.errmsg = "LoginNoViewstategenerator"; return null; }
            string viewstategenerator = regex.Match(response).Groups[1].Value;
            regex = new Regex("id=\"__EVENTVALIDATION\" value=\"(.*)\" />");
            if (!regex.IsMatch(response))
            { this.errmsg = "LoginNoEventvalidation"; return null; }
            string eventvalidation = regex.Match(response).Groups[1].Value;

            payload.Clear();
            payload.Add("__VIEWSTATE", viewstate);
            payload.Add("__VIEWSTATEGENERATOR", viewstategenerator);
            payload.Add("__EVENTVALIDATION", eventvalidation);

            regex = new Regex("<span id=\"lblErrorMessage\" style=\"color:Red;\">(.*)</span>");
            payload.Add("lblErrorMessage", regex.IsMatch(response) ? regex.Match(response).Groups[1].Value : "");

            return payload;
        }

        public string UnconnectedGame_AddAccount(string service_code, string service_region, string name, string txtNewPwd, string txtNewPwd2, string txtServiceAccountDN, NameValueCollection payload)
        {
            if (name == null || name == "")
                return null;
            if (txtNewPwd == null || txtNewPwd == "")
                return null;
            if (txtNewPwd2 == null || txtNewPwd2 == "")
                return null;
            if (payload == null) return null;

            payload.Add("__EVENTTARGET", "");
            payload.Add("__EVENTARGUMENT", "");
            payload.Add("txtServiceAccountID", name);
            if (txtServiceAccountDN != null)
            {
                if (App.LoginRegion == "TW") payload.Add("t1", txtServiceAccountDN);
                else payload.Add("txtServiceAccountDN", txtServiceAccountDN);
            }
            payload.Add("txtNewPwd", txtNewPwd);
            payload.Add("txtNewPwd2", txtNewPwd2);
            payload.Add("chkBox1", "on");
            payload.Add("imgbtn_Submit.x", "0");
            payload.Add("imgbtn_Submit.y", "0");

            string response;
            if (App.LoginRegion == "TW") response = this.UploadString("https://tw.beanfun.com/TW/accounts_management/02.aspx", payload);
            else response = this.UploadString("http://hk.beanfun.com/beanfun_web_ap/accounts_management/02.aspx", payload);
            Regex regex = new Regex("<span id=\"lblErrorMessage\" style=\"color:Red;\">(.*)</span>");

            return regex.IsMatch(response) ? regex.Match(response).Groups[1].Value : "";
        }

        public string UnconnectedGame_ChangePassword(string service_code, string service_region, int num, string txtEmail)
        {
            UnconnectedGame_InitAccountPayload(service_code, service_region);

            string response;
            if (App.LoginRegion == "TW") response = this.DownloadString("https://tw.beanfun.com/TW/accounts_management/01Accounts.aspx");
            else response = this.DownloadString("http://hk.beanfun.com/beanfun_web_ap/accounts_management/01Accounts.aspx");

            Regex regex = new Regex("id=\"__VIEWSTATE\" value=\"(.*)\" />");
            if (!regex.IsMatch(response))
            { this.errmsg = "LoginNoViewstate"; return null; }
            string viewstate = regex.Match(response).Groups[1].Value;
            regex = new Regex("id=\"__VIEWSTATEGENERATOR\" value=\"(.*)\" />");
            if (!regex.IsMatch(response))
            { this.errmsg = "LoginNoViewstategenerator"; return null; }
            string viewstategenerator = regex.Match(response).Groups[1].Value;
            regex = new Regex("id=\"__EVENTVALIDATION\" value=\"(.*)\" />");
            if (!regex.IsMatch(response))
            { this.errmsg = "LoginNoEventvalidation"; return null; }
            string eventvalidation = regex.Match(response).Groups[1].Value;

            NameValueCollection payload = new NameValueCollection();
            payload.Add("__VIEWSTATE", viewstate);
            payload.Add("__VIEWSTATEGENERATOR", viewstategenerator);
            payload.Add("__EVENTVALIDATION", eventvalidation);
            payload.Add("__EVENTTARGET", "gvServiceAccountList");
            payload.Add("__EVENTARGUMENT", "ChangePassword$" + num);
            payload.Add("x", "0");
            payload.Add("y", "0");

            if (App.LoginRegion == "TW")
            {
                response = this.UploadString("https://tw.beanfun.com/TW/accounts_management/01Accounts.aspx", payload);
                response = this.DownloadString("https://tw.beanfun.com/TW/accounts_management/03.aspx");
            }
            else
            {
                response = this.UploadString("http://hk.beanfun.com/beanfun_web_ap/accounts_management/01Accounts.aspx", payload);
                response = this.DownloadString("http://hk.beanfun.com/beanfun_web_ap/accounts_management/03.aspx");
            }

            regex = new Regex("id=\"__VIEWSTATE\" value=\"(.*)\" />");
            if (!regex.IsMatch(response))
            { this.errmsg = "LoginNoViewstate"; return null; }
            viewstate = regex.Match(response).Groups[1].Value;
            regex = new Regex("id=\"__VIEWSTATEGENERATOR\" value=\"(.*)\" />");
            if (!regex.IsMatch(response))
            { this.errmsg = "LoginNoViewstategenerator"; return null; }
            viewstategenerator = regex.Match(response).Groups[1].Value;
            regex = new Regex("id=\"__EVENTVALIDATION\" value=\"(.*)\" />");
            if (!regex.IsMatch(response))
            { this.errmsg = "LoginNoEventvalidation"; return null; }
            eventvalidation = regex.Match(response).Groups[1].Value;
            
            payload.Clear();
            payload.Add("__VIEWSTATE", viewstate);
            payload.Add("__VIEWSTATEGENERATOR", viewstategenerator);
            payload.Add("__EVENTVALIDATION", eventvalidation);
            payload.Add("txtEmail", txtEmail);
            payload.Add("imgbtn_Submit.x", "0");
            payload.Add("imgbtn_Submit.y", "0");

            regex = new Regex("<span id=\"lblErrorMessage\" style=\"color:Red;\">(.*)</span>");
            if (App.LoginRegion == "TW")
            {
                response = this.UploadString("https://tw.beanfun.com/TW/accounts_management/03.aspx", payload);

                string lblErrorMessage = regex.IsMatch(response) ? regex.Match(response).Groups[1].Value : "";
                if (lblErrorMessage != "") return lblErrorMessage;

                regex = new Regex("verify_code=(.*)");
                return regex.IsMatch(this.ResponseUri.ToString()) ? ("verify_code" + regex.Match(this.ResponseUri.ToString()).Groups[1].Value) : null;
            }
            else
            {
                response = this.UploadString("http://hk.beanfun.com/beanfun_web_ap/accounts_management/03.aspx", payload);

                string lblErrorMessage = regex.IsMatch(response) ? regex.Match(response).Groups[1].Value : null;
                if (lblErrorMessage == null || lblErrorMessage == "") return null;

                regex = new Regex("確認碼：(.*)");
                return regex.IsMatch(lblErrorMessage) ? ("verify_code" + regex.Match(lblErrorMessage).Groups[1].Value) : lblErrorMessage;
            }
        }

        public bool AddServiceAccount(string name, string service_code, string service_region)
        {
            if (name == null || name == "")
                return false;
            NameValueCollection payload = new NameValueCollection();
            payload.Add("strFunction", "AddServiceAccount");
            payload.Add("npsc", "");
            payload.Add("npsr", "");
            payload.Add("sc", service_code);
            payload.Add("sr", service_region);
            payload.Add("sadn", name);
            payload.Add("sag", "");

            string response = this.UploadString($"https://{ App.LoginRegion.ToLower() }.beanfun.com/{ (App.LoginRegion == "HK" ? "beanfun_block/" : "") }generic_handlers/gamezone.ashx", payload);
            if (response == "") return false;
            JObject jsonData = JObject.Parse(response);
            if (jsonData["intResult"] == null || (int)jsonData["intResult"] != 1)
                return false;
            else
                return true;
        }

        public bool ChangeServiceAccountDisplayName(string newName, string gameCode, ServiceAccount account)
        {
            if (newName == null || newName == "" || account == null || newName == account.sname)
            {
                return false;
            }
            NameValueCollection payload = new NameValueCollection();
            payload.Add("strFunction", "ChangeServiceAccountDisplayName");
            payload.Add("sl", gameCode);
            payload.Add("said", account.sid);
            payload.Add("nsadn", newName);

            string response = this.UploadString($"https://{ App.LoginRegion.ToLower() }.beanfun.com/{ (App.LoginRegion == "HK" ? "beanfun_block/" : "") }generic_handlers/gamezone.ashx", payload);
            if (response == "") return false;
            JObject jsonData = JObject.Parse(response);
            if (jsonData["intResult"] == null || (int)jsonData["intResult"] != 1)
                return false;
            else
                return true;
        }

        public string GetServiceContract(string service_code, string service_region)
        {
            NameValueCollection payload = new NameValueCollection();
            payload.Add("strFunction", "GetServiceContract");
            payload.Add("sc", service_code);
            payload.Add("sr", service_region);
            string response = this.UploadStringGZip($"https://{ App.LoginRegion.ToLower() }.beanfun.com/{ (App.LoginRegion == "HK" ? "beanfun_block/" : "") }generic_handlers/gamezone.ashx", payload);
            if (response == "") return "";
            JObject jsonData = JObject.Parse(response);
            if (jsonData["intResult"] == null || (int)jsonData["intResult"] != 1)
                return "";
            else
                return (string)jsonData["strResult"];
        }
    }
}
