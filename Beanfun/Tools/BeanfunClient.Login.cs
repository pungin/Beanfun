using System;
using System.Collections.Specialized;
using System.Diagnostics;
using System.IO;
using System.Net;
using System.Text.RegularExpressions;
using System.Web;
using System.Windows.Media.Imaging;
using Newtonsoft.Json.Linq;

namespace Beanfun
{
    public partial class BeanfunClient : WebClient
    {
        private string RegularLogin(string id, string pass, string skey)
        {
            if (App.LoginRegion == "TW")
                return RegularLoginOfficial(id, pass, skey);

            string loginHost;
            loginHost = "login.hk.beanfun.com";

            try
            {
                string response = this.DownloadString(
                    $"https://{loginHost}/login/id-pass_form{(App.LoginRegion == "HK" ? "_newBF.aspx?otp1" : ".aspx?skey")}={skey}"
                );
                if (
                    !TryGetAspNetFormState(
                        response,
                        true,
                        out string viewstate,
                        out string viewstateGenerator,
                        out string eventvalidation
                    )
                )
                    return null;

                Regex regex;
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

        private string RegularLoginOfficial(string id, string pass, string skey)
        {
            string indexUrl = $"https://login.beanfun.com/Login/Index?pSKey={HttpUtility.UrlEncode(skey)}";

            try
            {
                Debug.WriteLine("Official login: GET Index");
                SetBaseHeaders(false, "text/html");
                string response = this.DownloadString(indexUrl);
                if (!TryGetOfficialRequestVerificationToken(response, out string requestVerificationToken))
                {
                    this.errmsg = "LoginNoRequestVerificationToken";
                    return null;
                }

                Debug.WriteLine("Official login: POST CheckAccountType");
                JObject checkAccountTypeResponse = PostOfficialJson(
                    "https://login.beanfun.com/Login/CheckAccountType",
                    new JObject
                    {
                        ["Account"] = id,
                        ["Captcha"] = "",
                    },
                    requestVerificationToken,
                    indexUrl
                );
                if (checkAccountTypeResponse == null || !TryEnsureOfficialApiSuccess(checkAccountTypeResponse))
                    return null;

                if (checkAccountTypeResponse["ResultData"]?["IsGamaPass"]?.Value<bool>() == true)
                {
                    string gamaPassUrl =
                        checkAccountTypeResponse["ResultData"]?["GamaPassUrl"]?.Value<string>();
                    this.errmsg = string.IsNullOrWhiteSpace(gamaPassUrl)
                        ? "此账号需要使用 GamaPass 登录。"
                        : gamaPassUrl;
                    return null;
                }

                Debug.WriteLine("Official login: POST AccountLogin");
                response = PostOfficialAccountLogin(id, pass, requestVerificationToken, indexUrl);
                if (this.errmsg != null)
                    return null;

                if (TryHandleOfficialLoginInterruption(response))
                    return null;

                if (
                    TryParseOfficialApiJson(response, out JObject accountLoginJson)
                    && !TryEnsureOfficialApiSuccess(accountLoginJson)
                )
                    return null;

                Debug.WriteLine("Official login: GET SendLogin");
                if (!TryCompleteOfficialSendLogin(indexUrl))
                    return null;

                Debug.WriteLine("Official login: bfWebToken acquired");
                return null;
            }
            catch (Exception e)
            {
                this.errmsg = "LoginUnknown\n\n" + e.Message + "\n" + e.StackTrace;
                return null;
            }
        }

        private bool TryGetOfficialRequestVerificationToken(string html, out string token)
        {
            token = null;
            if (HtmlInputParser.TryGetInputValue(html, "__RequestVerificationToken", out token))
                return true;

            Match match = Regex.Match(
                html,
                @"name\s*=\s*[""']__RequestVerificationToken[""'][^>]*value\s*=\s*[""']([^""']+)[""']",
                RegexOptions.IgnoreCase | RegexOptions.Singleline
            );
            if (match.Success)
            {
                token = HttpUtility.HtmlDecode(match.Groups[1].Value);
                return true;
            }

            match = Regex.Match(
                html,
                @"requestverificationtoken[""']?\s*[:=]\s*[""']([^""']+)[""']",
                RegexOptions.IgnoreCase | RegexOptions.Singleline
            );
            if (match.Success)
            {
                token = HttpUtility.HtmlDecode(match.Groups[1].Value);
                return true;
            }

            return false;
        }

        private JObject PostOfficialJson(
            string url,
            JObject payload,
            string requestVerificationToken,
            string referer
        )
        {
            string response = PostOfficialJsonRaw(url, payload, requestVerificationToken, referer);
            if (!TryParseOfficialApiJson(response, out JObject json))
            {
                this.errmsg = "LoginJsonParseFailed";
                return null;
            }

            return json;
        }

        private string PostOfficialJsonRaw(
            string url,
            JObject payload,
            string requestVerificationToken,
            string referer
        )
        {
            SetBaseHeaders(true, "application/json, text/plain, */*", referer);
            this.Headers.Add("Origin", "https://login.beanfun.com");
            this.Headers.Add("RequestVerificationToken", requestVerificationToken);
            return this.UploadJsonString(url, payload.ToString(Newtonsoft.Json.Formatting.None));
        }

        private bool TryParseOfficialApiJson(string response, out JObject json)
        {
            json = null;
            if (string.IsNullOrWhiteSpace(response))
                return false;

            string trimmed = response.TrimStart();
            if (!trimmed.StartsWith("{") && !trimmed.StartsWith("["))
                return false;

            try
            {
                json = JObject.Parse(response);
                return true;
            }
            catch
            {
                return false;
            }
        }

        private bool TryEnsureOfficialApiSuccess(JObject response)
        {
            int? result = response["Result"]?.Value<int>();
            int? resultCode = response["ResultCode"]?.Value<int>();
            int? statusCode = response["StatusCode"]?.Value<int>();

            if ((result == null || result == 0) && (resultCode == null || resultCode == 1) && (statusCode == null || statusCode == 0))
                return true;

            string message =
                response["ResultData"]?["Message"]?.Value<string>()
                ?? response["ResultMessage"]?.Value<string>()
                ?? response["LogMessage"]?.Value<string>()
                ?? response["Message"]?.Value<string>();
            this.errmsg = string.IsNullOrWhiteSpace(message) ? "LoginUnknown" : message;
            return false;
        }

        private string PostOfficialAccountLogin(
            string id,
            string pass,
            string requestVerificationToken,
            string indexUrl
        )
        {
            string response = null;
            this.redirect = false;

            try
            {
                response = PostOfficialJsonRaw(
                    "https://login.beanfun.com/Login/AccountLogin",
                    new JObject
                    {
                        ["Account"] = id,
                        ["Captcha"] = "",
                        ["Pasw"] = pass,
                        ["IsMobile"] = false,
                    },
                    requestVerificationToken,
                    indexUrl
                );
            }
            finally
            {
                this.redirect = true;
            }

            string location = this.ResponseHeaders?["Location"];
            if (string.IsNullOrWhiteSpace(location))
                return response;

            string absoluteLocation = ToAbsoluteOfficialUrl(location);
            if (absoluteLocation.IndexOf("AdvanceCheck", StringComparison.OrdinalIgnoreCase) >= 0)
            {
                this.errmsg = "LoginAdvanceCheck";
                return null;
            }

            if (absoluteLocation.IndexOf("SendLogin", StringComparison.OrdinalIgnoreCase) >= 0)
                return response;

            SetBaseHeaders(
                true,
                "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
                indexUrl
            );
            return this.DownloadString(absoluteLocation);
        }

        private string ToAbsoluteOfficialUrl(string location)
        {
            if (Uri.TryCreate(location, UriKind.Absolute, out Uri absoluteUri))
                return absoluteUri.ToString();

            return new Uri(new Uri("https://login.beanfun.com/"), location).ToString();
        }

        private bool TryHandleOfficialLoginInterruption(string response)
        {
            if (
                this.ResponseUri != null
                && this.ResponseUri.AbsoluteUri.IndexOf("AdvanceCheck", StringComparison.OrdinalIgnoreCase)
                    >= 0
            )
            {
                this.errmsg = "LoginAdvanceCheck";
                return true;
            }

            if (string.IsNullOrWhiteSpace(response))
                return false;

            if (response.Contains("RELOAD_CAPTCHA_CODE") && response.Contains("alert"))
            {
                this.errmsg = "LoginAdvanceCheck";
                return true;
            }

            if (response.Contains("totpLoginBtn"))
            {
                this.totpResponse = response;
                this.totpUrl = this.ResponseUri?.ToString() ?? "https://login.beanfun.com/Login/AccountLogin";
                this.errmsg = "need_totp";
                return true;
            }

            Regex regex = new Regex(
                "<script type=\"text/javascript\">\\$\\(function\\(\\){MsgBox.Show\\('(.*)'\\);}\\);</script>"
            );
            if (regex.IsMatch(response))
            {
                this.errmsg = regex.Match(response).Groups[1].Value;
                return true;
            }

            regex = new Regex("pollRequest\\(\"([^\"]*)\",\"(\\w+)\",\"([^\"]+)\"\\);");
            if (regex.IsMatch(response))
            {
                this.errmsg =
                    regex.Match(response).Groups[1].Value
                    + "\",\""
                    + regex.Match(response).Groups[3].Value;
                LoginToken = regex.Match(response).Groups[2].Value;
                return true;
            }

            return false;
        }

        private bool TryCompleteOfficialSendLogin(string indexUrl)
        {
            SetBaseHeaders(
                true,
                "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8",
                indexUrl
            );
            string sendLoginHtml = this.DownloadString("https://login.beanfun.com/Login/SendLogin");
            NameValueCollection payload = new NameValueCollection();

            foreach (
                Match tag in Regex.Matches(
                    sendLoginHtml,
                    @"<input[^>]+>",
                    RegexOptions.IgnoreCase | RegexOptions.Singleline
                )
            )
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
                    && tagStr.IndexOf("type=\"submit\"", StringComparison.OrdinalIgnoreCase) == -1
                )
                    payload.Add(nameMatch.Groups[1].Value, valMatch.Groups[1].Value);
            }

            if (
                payload.Count == 0
                || string.IsNullOrWhiteSpace(payload["SessionKey"])
                || string.IsNullOrWhiteSpace(payload["AuthKey"])
            )
            {
                if (TryHandleOfficialLoginInterruption(sendLoginHtml))
                    return false;

                this.errmsg = "SendLoginNoFormData";
                return false;
            }

            this.redirect = false;
            try
            {
                string host = App.LoginRegion == "TW" ? "tw.beanfun.com" : "bfweb.hk.beanfun.com";
                SetBaseHeaders(true, null, "https://login.beanfun.com/");
                this.UploadString($"https://{host}/beanfun_block/bflogin/return.aspx", payload);
            }
            finally
            {
                this.redirect = true;
            }

            this.webtoken = this.GetCookie("bfWebToken");
            if (string.IsNullOrEmpty(this.webtoken))
            {
                Match tokenMatch = Regex.Match(this.ResponseHeaders?["Set-Cookie"] ?? "", @"bfWebToken=([^;]+)");
                if (tokenMatch.Success)
                    this.webtoken = tokenMatch.Groups[1].Value;
            }

            if (string.IsNullOrEmpty(this.webtoken))
            {
                this.errmsg = "LoginNoWebtoken";
                return false;
            }

            return true;
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
                if (
                    !TryGetAspNetFormState(
                        response,
                        true,
                        out string viewstate,
                        out string viewstateGenerator,
                        out string eventvalidation
                    )
                )
                    return;

                Regex regex;

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
            public string deeplink;
        }

        public QRCodeClass GetQRCodeValue(string skey)
        {
            SetBaseHeaders(false, "text/html");
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

            JObject result = (JObject)strEncryptData["ResultData"];
            if (result == null)
            {
                this.errmsg = "LoginIntResultError";
                return null;
            }

            string base64Image = (string)result["QRImage"];
            if (string.IsNullOrEmpty(base64Image))
            {
                this.errmsg = "LoginIntResultError";
                return null;
            }

            string deepLinkRaw = result["DeepLink"]?.Value<string>();
            string deeplink = NormalizeBeanfunAppDeeplink(deepLinkRaw);

            return new QRCodeClass
            {
                skey = skey,
                bitmapBase64 = "data:image/png;base64," + base64Image,
                deeplink = deeplink,
            };
        }

        public JObject getQRCodeStrEncryptData(string skey)
        {
            SetBaseHeaders(
                true,
                "application/json, text/plain, */*",
                $"https://login.beanfun.com/Login/Index?pSKey={skey}"
            );
            this.Headers.Add("X-Requested-With", "XMLHttpRequest");
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

        private string NormalizeBeanfunAppDeeplink(string raw)
        {
            if (string.IsNullOrWhiteSpace(raw))
                return raw;

            if (!Uri.TryCreate(raw.Trim(), UriKind.Absolute, out Uri uri))
                return raw;

            if (
                !string.Equals(
                    uri.Host,
                    "play.games.gamania.com",
                    StringComparison.OrdinalIgnoreCase
                )
            )
                return raw;

            if (uri.AbsolutePath.IndexOf("deeplink", StringComparison.OrdinalIgnoreCase) < 0)
                return raw;

            NameValueCollection query = HttpUtility.ParseQueryString(uri.Query);
            string inner = query["url"];
            if (!string.IsNullOrEmpty(inner))
                return inner;

            return raw;
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
                // QRLogin
                SetBaseHeaders(
                    true,
                    "application/json, text/plain, */*",
                    $"https://login.beanfun.com/Login/Index?pSKey={skey}"
                );
                string response = this.DownloadString("https://login.beanfun.com/QRLogin/QRLogin");
                Debug.WriteLine("QRLogin response: " + response);

                // SendLogin
                SetBaseHeaders(
                    true,
                    "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8",
                    $"https://login.beanfun.com/Login/Index?pSKey={skey}"
                );
                string sendLoginHtml = this.DownloadString(
                    "https://login.beanfun.com/Login/SendLogin"
                );

                NameValueCollection payload = new NameValueCollection();
                foreach (
                    Match tag in Regex.Matches(
                        sendLoginHtml,
                        @"<input[^>]+>",
                        RegexOptions.IgnoreCase | RegexOptions.Singleline
                    )
                )
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
                        payload.Add(nameMatch.Groups[1].Value, valMatch.Groups[1].Value);
                }

                if (payload.Count == 0)
                {
                    errmsg = "SendLoginNoFormData";
                    return null;
                }

                // Get bfWebToken Data
                this.redirect = false;
                SetBaseHeaders(true, null, "https://login.beanfun.com/");
                string returnUrl = "https://tw.beanfun.com/beanfun_block/bflogin/return.aspx";
                string returnResponse = this.UploadString(returnUrl, payload);
                string setCookieHeader = this.ResponseHeaders?["Set-Cookie"];
                if (!string.IsNullOrEmpty(setCookieHeader))
                {
                    Match tokenMatch = Regex.Match(setCookieHeader, @"bfWebToken=([^;]+)");
                    if (tokenMatch.Success)
                        this.webtoken = tokenMatch.Groups[1].Value;
                }
                this.redirect = true;
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
                this.Headers.Add("User-Agent", userAgent);
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
                Regex regex = new Regex(@"[?&](?:skey|pSKey)=([^&]+)", RegexOptions.IgnoreCase);
                if (!regex.IsMatch(response))
                {
                    this.errmsg = "LoginNoSkey";
                    return null;
                }
                return HttpUtility.UrlDecode(regex.Match(response).Groups[1].Value);
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

                if (!string.IsNullOrEmpty(this.webtoken))
                {
                    CompleteExternalLogin(service_code, service_region);
                    return;
                }

                LoginCompleted(akey, service_code, service_region);
            }
            catch (Exception e)
            {
                if (e is WebException)
                {
                    this.errmsg =
                        (
                            System.Windows.Application.Current.TryFindResource(
                                "NetworkConnectionError"
                            ) as string
                        ) + e.Message;
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

        private void SetBaseHeaders(
            bool withReferer = false,
            string accept = null,
            string referer = null
        )
        {
            this.Headers.Clear();
            this.Headers.Add("User-Agent", userAgent);
            if (accept != null)
                this.Headers.Add("Accept", accept);
            if (withReferer && referer != null)
                this.Headers.Add("Referer", referer);
        }
    }
}
