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
using System.Windows;

namespace Beanfun
{
    public partial class BeanfunClient : WebClient
    {
        private string RegularLogin(string id, string pass, string skey)
        {
            string loginHost;
            if (App.LoginRegion == "TW")
                loginHost = "tw.newlogin.beanfun.com";
            else
                loginHost = "login.hk.beanfun.com";

            try
            {
                string response = this.DownloadString($"https://{loginHost}/login/id-pass_form{(App.LoginRegion == "HK" ? "_newBF.aspx?otp1" : ".aspx?skey")}={skey}");
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
                /*
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
                */

                NameValueCollection payload = new NameValueCollection();
                payload.Add("__EVENTTARGET", "");
                payload.Add("__EVENTARGUMENT", "");
                payload.Add("__VIEWSTATE", viewstate);
                payload.Add("__VIEWSTATEGENERATOR", viewstateGenerator);
                if (App.LoginRegion == "HK") payload.Add("__VIEWSTATEENCRYPTED", "");
                payload.Add("__EVENTVALIDATION", eventvalidation);
                payload.Add("t_AccountID", id);
                payload.Add("t_Password", pass);
                //payload.Add("CodeTextBox", Captcha);
                //payload.Add("LBD_VCID_c_login_idpass_form_samplecaptcha", samplecaptcha);
                //payload.Add("g-recaptcha-response", samplecaptcha);
                //payload.Add("token1", "");
                payload.Add("btn_login", "登入");

                response = this.UploadString($"https://{loginHost}/login/id-pass_form{(App.LoginRegion == "HK" ? "_newBF.aspx?otp1" : ".aspx?skey")}={skey}", payload);
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
        }

        public QRCodeClass GetQRCodeValue(string skey)
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

            regex = new Regex("\\$\\(\"#theQrCodeImg\"\\).attr\\(\"src\", \"../(.*)\" \\+ obj.strEncryptData\\);");
            if (!regex.IsMatch(response))
            { this.errmsg = "LoginNoHash"; return null; }
            string value = regex.Match(response).Groups[1].Value;

            string strEncryptData = this.getQRCodeStrEncryptData(skey);
            if (strEncryptData == null || strEncryptData == "")
            { this.errmsg = "LoginIntResultError"; return null; }

            QRCodeClass res = new QRCodeClass();
            res.skey = skey;
            res.viewstate = viewstate;
            res.eventvalidation = eventvalidation;
            res.value = strEncryptData;
            res.bitmapUrl = "https://tw.newlogin.beanfun.com/" + value;

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
                byte[] buffer = this.DownloadData(qrcodeclass.bitmapUrl + qrcodeclass.value);
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
                payload.Add("status", qrcodeclass.value);
                //Debug.WriteLine(qrcodeclass.value);
                
                string response = this.UploadString("https://tw.newlogin.beanfun.com/generic_handlers/CheckLoginStatus.ashx", payload);
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
                string response = this.DownloadString("https://bfweb.hk.beanfun.com/beanfun_block/bflogin/default.aspx?service=999999_T0");
                if (response == null)
                { this.errmsg = "LoginNoResponse"; return null; }
                Regex regex = new Regex("<span id=\"ctl00_ContentPlaceHolder1_lblOtp1\">(.*)</span>");
                if (!regex.IsMatch(response))
                { this.errmsg = "LoginNoOTP1"; return null; }
                return regex.Match(response).Groups[1].Value;
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
                    SessionKey = GetSessionkey();
                }
                
                switch (loginMethod)
                {
                    case (int)LoginMethod.Regular:
                        akey = RegularLogin(id, pass, SessionKey);
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
                    this.errmsg = System.Windows.Application.Current.TryFindResource("NetworkConnectionError") as string + e.Message;
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

            string host;
            if (App.LoginRegion == "TW")
                host = "tw.beanfun.com";
            else
                host = "bfweb.hk.beanfun.com";

            NameValueCollection payload = new NameValueCollection();
            payload.Add("SessionKey", this.SessionKey);
            payload.Add("AuthKey", akey);
            payload.Add("ServiceCode", "");
            payload.Add("ServiceRegion", "");
            payload.Add("ServiceAccountSN", "0");
            Debug.WriteLine(this.SessionKey);
            Debug.WriteLine(akey);
            string response = this.UploadString($"https://{host}/beanfun_block/bflogin/return.aspx", payload);
            //Debug.WriteLine(response);
            response = this.DownloadString($"https://{host}/{this.ResponseHeaders["Location"]}");
            //Debug.WriteLine(response);
            Debug.WriteLine(this.ResponseHeaders);

            this.webtoken = this.GetCookie("bfWebToken");
            if (this.webtoken == "")
            { this.errmsg = "LoginNoWebtoken"; return; }
            GetAccounts(service_code, service_region, false);

            if (this.errmsg != null) return;

            this.remainPoint = getRemainPoint();

            this.errmsg = null;
        }

        public void Logout()
        {
            string host;
            string loginHost;
            if (App.LoginRegion == "TW")
            {
                host = "tw.beanfun.com";
                loginHost = "tw.newlogin.beanfun.com";
            }
            else
            {
                host = "bfweb.hk.beanfun.com";
                loginHost = "login.hk.beanfun.com";
            }
            this.DownloadString($"https://{host}/generic_handlers/remove_bflogin_session.ashx");
            this.DownloadString($"https://{loginHost}/logout.aspx?service=999999_T0");
            if (App.LoginRegion == "TW")
            {
                NameValueCollection payload = new NameValueCollection();
                payload.Add("web_token", "1");
                this.UploadString("https://tw.newlogin.beanfun.com/generic_handlers/erase_token.ashx", payload);
            }
        }
    }
}
