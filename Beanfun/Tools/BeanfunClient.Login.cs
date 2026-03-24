using System;
using System.Collections.Specialized;
using System.Diagnostics;
using System.IO;
using System.Linq;
using System.Net;
using System.Text;
using System.Text.RegularExpressions;
using System.Windows;
using System.Windows.Input;
using System.Windows.Media.Imaging;
using Newtonsoft.Json.Linq;

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
                string response = this.DownloadString(
                    $"https://{loginHost}/login/id-pass_form{(App.LoginRegion == "HK" ? "_newBF.aspx?otp1" : ".aspx?skey")}={skey}"
                );
                Regex regex = new Regex("id=\"__VIEWSTATE\" value=\"(.*)\" />");
                if (!regex.IsMatch(response))
                {
                    this.errmsg = "LoginNoViewstate";
                    return null;
                }
                string viewstate = regex.Match(response).Groups[1].Value;

                regex = new Regex("id=\"__EVENTVALIDATION\" value=\"(.*)\" />");
                if (!regex.IsMatch(response))
                {
                    this.errmsg = "LoginNoEventvalidation";
                    return null;
                }
                string eventvalidation = regex.Match(response).Groups[1].Value;
                regex = new Regex("id=\"__VIEWSTATEGENERATOR\" value=\"(.*)\" />");
                if (!regex.IsMatch(response))
                {
                    this.errmsg = "LoginNoViewstateGenerator";
                    return null;
                }
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
                if (App.LoginRegion == "HK")
                    payload.Add("__VIEWSTATEENCRYPTED", "");
                payload.Add("__EVENTVALIDATION", eventvalidation);
                payload.Add("t_AccountID", id);
                payload.Add("t_Password", pass);
                //payload.Add("CodeTextBox", Captcha);
                //payload.Add("LBD_VCID_c_login_idpass_form_samplecaptcha", samplecaptcha);
                //payload.Add("g-recaptcha-response", samplecaptcha);
                //payload.Add("token1", "");
                payload.Add("btn_login", "登入");

                response = this.UploadString(
                    $"https://{loginHost}/login/id-pass_form{(App.LoginRegion == "HK" ? "_newBF.aspx?otp1" : ".aspx?skey")}={skey}",
                    payload
                );
                if (response.Contains("RELOAD_CAPTCHA_CODE") && response.Contains("alert"))
                {
                    this.errmsg = "LoginAdvanceCheck";
                    return null;
                }

                if (response.Contains("totpLoginBtn"))
                {
                    this.totpResponse = response;
                    this.totpUrl =
                        $"https://{loginHost}/login/id-pass_form{(App.LoginRegion == "HK" ? "_newBF.aspx?otp1" : ".aspx?skey")}={skey}";
                    this.errmsg = "need_totp";
                    return null;
                }

                regex = new Regex("akey=(.*)");
                if (!regex.IsMatch(this.ResponseUri.ToString()))
                {
                    this.errmsg = "LoginNoAkey";
                    regex = new Regex(
                        "<script type=\"text/javascript\">\\$\\(function\\(\\){MsgBox.Show\\('(.*)'\\);}\\);</script>"
                    );
                    if (regex.IsMatch(response))
                    {
                        this.errmsg = regex.Match(response).Groups[1].Value;
                    }
                    else
                    {
                        regex = new Regex("pollRequest\\(\"([^\"]*)\",\"(\\w+)\",\"([^\"]+)\"\\);");
                        if (regex.IsMatch(response))
                        {
                            this.errmsg =
                                regex.Match(response).Groups[1].Value
                                + "\",\""
                                + regex.Match(response).Groups[3].Value;
                            LoginToken = regex.Match(response).Groups[2].Value;
                        }
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

        public void TotpLogin(
            string otp1,
            string otp2,
            string otp3,
            string otp4,
            string otp5,
            string otp6,
            string service_code = "610074",
            string service_region = "T9"
        )
        {
            string loginHost = this.totpUrl;

            try
            {
                string response = this.totpResponse;
                Regex regex = new Regex("id=\"__VIEWSTATE\" value=\"(.*)\" />");
                if (!regex.IsMatch(response))
                {
                    this.errmsg = "LoginNoViewstate";
                    return;
                }
                string viewstate = regex.Match(response).Groups[1].Value;

                regex = new Regex("id=\"__EVENTVALIDATION\" value=\"(.*)\" />");
                if (!regex.IsMatch(response))
                {
                    this.errmsg = "LoginNoEventvalidation";
                    return;
                }
                string eventvalidation = regex.Match(response).Groups[1].Value;
                regex = new Regex("id=\"__VIEWSTATEGENERATOR\" value=\"(.*)\" />");
                if (!regex.IsMatch(response))
                {
                    this.errmsg = "LoginNoViewstateGenerator";
                    return;
                }
                string viewstateGenerator = regex.Match(response).Groups[1].Value;

                NameValueCollection payload = new NameValueCollection();
                payload.Add("__EVENTTARGET", "");
                payload.Add("__EVENTARGUMENT", "");
                payload.Add("__VIEWSTATE", viewstate);
                payload.Add("__VIEWSTATEGENERATOR", viewstateGenerator);
                if (App.LoginRegion == "HK")
                    payload.Add("__VIEWSTATEENCRYPTED", "");
                payload.Add("__EVENTVALIDATION", eventvalidation);
                payload.Add("otpCode1", otp1);
                payload.Add("otpCode2", otp2);
                payload.Add("otpCode3", otp3);
                payload.Add("otpCode4", otp4);
                payload.Add("otpCode5", otp5);
                payload.Add("otpCode6", otp6);
                payload.Add("totpLoginBtn", "登入");

                response = this.UploadString(loginHost, payload);
                if (response.Contains("RELOAD_CAPTCHA_CODE") && response.Contains("alert"))
                {
                    this.errmsg = "LoginAdvanceCheck";
                    return;
                }

                regex = new Regex("akey=(.*)");
                if (!regex.IsMatch(this.ResponseUri.ToString()))
                {
                    this.errmsg = "LoginNoAkey";
                    regex = new Regex(
                        "<script type=\"text/javascript\">\\$\\(function\\(\\){MsgBox.Show\\('(.*)'\\);}\\);</script>"
                    );
                    if (regex.IsMatch(response))
                    {
                        this.errmsg = regex.Match(response).Groups[1].Value;
                    }
                    else
                    {
                        regex = new Regex("pollRequest\\(\"([^\"]*)\",\"(\\w+)\",\"([^\"]+)\"\\);");
                        if (regex.IsMatch(response))
                        {
                            this.errmsg =
                                regex.Match(response).Groups[1].Value
                                + "\",\""
                                + regex.Match(response).Groups[3].Value;
                            LoginToken = regex.Match(response).Groups[2].Value;
                        }
                    }
                    return;
                }
                string akey = regex.Match(this.ResponseUri.ToString()).Groups[1].Value;

                LoginCompleted(akey, service_code, service_region);
            }
            catch (Exception e)
            {
                this.errmsg = "LoginUnknown\n\n" + e.Message + "\n" + e.StackTrace;
                return;
            }
        }

        public class QRCodeClass
        {
            public string skey;
            public string bitmapBase64;
        }

        public QRCodeClass GetQRCodeValue(string skey)
        {
            this.Headers.Clear();
            this.Headers.Add("User-Agent", "Mozilla/5.0");
            this.Headers.Add("Accept", "text/html");

            string url = $"https://login.beanfun.com/Login/Index?pSKey={skey}";
            string response = this.DownloadString(url);
            //Regex regex = new Regex("id=\"__VIEWSTATE\" value=\"(.*)\" />");
            //if (!regex.IsMatch(response))
            //{ this.errmsg = "LoginNoViewstate"; return null; }
            //string viewstate = regex.Match(response).Groups[1].Value;

            //regex = new Regex("id=\"__EVENTVALIDATION\" value=\"(.*)\" />");
            //if (!regex.IsMatch(response))
            //{ this.errmsg = "LoginNoEventvalidation"; return null; }
            //string eventvalidation = regex.Match(response).Groups[1].Value;

            //Thread.Sleep(3000);

            //regex = new Regex("\\$\\(\"#theQrCodeImg\"\\)\\.attr\\(\"src\", \"\\.\\./(.*)\"");
            //if (!regex.IsMatch(response))
            //{ this.errmsg = "LoginNoHash"; return null; }
            //string value = regex.Match(response).Groups[1].Value;

            JObject strEncryptData = this.getQRCodeStrEncryptData(skey);
            if (strEncryptData == null)
            {
                this.errmsg = "LoginIntResultError";
                return null;
            }

            return new QRCodeClass
            {
                skey = skey,
                bitmapBase64 =
                    "data:image/png;base64," + (string)strEncryptData["ResultData"]["QRImage"],
            };
        }

        public JObject getQRCodeStrEncryptData(string skey)
        {
            this.Headers.Clear();
            this.Headers.Add("User-Agent", "Mozilla/5.0");
            this.Headers.Add("Accept", "application/json, text/plain, */*");
            this.Headers.Add("Referer", $"https://login.beanfun.com/Login/Index?pSKey={skey}");
            this.Headers.Add("Origin", "https://login.beanfun.com");

            string response = this.DownloadString(
                $"https://login.beanfun.com/Login/InitLogin?pSKey={skey}"
            );
            JObject jsonData = JObject.Parse(response);

            if (jsonData["Result"] == null || (int)jsonData["Result"] != 0)
            {
                this.errmsg = "LoginIntResultError";
                return null;
            }

            return jsonData;
        }

        public BitmapImage getQRCodeImage(QRCodeClass qrcodeclass)
        {
            try
            {
                byte[] bytes = Convert.FromBase64String(
                    qrcodeclass.bitmapBase64.Replace("data:image/png;base64,", "")
                );

                BitmapImage image = new BitmapImage();
                using (var ms = new MemoryStream(bytes))
                {
                    image.BeginInit();
                    image.CacheOption = BitmapCacheOption.OnLoad;
                    image.StreamSource = ms;
                    image.EndInit();
                }
                return image;
            }
            catch
            {
                return null;
            }
        }

        private string QRCodeLogin(QRCodeClass qrcodeclass)
        {
            try
            {
                string skey = qrcodeclass.skey;

                this.Headers.Clear();
                this.Headers.Add(
                    "User-Agent",
                    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/114.0.0.0 Safari/537.36"
                );
                this.Headers.Add("Accept", "application/json, text/plain, */*");
                this.Headers.Add("Referer", $"https://login.beanfun.com/Login/Index?pSKey={skey}");
                this.Headers.Add("Origin", "https://login.beanfun.com");

                string response = this.DownloadString("https://login.beanfun.com/QRLogin/QRLogin");
                Debug.WriteLine("QRLogin response: " + response);

                this.Headers.Clear();
                this.Headers.Add(
                    "User-Agent",
                    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/114.0.0.0 Safari/537.36"
                );
                this.Headers.Add(
                    "Accept",
                    "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8"
                );
                this.Headers.Add("Referer", $"https://login.beanfun.com/Login/Index?pSKey={skey}");

                string sendLoginHtml = this.DownloadString(
                    "https://login.beanfun.com/Login/SendLogin"
                );

                NameValueCollection payload = new NameValueCollection();
                MatchCollection inputTags = Regex.Matches(sendLoginHtml, @"<input[^>]+>");
                foreach (Match tag in inputTags)
                {
                    string tagStr = tag.Value;
                    Match nameMatch = Regex.Match(
                        tagStr,
                        @"name\s*=\s*['""]([^'""]+)['""]",
                        RegexOptions.IgnoreCase
                    );
                    Match valMatch = Regex.Match(
                        tagStr,
                        @"value\s*=\s*['""]([^'""]*)['""]",
                        RegexOptions.IgnoreCase
                    );

                    if (
                        nameMatch.Success
                        && valMatch.Success
                        && tagStr.IndexOf("type=\"submit\"", StringComparison.OrdinalIgnoreCase)
                            == -1
                    )
                    {
                        payload.Add(nameMatch.Groups[1].Value, valMatch.Groups[1].Value);
                    }
                }

                if (payload.Count == 0)
                {
                    this.errmsg = "SendLoginNoFormData: Not Found";
                    return null;
                }

                string host = "tw.beanfun.com";
                string returnUrl = $"https://{host}/beanfun_block/bflogin/return.aspx";

                this.Headers.Clear();
                this.Headers.Add(
                    "User-Agent",
                    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36"
                );
                this.Headers.Add("Referer", "https://login.beanfun.com/");

                this.redirect = false;
                string returnResponse = this.UploadString(returnUrl, payload);

                string setCookieHeader =
                    this.ResponseHeaders != null ? this.ResponseHeaders["Set-Cookie"] : "";
                if (!string.IsNullOrEmpty(setCookieHeader))
                {
                    Regex tokenRegex = new Regex(@"bfWebToken=([^;]+)");
                    Match tokenMatch = tokenRegex.Match(setCookieHeader);
                    if (tokenMatch.Success)
                    {
                        this.webtoken = tokenMatch.Groups[1].Value;
                    }
                }

                if (string.IsNullOrEmpty(this.webtoken))
                {
                    this.errmsg = "LoginNoWebtoken";
                    return null;
                }

                this.redirect = true;
                if (this.ResponseHeaders != null && this.ResponseHeaders["Location"] != null)
                {
                    string location = this.ResponseHeaders["Location"];
                    this.DownloadString(
                        location.StartsWith("http") ? location : $"https://{host}{location}"
                    );
                }

                return "OK";
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
                this.Headers.Add("User-Agent", "Mozilla/5.0");
                this.Headers.Add("Accept", "application/json, text/plain, */*");
                this.Headers.Add("Referer", $"https://login.beanfun.com/Login/Index?pSKey={skey}");
                this.Headers.Add("Origin", "https://login.beanfun.com");
                //Debug.WriteLine(qrcodeclass.value);

                string response = this.DownloadString(
                    $"https://login.beanfun.com/QRLogin/CheckLoginStatus?pSKey={skey}"
                );
                JObject jsonData;
                try
                {
                    jsonData = JObject.Parse(response);
                }
                catch
                {
                    this.errmsg = "LoginJsonParseFailed";
                    return -1;
                }

                result = (string)jsonData["ResultMessage"];
                Console.WriteLine(result);
                if (result == "Failed" || result == "Wait Login")
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
                this.errmsg =
                    "Network Error on QRCode checking login status\n\n"
                    + e.Message
                    + "\n"
                    + e.StackTrace;
            }

            return -1;
        }

        public JObject CheckIsRegisteDevice(
            string service_code = "610074",
            string service_region = "T9"
        )
        {
            NameValueCollection payload = new NameValueCollection();
            payload.Add("LT", LoginToken);

            string response = this.UploadString(
                "https://tw.newlogin.beanfun.com/login/bfAPPAutoLogin.ashx",
                payload
            );
            JObject json = JObject.Parse(response);
            if (json == null || json["IntResult"] == null || json["StrReslut"] == null)
                return null;

            if ((string)json["IntResult"] == "2")
            {
                string test = this.DownloadString(
                    "https://tw.newlogin.beanfun.com/login/" + (string)json["StrReslut"]
                );
                Regex regex = new Regex("akey=(.*)");
                if (!regex.IsMatch((string)json["StrReslut"]))
                {
                    this.errmsg = "AKeyParseFailed";
                    return null;
                }
                string akey = regex.Match((string)json["StrReslut"]).Groups[1].Value;

                LoginCompleted(akey, service_code, service_region);
            }

            return json;
        }

        public string GetSessionkey()
        {
            if (App.LoginRegion == "TW")
            {
                string response = this.DownloadString(
                    "https://tw.beanfun.com/beanfun_block/bflogin/default.aspx?service=999999_T0"
                );
                //this.DownloadString(this.ResponseHeaders["Location"]);
                //this.DownloadString(this.ResponseHeaders["Location"]);
                //response = this.ResponseHeaders["Location"];
                response = this.ResponseUri.ToString();
                if (response == null)
                {
                    this.errmsg = "LoginNoResponse";
                    return null;
                }
                Regex regex = new Regex("skey=(.*)&display");
                if (!regex.IsMatch(response))
                {
                    this.errmsg = "LoginNoSkey";
                    return null;
                }
                return regex.Match(response).Groups[1].Value;
            }
            else
            {
                string response = this.DownloadString(
                    "https://bfweb.hk.beanfun.com/beanfun_block/bflogin/default.aspx?service=999999_T0"
                );
                if (response == null)
                {
                    this.errmsg = "LoginNoResponse";
                    return null;
                }
                Regex regex = new Regex(
                    "<span id=\"ctl00_ContentPlaceHolder1_lblOtp1\">(.*)</span>"
                );
                if (!regex.IsMatch(response))
                {
                    this.errmsg = "LoginNoOTP1";
                    return null;
                }
                return regex.Match(response).Groups[1].Value;
            }
        }

        public void Login(
            string id,
            string pass,
            int loginMethod,
            QRCodeClass qrcodeClass = null,
            string service_code = "610074",
            string service_region = "T9"
        )
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
                    this.errmsg =
                        System.Windows.Application.Current.TryFindResource("NetworkConnectionError")
                            as string
                        + e.Message;
                }
                else
                {
                    this.errmsg = "LoginUnknown\n\n" + e.Message + "\n" + e.StackTrace;
                }
                return;
            }
        }

        private void LoginCompleted(
            string akey,
            string service_code = "610074",
            string service_region = "T9"
        )
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
            string response = this.UploadString(
                $"https://{host}/beanfun_block/bflogin/return.aspx",
                payload
            );
            //Debug.WriteLine(response);
            response = this.DownloadString($"https://{host}/{this.ResponseHeaders["Location"]}");
            //Debug.WriteLine(response);
            Debug.WriteLine(this.ResponseHeaders);

            this.webtoken = this.GetCookie("bfWebToken");
            if (this.webtoken == "")
            {
                this.errmsg = "LoginNoWebtoken";
                return;
            }
            GetAccounts(service_code, service_region, false);

            if (this.errmsg != null)
                return;

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
                this.UploadString(
                    "https://tw.newlogin.beanfun.com/generic_handlers/erase_token.ashx",
                    payload
                );
            }
        }
    }
}
