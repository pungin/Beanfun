using System;
using System.Collections.Generic;
using System.Text;
using System.Net;
using System.Collections.Specialized;
using System.IO;

namespace Beanfun
{
    public partial class BeanfunClient : WebClient
    {
        private System.Net.CookieContainer CookieContainer;
        private Uri ResponseUri;
        public string errmsg;
        private string webtoken;
        public List<ServiceAccount> accountList;
        public int remainPoint = 0;
        public string accountAmountLimitNotice;
        bool redirect;
        private const string userAgent = "Mozilla/5.0 (Windows NT 10.0; WOW64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/55.0.2883.87 Safari/537.36";
        private string LoginToken;
        private string SessionKey;
        private string totpResponse;
        private string totpUrl;

        public int Timeout { get; set; }

        public string WebToken
        {
            get { return webtoken; }
        }

        public BeanfunClient()
        {
            this.redirect = true;
            this.CookieContainer = new System.Net.CookieContainer();
            this.Headers.Set("User-Agent", userAgent);
            this.Headers.Set("Accept-Encoding", "identity");
            this.Encoding = Encoding.UTF8;
            this.ResponseUri = null;
            this.errmsg = null;
            this.webtoken = null;
            this.accountList = new List<ServiceAccount>();
            this.accountAmountLimitNotice = "";
            this.Timeout = 30 * 1000;
        }

        public string DownloadString(string Uri, Encoding Encoding)
        {
            this.Headers.Set("User-Agent", userAgent);
            this.Headers.Set("Accept-Encoding", "identity");
            var ret = (Encoding.GetString(base.DownloadData(Uri)));
            return ret;
        }

        public new string DownloadString(string Uri)
        {
            
            this.Headers.Set("User-Agent", userAgent);
            this.Headers.Set("Accept-Encoding", "identity");
            var ret = base.DownloadString(Uri);
            return ret;
        }

        public string UploadString(string skey, NameValueCollection payload)
        {
            this.Headers.Set("User-Agent", userAgent);
            this.Headers.Set("Accept-Encoding", "identity");
            return Encoding.UTF8.GetString(base.UploadValues(skey, payload));
        }

        public string UploadStringGZip(string skey, NameValueCollection payload)
        {
            this.Headers.Set("User-Agent", userAgent);
            this.Headers.Set("Accept-Encoding", "gzip, deflate, br");
            byte[] byteArray = base.UploadValues(skey, payload);
            if (byteArray.Length <= 0) return "";
            if (byteArray[0] == 0x1F && byteArray[1] == 0x8B)
            {
                MemoryStream ms = new MemoryStream(byteArray);
                MemoryStream msTemp = new MemoryStream();
                int count = 0;
                System.IO.Compression.GZipStream gzip = new System.IO.Compression.GZipStream(ms, System.IO.Compression.CompressionMode.Decompress);
                byte[] buf = new byte[1000];

                while ((count = gzip.Read(buf, 0, buf.Length)) > 0)
                { msTemp.Write(buf, 0, count); }
                byteArray = msTemp.ToArray();
            }
            return Encoding.UTF8.GetString(byteArray);
        }

        protected override WebRequest GetWebRequest(Uri address)
        {
            HttpWebRequest webRequest = base.GetWebRequest(address) as HttpWebRequest;
            webRequest.Timeout = Timeout;

            if (webRequest != null)
            {
                webRequest.CookieContainer = this.CookieContainer;
                webRequest.AllowAutoRedirect = this.redirect;
            }
            return webRequest;
        }

        protected override WebResponse GetWebResponse(WebRequest request)
        {
            WebResponse webResponse = base.GetWebResponse(request);
            this.ResponseUri = webResponse.ResponseUri;
            return webResponse;
        }

        public CookieCollection GetCookies()
        {
            return this.CookieContainer.GetCookies(new Uri("https://" + (App.LoginRegion == "TW" ? "tw" : "bfweb.hk") + ".beanfun.com/"));
        }

        private string GetCookie(string cookieName)
        {
            foreach (Cookie cookie in GetCookies())
            {
                if (cookie.Name == cookieName)
                {
                    return cookie.Value;
                }
            }
            return null;
        }

        private string GetCurrentTime(int method = 0)
        {
            DateTime date = DateTime.Now;
            switch (method)
            {
                case 1:
                    return (date.Year - 1900).ToString() + (date.Month - 1).ToString() + date.ToString("ddHHmmssfff");
                case 2:
                    return date.Year.ToString() + (date.Month - 1).ToString() + date.ToString("ddHHmmssfff");
                default:
                    return date.ToString("yyyyMMddHHmmss.fff");
            }
        }

        public void Ping()
        {
            try
            {
                string url = "https://";
                if (App.LoginRegion == "TW")
                    url += "tw";
                else
                    url += "bfweb.hk";
                string ret = Encoding.GetString(this.DownloadData(url + ".beanfun.com/beanfun_block/generic_handlers/echo_token.ashx?webtoken=1"));

                Console.WriteLine(GetCurrentTime() + " @ " + ret);
            } catch {
            }
        }

        public int getRemainPoint()
        {
            string response = null;
            System.Text.RegularExpressions.Regex regex;

            string url = "https://";
            if (App.LoginRegion == "TW")
                url += "tw";
            else
                url += "bfweb.hk";
            response = this.DownloadString(url += ".beanfun.com/beanfun_block/generic_handlers/get_remain_point.ashx?webtoken=1");

            try
            {
                regex = new System.Text.RegularExpressions.Regex("\"RemainPoint\" : \"(.*)\" }");
                if (regex.IsMatch(response))
                    return int.Parse(regex.Match(response).Groups[1].Value);
                else
                    return 0;
            }
            catch
            { return 0; }
        }


        public string getEmail()
        {
            if (App.LoginRegion == "HK")
                return "";

            this.Headers.Set("Referer", @"https://tw.beanfun.com/");
            string response = this.DownloadString("https://tw.beanfun.com/beanfun_block/loader.ashx?service_code=999999&service_region=T0");
            System.Text.RegularExpressions.Regex regex = new System.Text.RegularExpressions.Regex("BeanFunBlock.LoggedInUserData.Email = \"(.*)\";BeanFunBlock.LoggedInUserData.MessageCount");
            if (regex.IsMatch(response))
                return regex.Match(response).Groups[1].Value;
            else
                return "";
        }
    }
}
