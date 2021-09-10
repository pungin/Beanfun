using System;
using System.Text;
using System.Net;
using System.Text.RegularExpressions;
using System.Collections.Specialized;
using Newtonsoft.Json.Linq;

namespace Beanfun
{
    public partial class BeanfunClient : WebClient
    {

        public string GetOTP(ServiceAccount acc, string service_code="610074", string service_region="T9")
        {
            try
            {
                string response;
                if (App.LoginRegion == "TW")
                {
                    response = this.DownloadString("https://tw.beanfun.com/beanfun_block/game_zone/game_start_step2.aspx?service_code=" + service_code + "&service_region=" + service_region + "&sotp=" + acc.ssn + "&dt=" + GetCurrentTime(2));
                    Regex regex = new Regex("GetResultByLongPolling&key=(.*)\"");
                    if (!regex.IsMatch(response))
                    { this.errmsg = "OTPNoLongPollingKey:" + response; return null; }
                    string longPollingKey = regex.Match(response).Groups[1].Value;
                    if (acc.screatetime == null)
                    {
                        regex = new Regex("ServiceAccountCreateTime: \"([^\"]+)\"");
                        if (!regex.IsMatch(response))
                        { this.errmsg = "OTPNoCreateTime"; return null; }
                        acc.screatetime = regex.Match(response).Groups[1].Value;
                    }
                    response = this.DownloadString("https://tw.newlogin.beanfun.com/generic_handlers/get_cookies.ashx");

                    regex = new Regex("var m_strSecretCode = '(.*)';");
                    if (!regex.IsMatch(response))
                    { this.errmsg = "OTPNoSecretCode"; return null; }
                    string secretCode = regex.Match(response).Groups[1].Value;

                    NameValueCollection payload = new NameValueCollection();
                    payload.Add("service_code", service_code);
                    payload.Add("service_region", service_region);
                    payload.Add("service_account_id", acc.sid);
                    payload.Add("sotp", acc.ssn);
                    payload.Add("service_account_display_name", acc.sname);
                    payload.Add("service_account_create_time", acc.screatetime);
                    // testing...
                    System.Net.ServicePointManager.Expect100Continue = false;
                    this.UploadString("https://tw.beanfun.com/beanfun_block/generic_handlers/record_service_start.ashx", payload);
                    response = this.DownloadString("https://tw.beanfun.com/generic_handlers/get_result.ashx?meth=GetResultByLongPolling&key=" + longPollingKey + "&_=" + GetCurrentTime());
                    //Thread.Sleep(5000);
                    //Console.WriteLine(Environment.TickCount);
                    response = this.DownloadString("https://tw.beanfun.com/beanfun_block/generic_handlers/get_webstart_otp.ashx?SN=" + longPollingKey + "&WebToken=" + this.webtoken + "&SecretCode=" + secretCode + "&ppppp=1F552AEAFF976018F942B13690C990F60ED01510DDF89165F1658CCE7BC21DBA&ServiceCode=" + service_code + "&ServiceRegion=" + service_region + "&ServiceAccount=" + acc.sid + "&CreateTime=" + acc.screatetime.Replace(" ", "%20") + "&d=" + Environment.TickCount);
                }
                else
                {
                    response = this.DownloadString("http://hk.beanfun.com/beanfun_block/auth.aspx?channel=game_zone&page_and_query=game_start_step2.aspx%3Fservice_code%3D" + service_code + "%26service_region%3D" + service_region + "%26service_account_sn%3D" + acc.ssn + "&token=" + bfServ.Token);
                    if (response == "")
                    { this.errmsg = "OTPNoResponse"; return null; }
                    Regex regex = new Regex("<span id=\"lblLoginFailMsg\">(.*)</span>");
                    if (regex.IsMatch(response))
                    { this.errmsg = "OTPNoLongPollingKey:" + regex.Match(response).Groups[1].Value; return null; }
                    regex = new Regex("var MyAccountData = (.*);");
                    if (!regex.IsMatch(response))
                    { this.errmsg = "OTPNoMyAccountData"; return null; }
                    JObject json = JObject.Parse(regex.Match(response).Groups[1].Value);
                    acc.sname = (string)json["ServiceAccountDisplayName"];
                    acc.screatetime = (string)json["ServiceAccountCreateTime"];

                    NameValueCollection payload = new NameValueCollection();
                    payload.Add("service_code", service_code);
                    payload.Add("service_region", service_region);
                    payload.Add("service_account_id", acc.sid);
                    payload.Add("service_sotp", acc.ssn);
                    payload.Add("service_display_name", acc.sname);
                    payload.Add("service_create_time", acc.screatetime);
                    // testing...
                    System.Net.ServicePointManager.Expect100Continue = false;
                    this.UploadString("http://hk.beanfun.com/beanfun_block/generic_handlers/record_service_start.ashx", payload);
                    response = this.DownloadString("http://hk.beanfun.com/beanfun_block/generic_handlers/get_otp.ashx?ppppp=&token=" + bfServ.Token + "&account_service_code=" + service_code + "&account_service_region=" + service_region + "&service_account_id=" + acc.sid + "&create_time=" + acc.screatetime.Replace(" ", "%20") + "&d=" + Environment.TickCount);
                }
                if (response == null || response == "")
                { this.errmsg = "OTPNoResponse"; return null; }
                string[] responses = response.Split(';');
                if (responses.Length < 2)
                { this.errmsg = "OTPNoResponse"; return null; }
                response = responses[1];
                if (responses[0] != "1")
                { this.errmsg = "密碼取得失敗。\r\n" + response; return null; }
                string key = response.Substring(0, 8);
                string plain = response.Substring(8);
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
            catch (Exception e)
            {
                this.errmsg = "密碼取得失敗。\n\n" + e.Message + "\n" + e.StackTrace;
                return null;
            }
        }

    }
}
