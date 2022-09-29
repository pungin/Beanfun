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
                response = this.DownloadString($"https://{host}/beanfun_block/game_zone/game_start_step2.aspx?service_code={service_code}&service_region={service_region}&sotp={acc.ssn}&dt={GetCurrentTime(2)}");
                Regex regex = new Regex("GetResultByLongPolling&key=(.*)\"");
                if (!regex.IsMatch(response))
                { this.errmsg = "OTPNoLongPollingKey:" + response; return null; }
                string longPollingKey = regex.Match(response).Groups[1].Value;
                string unkKey = null;
                string unkValue = null;
                if (App.LoginRegion == "TW")
                {
                    regex = new Regex("MyAccountData.ServiceAccountCreateTime \\+ \"(.*)=(.*)\";");
                    if (!regex.IsMatch(response))
                    { this.errmsg = "OTPNoUnkData"; return null; }
                    unkKey = Uri.UnescapeDataString(regex.Match(response).Groups[1].Value);
                    unkValue = Uri.UnescapeDataString(regex.Match(response).Groups[2].Value);
                }
                if (acc.screatetime == null)
                {
                    regex = new Regex("ServiceAccountCreateTime: \"([^\"]+)\"");
                    if (!regex.IsMatch(response))
                    { this.errmsg = "OTPNoCreateTime"; return null; }
                    acc.screatetime = regex.Match(response).Groups[1].Value;
                }
                response = this.DownloadString($"https://{loginHost}/generic_handlers/get_cookies.ashx");

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
                if (unkKey != null && unkValue != null)
                {
                    payload.Add(unkKey, unkValue);
                }
                // testing...
                System.Net.ServicePointManager.Expect100Continue = false;
                this.UploadString($"https://{host}/beanfun_block/generic_handlers/record_service_start.ashx", payload);
                response = this.DownloadString($"https://{host}/generic_handlers/get_result.ashx?meth=GetResultByLongPolling&key={longPollingKey}&_={GetCurrentTime()}");
                //Thread.Sleep(5000);
                //Console.WriteLine(Environment.TickCount);
                response = this.DownloadString($"https://{host}/beanfun_block/generic_handlers/get_webstart_otp.ashx?SN={longPollingKey}&WebToken={this.WebToken}&SecretCode={secretCode}&ppppp=1F552AEAFF976018F942B13690C990F60ED01510DDF89165F1658CCE7BC21DBA&ServiceCode={service_code}&ServiceRegion={service_region}&ServiceAccount={acc.sid}&CreateTime={acc.screatetime.Replace(" ", "%20")}&d={Environment.TickCount}");
                if (response == null || response == "")
                { this.errmsg = "OTPNoResponse"; return null; }
                string[] responses = response.Split(';');
                if (responses.Length < 2)
                { this.errmsg = "OTPNoResponse"; return null; }
                response = responses[1];
                if (responses[0] != "1")
                { this.errmsg = System.Windows.Application.Current.TryFindResource("GetOtpError") as string + "\r\n" + response; return null; }
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
                this.errmsg = System.Windows.Application.Current.TryFindResource("GetOtpError") as string + "\n\n" + e.Message + "\n" + e.StackTrace;
                return null;
            }
        }

    }
}
