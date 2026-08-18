using System;
using System.Collections.Specialized;
using System.Net;
using System.Text.RegularExpressions;
using Newtonsoft.Json.Linq;

namespace Beanfun
{
    public partial class BeanfunClient : WebClient
    {
        /// <summary>
        /// game_start_step2.aspx 為原生啟動器（GGM）準備的 m_objData 交接資料。
        /// region 成員固定是 "TW;Production"，我們送出的請求都不帶它，所以忽略。
        /// </summary>
        private class LaunchHandoff
        {
            public string Sn;
            public string Data;

            /// <summary>只有帶舊 payload 的頁面才會宣告這兩個。</summary>
            public string WebToken;
            public string SecretCode;
        }

        /// <summary>
        /// 取遊戲密碼。
        ///
        /// 台服在 2026 年 8 月把開始遊戲流程搬到橘子遊戲大廳（GGM），頁面不再自己
        /// 組 OTP 網址，而是丟出一個 m_objData 交接給啟動器（issue #368）。所以路線
        /// 由「頁面自己的形狀」決定，不是由地區決定：頁面帶 m_objData 就走交接路線，
        /// 其餘一律照舊。港服現在不必改，將來就算也搬過去了同樣不用改。
        ///
        /// 交接路線還要再分一次，而且一定要先解密才知道分到哪邊（issue #376）：
        /// m_objData.data 有兩種 payload，楓之谷給的是 LaunchTicket，要送
        /// get_webstart_otp_v2.ashx；其他遊戲給的是舊的 ppppp 參數組，還是送
        /// get_webstart_otp.ashx。兩種頁面從外面看一模一樣。
        /// </summary>
        public string GetOTP(
            ServiceAccount acc,
            string service_code = "610074",
            string service_region = "T9"
        )
        {
            try
            {
                string response;
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
                // generic_handlers 底下的處理器開始檢查 Referer，沒帶會回
                // 「The URL referrer is null or from a different domain!」。
                // 同源加上 beanfun 的 strict-origin-when-cross-origin 政策，
                // 瀏覽器送的就是整串網址，所以我們也原樣送。
                string pageUrl =
                    $"https://{host}/beanfun_block/game_zone/game_start_step2.aspx?service_code={service_code}&service_region={service_region}&sotp={acc.ssn}&dt={GetCurrentTime(2)}";
                response = this.DownloadString(pageUrl);

                // 先讀交接資料：它決定這頁走哪條路線，也就決定了下面哪些字串
                // 是必要的。已遷移的頁面根本不再有 GetResultByLongPolling&key=，
                // 因為它不再輪詢而是開啟動器；先解那些會在讀到交接資料之前就
                // 讓整個流程失敗，而且報的錯會指著一個「本來就該消失」的字串。
                LaunchHandoff handoff = ParseLaunchHandoff(response);

                Regex regex = new Regex("GetResultByLongPolling&key=(.*)\"");
                string longPollingKey = null;
                if (regex.IsMatch(response))
                {
                    longPollingKey = regex.Match(response).Groups[1].Value;
                }
                else if (handoff == null)
                {
                    this.errmsg = "OTPNoLongPollingKey:" + response;
                    return null;
                }

                string unkKey = null;
                string unkValue = null;
                if (App.LoginRegion == "TW")
                {
                    regex = new Regex("MyAccountData.ServiceAccountCreateTime \\+ \"(.*)=(.*)\";");
                    if (regex.IsMatch(response))
                    {
                        unkKey = Uri.UnescapeDataString(regex.Match(response).Groups[1].Value);
                        unkValue = Uri.UnescapeDataString(regex.Match(response).Groups[2].Value);
                    }
                    else if (handoff == null)
                    {
                        // 已遷移的頁面不必再帶這個防偽欄位，有就用，沒有不該
                        // 讓使用者拿不到密碼。
                        this.errmsg = "OTPNoUnkData";
                        return null;
                    }
                }
                if (string.IsNullOrEmpty(acc.screatetime))
                {
                    regex = new Regex("ServiceAccountCreateTime: \"([^\"]+)\"");
                    if (regex.IsMatch(response))
                    {
                        acc.screatetime = regex.Match(response).Groups[1].Value;
                    }
                    else if (handoff == null)
                    {
                        this.errmsg = "OTPNoCreateTime";
                        return null;
                    }
                }

                if (handoff != null)
                {
                    // 記錄開始遊戲是頁面開啟動器時順手做的事，後面沒有任何東西
                    // 依賴它，所以已遷移頁面少帶一個欄位不該賠上使用者的密碼。
                    try
                    {
                        RecordServiceStart(
                            host,
                            pageUrl,
                            acc,
                            service_code,
                            service_region,
                            unkKey,
                            unkValue
                        );
                    }
                    catch (Exception e)
                    {
                        Console.WriteLine("record_service_start failed, continuing: " + e.Message);
                    }

                    // 這條路線刻意不取 SecretCode 也不長輪詢：v2 請求不帶
                    // SecretCode，而頁面那個 GetResultByLongPolling 是啟動器的
                    // 安裝檢查，跟密碼無關又會把連線掛著。
                    LaunchPayload payload = LaunchData.Decode(handoff.Data);
                    if (payload == null)
                    {
                        this.errmsg = "DecryptOTPError";
                        return null;
                    }

                    ClientIntegrity integrity = ClientIntegrity.Resolve();
                    Console.WriteLine(
                        "[OTP] handoff route="
                            + (payload.LaunchTicket != null ? "v2" : "pre-v2")
                            + " CV="
                            + integrity.CV
                            + " arch="
                            + integrity.Arch
                    );

                    if (payload.LaunchTicket != null)
                        return GetOtpV2(host, pageUrl, handoff, payload.LaunchTicket, integrity);

                    // 頁面通常會跟 blob 一起宣告這兩個，但沒有觀察到保證；舊流程
                    // 本來就各有來源，所以掉回去用而不是直接放棄。
                    string webToken = handoff.WebToken ?? this.WebToken;
                    string secretCode = handoff.SecretCode;
                    if (secretCode == null)
                    {
                        secretCode = GetSecretCode(loginHost);
                        if (secretCode == null)
                            return null;
                    }
                    return GetOtpPreV2FromHandoff(
                        host,
                        pageUrl,
                        handoff,
                        payload.Legacy,
                        webToken,
                        secretCode,
                        integrity
                    );
                }

                string secretCodeLegacy = GetSecretCode(loginHost);
                if (secretCodeLegacy == null)
                    return null;

                RecordServiceStart(
                    host,
                    pageUrl,
                    acc,
                    service_code,
                    service_region,
                    unkKey,
                    unkValue
                );
                this.DownloadString(
                    $"https://{host}/generic_handlers/get_result.ashx?meth=GetResultByLongPolling&key={longPollingKey}&_={GetCurrentTime()}"
                );

                string url =
                    $"https://{host}/beanfun_block/generic_handlers/get_webstart_otp.ashx?SN={longPollingKey}&WebToken={this.WebToken}&SecretCode={secretCodeLegacy}&ppppp={PPPPP_LITERAL}&ServiceCode={service_code}&ServiceRegion={service_region}&ServiceAccount={acc.sid}&CreateTime={(acc.screatetime ?? "").Replace(" ", "%20")}&d={Environment.TickCount}";
                if (App.LoginRegion == "TW")
                {
                    // CV/Hash/arch 是遊戲大廳的慣例，而遊戲大廳是台服的產品；
                    // 港服的舊端點沒有觀察到需要它們，請求維持原樣。
                    url += IntegritySuffix(ClientIntegrity.Resolve());
                }
                response = this.DownloadString(url);
                return DecryptEnvelope(response);
            }
            catch (Exception e)
            {
                this.errmsg =
                    (System.Windows.Application.Current.TryFindResource("GetOtpError") as string)
                    + "\n\n"
                    + e.Message
                    + "\n"
                    + e.StackTrace;
                return null;
            }
        }

        /// <summary>
        /// 送 ppppp= 的 64 字元常數。舊版 WPF 寫死的值，來源不明；已遷移的頁面
        /// 會在 blob 裡給出當下真正的值（目前是 96 字元），所以那條路線不用它。
        /// </summary>
        private const string PPPPP_LITERAL =
            "1F552AEAFF976018F942B13690C990F60ED01510DDF89165F1658CCE7BC21DBA";

        /// <summary>取 m_strSecretCode；失敗時設好 errmsg 並回 null。</summary>
        private string GetSecretCode(string loginHost)
        {
            string response = this.DownloadString(
                $"https://{loginHost}/generic_handlers/get_cookies.ashx"
            );
            Regex regex = new Regex("var m_strSecretCode = '(.*)';");
            if (!regex.IsMatch(response))
            {
                this.errmsg = "OTPNoSecretCode";
                return null;
            }
            return regex.Match(response).Groups[1].Value;
        }

        private void RecordServiceStart(
            string host,
            string pageUrl,
            ServiceAccount acc,
            string service_code,
            string service_region,
            string unkKey,
            string unkValue
        )
        {
            NameValueCollection payload = new NameValueCollection();
            payload.Add("service_code", service_code);
            payload.Add("service_region", service_region);
            payload.Add("service_account_id", acc.sid);
            payload.Add("sotp", acc.ssn);
            payload.Add("service_account_display_name", acc.sname);
            payload.Add("service_account_create_time", acc.screatetime ?? "");
            if (unkKey != null && unkValue != null)
            {
                payload.Add(unkKey, unkValue);
            }
            System.Net.ServicePointManager.Expect100Continue = false;
            this.Headers.Set("Referer", pageUrl);
            try
            {
                this.UploadString(
                    $"https://{host}/beanfun_block/generic_handlers/record_service_start.ashx",
                    payload
                );
            }
            finally
            {
                this.Headers.Remove("Referer");
            }
        }

        /// <summary>CV/Hash/arch 三件套，照 GGM 的 BuildOtpUrl 的順序附在最後。</summary>
        private static string IntegritySuffix(ClientIntegrity integrity)
        {
            return "&CV="
                + Uri.EscapeDataString(integrity.CV)
                + "&Hash="
                + Uri.EscapeDataString(integrity.Hash)
                + "&arch="
                + Uri.EscapeDataString(integrity.Arch);
        }

        /// <summary>
        /// 交接路線的舊端點：同一個 get_webstart_otp.ashx，但每個值都來自頁面
        /// 而不是我們自己的 session。最關鍵的是 ppppp — 舊版寫死的常數已經過時，
        /// 現行的值就在 blob 裡。
        /// </summary>
        private string GetOtpPreV2FromHandoff(
            string host,
            string pageUrl,
            LaunchHandoff handoff,
            LegacyOtpParams p,
            string webToken,
            string secretCode,
            ClientIntegrity integrity
        )
        {
            // 跟舊組法一樣：這些值裡只有 CreateTime 的空白需要編碼。
            string url =
                $"https://{host}/beanfun_block/generic_handlers/get_webstart_otp.ashx?SN={handoff.Sn}&WebToken={webToken}&SecretCode={secretCode}&ppppp={p.ppppp}&ServiceCode={p.ServiceCode}&ServiceRegion={p.ServiceRegion}&ServiceAccount={p.ServiceAccount}&CreateTime={(p.CreateTime ?? "").Replace(" ", "%20")}&d={Environment.TickCount}"
                + IntegritySuffix(integrity);

            this.Headers.Set("Referer", pageUrl);
            string response;
            try
            {
                response = this.DownloadString(url);
            }
            finally
            {
                this.Headers.Remove("Referer");
            }
            return DecryptEnvelope(response);
        }

        /// <summary>
        /// v2 端點：POST 一包 JSON，OTP 藏在回應的 data 裡，構造跟舊信封第二段
        /// 相同（8 字元 ASCII key + hex 密文）。
        ///
        /// 它不是舊端點的替代品，只是同輩 — 遊戲交出哪種 payload 就找哪個端點。
        /// 舊端點回 Query String Error 代表請求組錯了，不代表它不在了。
        /// </summary>
        private string GetOtpV2(
            string host,
            string pageUrl,
            LaunchHandoff handoff,
            string launchTicket,
            ClientIntegrity integrity
        )
        {
            var body = new JObject
            {
                ["SN"] = handoff.Sn,
                ["LaunchTicket"] = launchTicket,
                ["CV"] = integrity.CV,
                ["Hash"] = integrity.Hash,
                ["arch"] = integrity.Arch,
            };

            string url = $"https://{host}/beanfun_block/generic_handlers/get_webstart_otp_v2.ashx";
            this.Headers.Set("User-Agent", userAgent);
            this.Headers.Set("Accept-Encoding", "identity");
            this.Headers.Set("Content-Type", "application/json; charset=utf-8");
            this.Headers.Set("Referer", pageUrl);
            string response;
            try
            {
                response = base.UploadString(url, body.ToString(Newtonsoft.Json.Formatting.None));
            }
            finally
            {
                this.Headers.Remove("Referer");
                this.Headers.Remove("Content-Type");
            }

            if (string.IsNullOrEmpty(response))
            {
                this.errmsg = "OTPNoResponse";
                return null;
            }

            JObject parsed;
            try
            {
                parsed = JObject.Parse(response);
            }
            catch
            {
                this.errmsg = "OTPNoResponse";
                return null;
            }

            if (parsed.Value<int?>("result") != 1)
            {
                string message = parsed.Value<string>("message");
                if (string.IsNullOrEmpty(message))
                    message = "result=" + parsed.Value<string>("result");
                this.errmsg =
                    (System.Windows.Application.Current.TryFindResource("GetOtpError") as string)
                    + "\r\n"
                    + message;
                return null;
            }

            string data = parsed.Value<string>("data");
            if (string.IsNullOrEmpty(data))
            {
                this.errmsg = "OTPNoResponse";
                return null;
            }
            return DecryptOtpPayload(data);
        }

        /// <summary>從 step 1 的回應抓出 m_objData；沒有就回 null（代表走舊路線）。</summary>
        private static LaunchHandoff ParseLaunchHandoff(string html)
        {
            if (string.IsNullOrEmpty(html))
                return null;
            // 頁面會把這段排版成多行，所以要 Singleline；非貪婪的 .*? 停在第一個
            // 右大括號，也就是這個扁平物件的結尾。
            Match block = Regex.Match(
                html,
                @"var m_objData\s*=\s*\{(.*?)\}",
                RegexOptions.Singleline
            );
            if (!block.Success)
                return null;
            string inner = block.Groups[1].Value;

            string sn = MemberOf(inner, "sn");
            string data = MemberOf(inner, "data");
            if (string.IsNullOrEmpty(sn) || string.IsNullOrEmpty(data))
                return null;

            return new LaunchHandoff
            {
                Sn = sn,
                Data = data,
                WebToken = NullIfEmpty(MemberOf(inner, "webToken")),
                SecretCode = NullIfEmpty(MemberOf(inner, "secretCode")),
            };
        }

        /// <summary>只在物件字面值內部找，免得這種通用的鍵名比對到別的地方。</summary>
        private static string MemberOf(string objectLiteral, string name)
        {
            Match m = Regex.Match(objectLiteral, "\"" + name + "\"\\s*:\\s*\"([^\"]*)\"");
            return m.Success ? m.Groups[1].Value : null;
        }

        private static string NullIfEmpty(string value)
        {
            return string.IsNullOrEmpty(value) ? null : value;
        }

        /// <summary>拆 "1;{key}{密文hex}" 這個舊信封。</summary>
        private string DecryptEnvelope(string response)
        {
            if (string.IsNullOrEmpty(response))
            {
                this.errmsg = "OTPNoResponse";
                return null;
            }
            string[] responses = response.Split(';');
            if (responses.Length < 2)
            {
                this.errmsg = "OTPNoResponse";
                return null;
            }
            if (responses[0] != "1")
            {
                this.errmsg =
                    (System.Windows.Application.Current.TryFindResource("GetOtpError") as string)
                    + "\r\n"
                    + responses[1];
                return null;
            }
            return DecryptOtpPayload(responses[1]);
        }

        /// <summary>
        /// 解 "{8 字元 ASCII key}{密文 hex}"。兩種協定共用：舊信封放在 "1;" 之後，
        /// v2 則是 JSON 的 data 成員。
        /// </summary>
        private string DecryptOtpPayload(string payload)
        {
            if (payload == null || payload.Length < 8)
            {
                this.errmsg = "DecryptOTPError";
                return null;
            }
            string key = payload.Substring(0, 8);
            string plain = payload.Substring(8);
            string otp = WCDESComp.DecryStrHex(plain, key);
            if (otp != null)
            {
                otp = otp.Trim('\0');
                this.errmsg = null;
            }
            else
            {
                this.errmsg = "DecryptOTPError";
            }
            return otp;
        }
    }
}
