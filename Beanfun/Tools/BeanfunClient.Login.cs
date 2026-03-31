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
        private string RegularLogin(
            string id,
            string pass,
            string skey,
            string service_code = "610074",
            string service_region = "T9"
        )
        {
            if (App.LoginRegion == "TW")
                return TwRegularLogin(id, pass, skey, service_code, service_region);
            else
                return HkRegularLogin(id, pass, skey);
        }

        private string TwRegularLogin(
            string id,
            string pass,
            string skey,
            string service_code,
            string service_region
        )
        {
            try
            {
                string apiBase = "https://login.beanfun.com";
                string indexUrl = $"{apiBase}/Login/Index?pSKey={skey}";

                // Step 1: Get index page and antiforgery token
                SetBaseHeaders(false, "text/html");
                string indexHtml = this.DownloadString(indexUrl);
                string formToken = Regex
                    .Match(indexHtml, "name=\"__RequestVerificationToken\"[^>]+value=\"([^\"]+)\"")
                    .Groups[1]
                    .Value;

                if (string.IsNullOrEmpty(formToken))
                {
                    this.errmsg = "LoginNoToken";
                    return null;
                }

                // Step 2: Check account type
                SetJsonHeaders(formToken, indexUrl);
                string checkTypeBody = new JObject
                {
                    ["Account"] = id,
                    ["Captcha"] = "",
                    ["__RequestVerificationToken"] = formToken,
                }.ToString(Newtonsoft.Json.Formatting.None);
                string checkTypeRes = this.UploadString(
                    $"{apiBase}/Login/CheckAccountType?pSKey={skey}",
                    "POST",
                    checkTypeBody
                );

                string captchaToken = "";
                if (
                    !string.IsNullOrWhiteSpace(checkTypeRes)
                    && checkTypeRes.TrimStart().StartsWith("{")
                )
                {
                    var checkJson = JObject.Parse(checkTypeRes);
                    captchaToken = checkJson["ResultData"]?["Captcha"]?.ToString() ?? "";
                }

                // Step 3: Account login
                SetJsonHeaders(formToken, indexUrl);
                string loginBody = new JObject
                {
                    ["Account"] = id,
                    ["Pasw"] = pass,
                    ["IsMobile"] = false,
                    ["Captcha"] = captchaToken,
                    ["__RequestVerificationToken"] = formToken,
                }.ToString(Newtonsoft.Json.Formatting.None);
                string loginRes = this.UploadString(
                    $"{apiBase}/Login/AccountLogin?pSKey={skey}",
                    "POST",
                    loginBody
                );

                var loginJson = JObject.Parse(loginRes);
                string resultCode = loginJson["ResultCode"]?.ToString();
                string result = loginJson["Result"]?.ToString();
                string resultMsg = loginJson["ResultMessage"]?.ToString() ?? "";

                if (resultCode == "1")
                {
                    if (result == "1")
                    {
                        this.errmsg = "LoginAdvanceCheck";
                        return null;
                    }

                    // Use SendLogin flow (same as QRCode) to get bfWebToken
                    SetBaseHeaders(
                        true,
                        "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
                        indexUrl
                    );
                    string sendLoginHtml = this.DownloadString($"{apiBase}/Login/SendLogin");

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
                        this.errmsg = "SendLoginNoFormData";
                        return null;
                    }

                    this.redirect = false;
                    SetBaseHeaders(true, null, $"{apiBase}/");
                    string returnResponse = this.UploadString(
                        "https://tw.beanfun.com/beanfun_block/bflogin/return.aspx",
                        payload
                    );
                    string setCookieHeader = this.ResponseHeaders?["Set-Cookie"];
                    if (!string.IsNullOrEmpty(setCookieHeader))
                    {
                        Match tokenMatch = Regex.Match(setCookieHeader, @"bfWebToken=([^;]+)");
                        if (tokenMatch.Success)
                            this.webtoken = tokenMatch.Groups[1].Value;
                    }
                    this.redirect = true;

                    if (string.IsNullOrEmpty(this.webtoken))
                    {
                        this.errmsg = "LoginNoWebtoken";
                        return null;
                    }

                    GetAccounts(service_code, service_region, false);
                    if (this.errmsg != null)
                        return null;

                    this.remainPoint = getRemainPoint();
                    this.errmsg = null;
                    return null; // return null with no errmsg = success, skip LoginCompleted
                }

                // ResultCode 2: AdvanceCheck required
                if (resultCode == "2" && resultMsg.StartsWith("http"))
                    return HandleAdvanceCheck(resultMsg);

                this.errmsg = resultMsg;
                return null;
            }
            catch (Exception ex)
            {
                Debug.WriteLine($"[TwRegularLogin] Exception: {ex.Message}");
                this.errmsg = "LoginUnknown\n\n" + ex.Message + "\n" + ex.StackTrace;
                return null;
            }
        }

        private string HandleAdvanceCheck(string advanceUrl)
        {
            if (advanceUrl.Contains("gamaweb.beanfun.com"))
            {
                this.errmsg = "AdvanceCheckAccountAbnormal";
                return null;
            }

            string advanceHtml = this.DownloadString(advanceUrl);

            var captchaMatch = Regex.Match(
                advanceHtml,
                "id=\"LBD_VCID_[^\"]+\"[^>]+value=\"([^\"]+)\""
            );
            if (!captchaMatch.Success)
            {
                this.errmsg = "AdvanceCheckNoCaptchaId";
                return null;
            }
            string captchaId = captchaMatch.Groups[1].Value;

            string viewState = Regex
                .Match(advanceHtml, "id=\"__VIEWSTATE\"[^>]+value=\"([^\"]+)\"")
                .Groups[1]
                .Value;
            string viewStateGen = Regex
                .Match(advanceHtml, "id=\"__VIEWSTATEGENERATOR\"[^>]+value=\"([^\"]+)\"")
                .Groups[1]
                .Value;
            string eventValidation = Regex
                .Match(advanceHtml, "id=\"__EVENTVALIDATION\"[^>]+value=\"([^\"]+)\"")
                .Groups[1]
                .Value;
            string captchaImgSrc = Regex
                .Match(advanceHtml, "class=\"LBD_CaptchaImage\"[^>]+src=\"([^\"]+)\"")
                .Groups[1]
                .Value;
            string emailHint = Regex
                .Match(advanceHtml, "id=\"lblAuthType\">([^<]+)<")
                .Groups[1]
                .Value;
            string formAction = Regex
                .Match(advanceHtml, "action=\"(AdvanceCheck\\.aspx[^\"]+)\"")
                .Groups[1]
                .Value;

            string captchaImgUrl =
                $"https://tw.newlogin.beanfun.com/LoginCheck/{captchaImgSrc.TrimStart('/')}".Replace(
                    "&amp;",
                    "&"
                );
            string submitUrl =
                $"https://tw.newlogin.beanfun.com/LoginCheck/{formAction.Replace("&amp;", "&")}";

            byte[] captchaImgBytes = null;
            try
            {
                captchaImgBytes = this.DownloadData(captchaImgUrl);
            }
            catch { }

            if (captchaImgBytes == null || captchaImgBytes.Length < 500)
            {
                this.errmsg = "AdvanceCheckCaptchaLoadFailed";
                return null;
            }

            string userEmail = "",
                userCaptcha = "";
            bool? dialogResult = false;

            App.Current.Dispatcher.Invoke(() =>
            {
                var dlg = new AdvanceCheckDialog(emailHint, captchaImgBytes);
                dialogResult = dlg.ShowDialog();
                if (dialogResult == true)
                {
                    userEmail = dlg.Email;
                    userCaptcha = dlg.CaptchaCode;
                }
            });

            if (dialogResult != true || string.IsNullOrEmpty(userEmail))
            {
                this.errmsg = "LoginAdvanceCheck";
                return null;
            }

            // Submit advance check form
            SetBaseHeaders(true, null, advanceUrl);
            this.Headers[HttpRequestHeader.ContentType] = "application/x-www-form-urlencoded";

            string submitBody =
                $"__VIEWSTATE={Uri.EscapeDataString(viewState)}"
                + $"&__VIEWSTATEGENERATOR={Uri.EscapeDataString(viewStateGen)}"
                + $"&__EVENTVALIDATION={Uri.EscapeDataString(eventValidation)}"
                + $"&txtVerify={Uri.EscapeDataString(userEmail)}"
                + $"&CodeTextBox={Uri.EscapeDataString(userCaptcha)}"
                + $"&LBD_VCID_c_logincheck_advancecheck_samplecaptcha={Uri.EscapeDataString(captchaId)}"
                + "&imgbtnSubmit.x=30&imgbtnSubmit.y=10";

            string submitRes = this.UploadString(submitUrl, "POST", submitBody);

            if (submitRes.Contains("驗證成功") || submitRes.Contains("再次輸入您的帳號密碼"))
            {
                this.errmsg = "AdvanceCheckSuccessRetry";
                return null;
            }

            string akeyFinal = Regex
                .Match(this.ResponseUri?.ToString() ?? "", @"akey=([^&]+)")
                .Groups[1]
                .Value;
            return string.IsNullOrEmpty(akeyFinal) ? null : akeyFinal;
        }

        private string HkRegularLogin(string id, string pass, string skey)
        {
            string loginHost = "login.hk.beanfun.com";
            try
            {
                string url = $"https://{loginHost}/login/id-pass_form_newBF.aspx?otp1={skey}";
                string response = this.DownloadString(url);

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

                NameValueCollection payload = new NameValueCollection();
                payload.Add("__EVENTTARGET", "");
                payload.Add("__EVENTARGUMENT", "");
                payload.Add("__VIEWSTATE", viewstate);
                payload.Add("__VIEWSTATEGENERATOR", viewstateGenerator);
                payload.Add("__VIEWSTATEENCRYPTED", "");
                payload.Add("__EVENTVALIDATION", eventvalidation);
                payload.Add("t_AccountID", id);
                payload.Add("t_Password", pass);
                payload.Add("btn_login", "登入");

                response = this.UploadString(url, payload);

                if (response.Contains("RELOAD_CAPTCHA_CODE") && response.Contains("alert"))
                {
                    this.errmsg = "LoginAdvanceCheck";
                    return null;
                }

                if (response.Contains("totpLoginBtn"))
                {
                    this.totpResponse = response;
                    this.totpUrl = url;
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
                return regex.Match(this.ResponseUri.ToString()).Groups[1].Value;
            }
            catch (Exception e)
            {
                this.errmsg = "LoginUnknown\n\n" + e.Message + "\n" + e.StackTrace;
                return null;
            }
        }

        private void SetJsonHeaders(string verificationToken, string referer)
        {
            SetBaseHeaders(true, "application/json, text/plain, */*", referer);
            this.Headers.Add("X-Requested-With", "XMLHttpRequest");
            this.Headers.Add("RequestVerificationToken", verificationToken);
            this.Headers[HttpRequestHeader.ContentType] = "application/json; charset=utf-8";
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
            public string deeplink;
        }

        public QRCodeClass GetQRCodeValue(string skey)
        {
            SetBaseHeaders(false, "text/html");
            string url = $"https://login.beanfun.com/Login/Index?pSKey={skey}";
            string response = this.DownloadString(url);

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
                SetBaseHeaders(
                    true,
                    "application/json, text/plain, */*",
                    $"https://login.beanfun.com/Login/Index?pSKey={skey}"
                );
                string response = this.DownloadString("https://login.beanfun.com/QRLogin/QRLogin");
                Debug.WriteLine("QRLogin response: " + response);

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
                string result;
                this.Headers.Add("User-Agent", "Mozilla/5.0");
                this.Headers.Add("Accept", "application/json, text/plain, */*");
                this.Headers.Add("Referer", $"https://login.beanfun.com/Login/Index?pSKey={skey}");
                this.Headers.Add("Origin", "https://login.beanfun.com");

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
                    return -2;
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
                this.DownloadString(
                    "https://tw.beanfun.com/beanfun_block/bflogin/default.aspx?service=999999_T0"
                );
                string finalUrl = this.ResponseUri?.ToString();
                if (string.IsNullOrEmpty(finalUrl))
                {
                    this.errmsg = "LoginNoResponse";
                    return null;
                }

                Regex regex = new Regex(@"[sp][Ss]?[Kk]ey=([^&]+)");
                if (!regex.IsMatch(finalUrl))
                {
                    this.errmsg = "LoginNoSkey";
                    return null;
                }
                return regex.Match(finalUrl).Groups[1].Value;
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
                        akey = RegularLogin(id, pass, SessionKey, service_code, service_region);
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

        public void GamePassLogin(
            string webToken,
            System.Collections.Generic.IEnumerable<System.Net.Cookie> cookies,
            string service_code = "610074",
            string service_region = "T9"
        )
        {
            try
            {
                // Sync all cookies from WebView2 to BeanfunClient
                foreach (var cookie in cookies)
                {
                    SetCookie(
                        cookie.Name,
                        cookie.Value,
                        cookie.Domain.TrimStart('.'),
                        string.IsNullOrEmpty(cookie.Path) ? "/" : cookie.Path
                    );
                }

                this.webtoken = webToken;

                GetAccounts(service_code, service_region, false);
                if (this.errmsg != null)
                    return;

                this.remainPoint = getRemainPoint();
                this.errmsg = null;
            }
            catch (Exception e)
            {
                this.errmsg = "LoginUnknown\n\n" + e.Message + "\n" + e.StackTrace;
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
            response = this.DownloadString($"https://{host}/{this.ResponseHeaders["Location"]}");
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
            this.Headers.Add(
                "User-Agent",
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36"
            );
            if (accept != null)
                this.Headers.Add("Accept", accept);
            if (withReferer && referer != null)
                this.Headers.Add("Referer", referer);
        }
    }
}
