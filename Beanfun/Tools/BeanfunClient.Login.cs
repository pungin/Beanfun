using System;
using System.Linq;
using System.Text;
using System.Net;
using System.Text.RegularExpressions;
using System.Collections.Specialized;
using System.Diagnostics;
using System.IO;
using Newtonsoft.Json.Linq;
using System.Windows.Media.Imaging;

namespace Beanfun
{
    public partial class BeanfunClient : WebClient
    {

        private string RegularLogin_HK(string id, string pass, string otp1)
        {
            try
            {
                string response = this.DownloadString($"http://hk.beanfun.com/beanfun_block/login/id-pass_form.aspx?otp1={ otp1 }&seed=0");
                Regex regex = new Regex("id=\"__VIEWSTATE\" value=\"(.*)\" />");
                if (!regex.IsMatch(response))
                { this.errmsg = "LoginNoViewstate"; return null; }
                string viewstate = regex.Match(response).Groups[1].Value;

                regex = new Regex("id=\"__EVENTVALIDATION\" value=\"(.*)\" />");
                if (!regex.IsMatch(response))
                { this.errmsg = "LoginNoEventvalidation"; return null; }
                string eventvalidation = regex.Match(response).Groups[1].Value;
                regex = new Regex("id=\"__VIEWSTATEGENERATOR\" value=\"(.*)\" />");
                if (!regex.IsMatch(response))
                { this.errmsg = "LoginNoViewstateGenerator"; return null; }
                string viewstateGenerator = regex.Match(response).Groups[1].Value;

                NameValueCollection payload = new NameValueCollection();
                payload.Add("__EVENTTARGET", "");
                payload.Add("__EVENTARGUMENT", "");
                payload.Add("__VIEWSTATE", viewstate);
                payload.Add("__VIEWSTATEGENERATOR", viewstateGenerator);
                payload.Add("__EVENTVALIDATION", eventvalidation);
                payload.Add("t_AccountID", id);
                payload.Add("t_Password", pass);
                payload.Add("btn_login.x", "0");
                payload.Add("btn_login.y", "0");
                payload.Add("recaptcha_response_field", "manual_challenge");

                response = this.UploadString($"http://hk.beanfun.com/beanfun_block/login/id-pass_form.aspx?otp1={ otp1 }&seed=0", payload);

                regex = new Regex("ProcessLoginV2\\((.*)\\);\\\"");
                if (!regex.IsMatch(response))
                { this.errmsg = "LoginNoProcessLoginV2JSON"; return null; }
                string json = regex.Match(response).Groups[1].Value.Replace("\\", "");
                bfServ.Token = (string)JObject.Parse(json)["token"];
                Debug.WriteLine(json);
                return "true";
            }
            catch (Exception e)
            {
                this.errmsg = "LoginUnknown\n\n" + e.Message + "\n" + e.StackTrace;
                return null;
            }
        }

        private string RegularLogin(string id, string pass, string skey)
        {
            try
            {
                string response = this.DownloadString("https://tw.newlogin.beanfun.com/login/id-pass_form.aspx?skey=" + skey);
                Regex regex = new Regex("id=\"__VIEWSTATE\" value=\"(.*)\" />");
                if (!regex.IsMatch(response))
                    {this.errmsg = "LoginNoViewstate"; return null;}
                string viewstate = regex.Match(response).Groups[1].Value;

                regex = new Regex("id=\"__EVENTVALIDATION\" value=\"(.*)\" />");
                if (!regex.IsMatch(response))
                    { this.errmsg = "LoginNoEventvalidation"; return null; }
                string eventvalidation = regex.Match(response).Groups[1].Value;
                regex = new Regex("id=\"__VIEWSTATEGENERATOR\" value=\"(.*)\" />");
                if (!regex.IsMatch(response))
                { this.errmsg = "LoginNoViewstateGenerator"; return null; }
                string viewstateGenerator = regex.Match(response).Groups[1].Value;
                regex = new Regex("id=\"LBD_VCID_c_login_idpass_form_samplecaptcha\" value=\"(.*)\" />");
                if (!regex.IsMatch(response))
                { this.errmsg = "LoginNoSamplecaptcha"; return null; }
                string samplecaptcha = regex.Match(response).Groups[1].Value;

                string Captcha = "";
                regex = new Regex("isHideCaptcha\\s?=\\s?false");
                if (regex.IsMatch(response))
                {
                    CaptchaWnd wnd = null;
                    (wnd = new CaptchaWnd(this, samplecaptcha)).ShowDialog();
                    if (wnd == null) { this.errmsg = "LoginInitCaptcha"; return null; }
                    else Captcha = wnd.Captcha;
                }

                NameValueCollection payload = new NameValueCollection();
                payload.Add("__EVENTTARGET", "");
                payload.Add("__EVENTARGUMENT", "");
                payload.Add("__VIEWSTATE", viewstate);
                payload.Add("__VIEWSTATEGENERATOR", viewstateGenerator);
                payload.Add("__EVENTVALIDATION", eventvalidation);
                payload.Add("t_AccountID", id);
                payload.Add("t_Password", pass);
                payload.Add("CodeTextBox", Captcha);
                payload.Add("LBD_VCID_c_login_idpass_form_samplecaptcha", samplecaptcha);
                //payload.Add("g-recaptcha-response", samplecaptcha);
                payload.Add("btn_login", "登入");

                response = this.UploadString("https://tw.newlogin.beanfun.com/login/id-pass_form.aspx?skey=" + skey, payload);
                if (response.Contains("RELOAD_CAPTCHA_CODE") && response.Contains("alert"))
                { this.errmsg = "LoginAdvanceCheck"; return null; }

                regex = new Regex("akey=(.*)");
                if (!regex.IsMatch(this.ResponseUri.ToString()))
                {
                    this.errmsg = "LoginNoAkey";
                    regex = new Regex("<script type=\"text/javascript\">\\$\\(function\\(\\){MsgBox.Show\\('(.*)'\\);}\\);</script>");
                    if (regex.IsMatch(response))
                    { this.errmsg = regex.Match(response).Groups[1].Value; }
                    else
                    {
                        regex = new Regex("pollRequest\\(\"([^\"]*)\",\"(\\w+)\",\"([^\"]+)\"\\);");
                        if (regex.IsMatch(response))
                        { this.errmsg = regex.Match(response).Groups[1].Value + "\",\"" + regex.Match(response).Groups[3].Value; LoginToken = regex.Match(response).Groups[2].Value; }
                    }
                    return null;
                }
                string akey = regex.Match(this.ResponseUri.ToString()).Groups[1].Value;

                return akey;
            }
            catch (Exception e)
            {
                this.errmsg = "LoginUnknown\n\n" + e.Message + "\n" + e.StackTrace;
                return null;
            }
        }

        public class QRCodeClass
        {
            public string skey;
            public string value;
            public string viewstate;
            public string eventvalidation;
            public string bitmapUrl;
            public bool oldAppQRCode;
        }

        public QRCodeClass GetQRCodeValue(string skey, bool oldAppQRCode)
        {
            string response = this.DownloadString("https://tw.newlogin.beanfun.com/login/qr_form.aspx?skey=" + skey );
            Regex regex = new Regex("id=\"__VIEWSTATE\" value=\"(.*)\" />");
            if (!regex.IsMatch(response))
            { this.errmsg = "LoginNoViewstate"; return null; }
            string viewstate = regex.Match(response).Groups[1].Value;

            regex = new Regex("id=\"__EVENTVALIDATION\" value=\"(.*)\" />");
            if (!regex.IsMatch(response))
            { this.errmsg = "LoginNoEventvalidation"; return null; }
            string eventvalidation = regex.Match(response).Groups[1].Value;

            //Thread.Sleep(3000);

            string value;
            string strEncryptData;
            if (oldAppQRCode)
            {
                string qrdata = this.DownloadString("https://tw.newlogin.beanfun.com/generic_handlers/get_qrcodeData.ashx?skey=" + skey + "&startGame=");
                regex = new Regex("\"strEncryptData\": \"(.*)\"}");
                if (!regex.IsMatch(qrdata))
                { this.errmsg = "LoginNoQrcodedata"; return null; }
                value = regex.Match(qrdata).Groups[1].Value;
                strEncryptData = Uri.UnescapeDataString(value);
            }
            else
            {
                regex = new Regex("\\$\\(\"#theQrCodeImg\"\\).attr\\(\"src\", \"../(.*)\" \\+ obj.strEncryptData\\);");
                if (!regex.IsMatch(response))
                { this.errmsg = "LoginNoHash"; return null; }
                value = regex.Match(response).Groups[1].Value;

                strEncryptData = this.getQRCodeStrEncryptData(skey);
                if (strEncryptData == null || strEncryptData == "")
                { this.errmsg = "LoginIntResultError"; return null; }
            }

            QRCodeClass res = new QRCodeClass();
            res.skey = skey;
            res.viewstate = viewstate;
            res.eventvalidation = eventvalidation;
            res.value = strEncryptData;
            res.bitmapUrl = oldAppQRCode ? ("http://tw.newlogin.beanfun.com/qrhandler.ashx?u=" + value) : ("https://tw.newlogin.beanfun.com/" + value);
            res.oldAppQRCode = oldAppQRCode;

            return res;
        }

        public string getQRCodeStrEncryptData(string skey)
        {
            string response = this.DownloadString("https://tw.newlogin.beanfun.com/generic_handlers/get_qrcodeData.ashx?skey=" + skey);
            JObject jsonData = JObject.Parse(response);
            if (jsonData["intResult"] == null || (int)jsonData["intResult"] != 1)
            { this.errmsg = "LoginIntResultError"; return null; }

            return (string)jsonData["strEncryptData"];
        }

        public BitmapImage getQRCodeImage(QRCodeClass qrcodeclass)
        {
            BitmapImage result;
            try
            {
                byte[] buffer = this.DownloadData(qrcodeclass.bitmapUrl + (qrcodeclass.oldAppQRCode ? "" : qrcodeclass.value));
                result = new BitmapImage();
                result.BeginInit();
                result.StreamSource = new MemoryStream(buffer);
                result.EndInit();
            }
            catch (Exception)
            {
                result = null;
            }
            return result;
        }

        private string QRCodeLogin(QRCodeClass qrcodeclass)
        {
            try
            {
                string skey = qrcodeclass.skey;
                
                this.Headers.Set("Referer", @"https://tw.newlogin.beanfun.com/login/qr_form.aspx?skey=" + skey);
                this.redirect = false;
                byte[] tmp2 = this.DownloadData("https://tw.newlogin.beanfun.com/login/qr_step2.aspx?skey=" + skey);
                this.redirect = true;
                string response2 = Encoding.UTF8.GetString(tmp2);
                Debug.Write(response2);
                Regex regex2 = new Regex("akey=(.*)&authkey");
                if (!regex2.IsMatch(response2))
                { this.errmsg = "AKeyParseFailed"; return null; }
                string akey = regex2.Match(response2).Groups[1].Value;

                regex2 = new Regex("authkey=(.*)&");
                if (!regex2.IsMatch(response2))
                { this.errmsg = "authkeyParseFailed"; return null; }
                string authkey = regex2.Match(response2).Groups[1].Value;
                Debug.WriteLine(authkey);
                string test = this.DownloadString("https://tw.newlogin.beanfun.com/login/final_step.aspx?akey="+akey+"&authkey="+ authkey+"&bfapp=1");
                return akey;
            }
            catch (Exception e)
            {
                this.errmsg = "LoginUnknown\n\n" + e.Message + "\n" + e.StackTrace;
                return null;
            }
        }

        public int QRCodeCheckLoginStatus(QRCodeClass qrcodeclass)
        {
            try
            {
                string skey = qrcodeclass.skey;
                //int errorCount = 0;
                string result;
                this.Headers.Set("Referer", @"https://tw.newlogin.beanfun.com/login/qr_form.aspx?skey=" + skey);

                NameValueCollection payload = new NameValueCollection();
                payload.Add(qrcodeclass.oldAppQRCode ? "data" : "status", qrcodeclass.value);
                //Debug.WriteLine(qrcodeclass.value);
                
                string response = this.UploadString(qrcodeclass.oldAppQRCode ? "https://tw.bfapp.beanfun.com/api/Check/CheckLoginStatus" : "https://tw.newlogin.beanfun.com/generic_handlers/CheckLoginStatus.ashx", payload);
                JObject jsonData;
                try { jsonData = JObject.Parse(response); }
                catch { this.errmsg = "LoginJsonParseFailed"; return -1; }

                result = (string) jsonData["ResultMessage"];
                Console.WriteLine(result);
                if (result == "Failed")
                    return 0;
                else if (result == "Token Expired")
                {
                    //this.errmsg = "登入逾時，請重新取得QRCode";
                    return -2;
                }
                else if (result == "Success")
                    return 1;
                else
                {
                    this.errmsg = response;
                    return -1;
                }
            }
            catch (Exception e)
            {
                this.errmsg = "Network Error on QRCode checking login status\n\n" + e.Message + "\n" + e.StackTrace;
            }

            return -1;
        }

        public JObject CheckIsRegisteDevice(string service_code = "610074", string service_region = "T9")
        {
            NameValueCollection payload = new NameValueCollection();
            payload.Add("LT", LoginToken);

            string response = this.UploadString("https://tw.newlogin.beanfun.com/login/bfAPPAutoLogin.ashx", payload);
            JObject json = JObject.Parse(response);
            if (json == null || json["IntResult"] == null || json["StrReslut"] == null)
                return null;

            if ((string)json["IntResult"] == "2")
            {
                string test = this.DownloadString("https://tw.newlogin.beanfun.com/login/" + (string)json["StrReslut"]);
                Regex regex = new Regex("akey=(.*)");
                if (!regex.IsMatch((string)json["StrReslut"]))
                { this.errmsg = "AKeyParseFailed"; return null; }
                string akey = regex.Match((string)json["StrReslut"]).Groups[1].Value;

                LoginCompleted(akey, service_code, service_region);
            }

            return json;
        }

        public string GetSessionkey()
        {
            if (App.LoginRegion == "TW")
            {
                string response = this.DownloadString("https://tw.beanfun.com/beanfun_block/bflogin/default.aspx?service=999999_T0");
                //this.DownloadString(this.ResponseHeaders["Location"]);
                //this.DownloadString(this.ResponseHeaders["Location"]);
                //response = this.ResponseHeaders["Location"];
                response = this.ResponseUri.ToString();
                if (response == null)
                { this.errmsg = "LoginNoResponse"; return null; }
                Regex regex = new Regex("skey=(.*)&display");
                if (!regex.IsMatch(response))
                { this.errmsg = "LoginNoSkey"; return null; }
                return regex.Match(response).Groups[1].Value;
            }
            else
            {
                string response = this.DownloadString("http://hk.beanfun.com/beanfun_block/login/index.aspx?service=999999_T0");
                if (response == null)
                { this.errmsg = "LoginNoResponse"; return null; }
                Regex regex = new Regex("otp1 = \"(.*)\";");
                if (!regex.IsMatch(response))
                { this.errmsg = "LoginNoOTP1"; return null; }
                string opt1 = regex.Match(response).Groups[1].Value;
                //regex = new Regex("seed = \"(.*)\";");
                //if (!regex.IsMatch(response))
                //{ this.errmsg = "LoginNoSeed"; return null; }
                //bfServ.Seed = regex.Match(response).Groups[1].Value;
                return opt1;
            }
        }

        public void Login(string id, string pass, int loginMethod, QRCodeClass qrcodeClass = null, string service_code = "610074", string service_region = "T9")
        {
            this.webtoken = null;
            this.SessionKey = null;
            try
            {
                string akey = null;
                if (loginMethod == (int)LoginMethod.QRCode)
                {
                    SessionKey = qrcodeClass.skey;
                }
                else
                {
                    if (App.LoginRegion == "HK")
                    {
                        if (this.bfServ == null)
                        {
                            try { bfServ = new BFServiceX(); }
                            catch { this.errmsg = "BFServiceXNotFound"; return; }
                        }
                        bfServ.Initialize2();
                    }
                    SessionKey = GetSessionkey();
                }
                
                switch (loginMethod)
                {
                    case (int)LoginMethod.Regular:
                        if (App.LoginRegion == "TW")
                            akey = RegularLogin(id, pass, SessionKey);
                        else
                            akey = RegularLogin_HK(id, pass, SessionKey);
                        break;
                    case (int)LoginMethod.QRCode:
                        akey = QRCodeLogin(qrcodeClass);
                        break;
                    default:
                        this.errmsg = "LoginNoMethod";
                        return;
                }

                LoginCompleted(akey, service_code, service_region);
            }
            catch (Exception e)
            {
                if (e is WebException)
                {
                    this.errmsg = "網路連線錯誤，請檢查官方網站連線是否正常。" + e.Message;
                }
                else
                {
                    this.errmsg = "LoginUnknown\n\n" + e.Message + "\n" + e.StackTrace;
                }
                return;
            }
        }

        private void LoginCompleted(string akey, string service_code = "610074", string service_region = "T9")
        {
            if (this.SessionKey == null || akey == null)
                return;

            if (App.LoginRegion == "TW")
            {
                NameValueCollection payload = new NameValueCollection();
                payload.Add("SessionKey", this.SessionKey);
                payload.Add("AuthKey", akey);
                payload.Add("ServiceCode", "");
                payload.Add("ServiceRegion", "");
                payload.Add("ServiceAccountSN", "0");
                Debug.WriteLine(this.SessionKey);
                Debug.WriteLine(akey);
                string response = this.UploadString("https://tw.beanfun.com/beanfun_block/bflogin/return.aspx", payload);
                Debug.WriteLine(response);
                response = this.DownloadString("https://tw.beanfun.com/" + this.ResponseHeaders["Location"]);
                Debug.WriteLine(response);
                Debug.WriteLine(this.ResponseHeaders);

                this.webtoken = this.GetCookie("bfWebToken");
                if (this.webtoken == "")
                { this.errmsg = "LoginNoWebtoken"; return; }
                GetAccounts(service_code, service_region, false);
            }
            else
            {
                string response = this.DownloadString("http://hk.beanfun.com/beanfun_block/auth.aspx?channel=game_zone&page_and_query=game_start.aspx%3Fservice_code_and_region%3D" + service_code + "_" + service_region + "&token=" + bfServ.Token);
                if (!response.Contains("document.location = \"http://hk.beanfun.com/beanfun_block/game_zone/game_server_account_list.aspx"))
                { this.errmsg = "LoginAuthErr"; return; }
                GetAccounts_HK(service_code, service_region, false);
            }

            if (this.errmsg != null) return;

            this.remainPoint = getRemainPoint();

            this.errmsg = null;
        }

        public void Logout()
        {
            string response;
            if (App.LoginRegion == "TW")
            {
                response = this.DownloadString("https://tw.beanfun.com/generic_handlers/remove_bflogin_session.ashx");
                //response = this.DownloadString("https://tw.newlogin.beanfun.com/logout.aspx?service=999999_T0");
                NameValueCollection payload = new NameValueCollection();
                payload.Add("web_token", "1");
                response = this.UploadString("https://tw.newlogin.beanfun.com/generic_handlers/erase_token.ashx", payload);
            }
            else
            {
                response = this.DownloadString("http://hk.beanfun.com/beanfun_block/generic_handlers/remove_login_session.ashx");
                response = this.DownloadString("http://hk.beanfun.com/beanfun_web_ap/remove_login_session.ashx");
                if (bfServ != null) response = this.DownloadString("http://hk.beanfun.com/beanfun_block/generic_handlers/erase_token.ashx?token=" + bfServ.Token);
            }
        }
    }
}
