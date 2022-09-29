using System;
using System.IO;
using System.Net;

namespace Amemiya.Net
{
    class WebClientEx : WebClient
    {
        public WebClientEx() : this(30 * 1000) { }

        public WebClientEx(int timeout)
        {
            Timeout = timeout;
        }

        public int Timeout { get; set; }

        protected override WebRequest GetWebRequest(Uri address)
        {
            var request = base.GetWebRequest(address);

            request.Timeout = Timeout;

            return request;
        }

        public MemoryStream DownloadDataStream(string address)
        {
            var buffer = DownloadData(address);

            return new MemoryStream(buffer);
        }

        public MemoryStream DownloadDataStream(Uri address)
        {
            return DownloadDataStream(address.ToString());
        }
    }
}