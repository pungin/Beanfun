using System;
using System.Collections.Generic;
using System.Text;

namespace Beanfun
{
    /// <summary>舊 get_webstart_otp.ashx 需要的參數，由 blob 帶出來。</summary>
    public class LegacyOtpParams
    {
        public string ppppp;
        public string ServiceCode;
        public string ServiceRegion;
        public string ServiceAccount;
        public string CreateTime;
    }

    public class LaunchPayload
    {
        /// <summary>非 null 表示走 get_webstart_otp_v2.ashx。</summary>
        public string LaunchTicket;

        /// <summary>非 null 表示走舊的 get_webstart_otp.ashx。</summary>
        public LegacyOtpParams Legacy;
    }

    /// <summary>
    /// game_start_step2.aspx 上 m_objData.data 這塊混淆資料的解碼器。
    ///
    /// 頁面會嵌入
    /// <code>
    /// var m_objData = { "region": "TW;Production", "sn": "...", "data": "..." };
    /// </code>
    /// 並用 gamaniagames:// 交給橘子的原生啟動器（GGM）。data 裡就是該遊戲取
    /// 密碼所需的參數，只是被混淆過，所以沒裝 GGM 也還原得出來。
    ///
    /// 兩種 payload 都還活著：
    ///   - LaunchTicket=...  → 給 get_webstart_otp_v2.ashx（楓之谷）
    ///   - ppppp=...         → 給舊的 get_webstart_otp.ashx（新楓、CSO、艾爾之光、瑪奇…）
    /// 兩種頁面從外面看完全一樣，都只宣告 m_objData，所以一定要先解出來才知道
    /// 該走哪個端點。只認 LaunchTicket 會讓其他遊戲全部誤判成解密失敗（#376）。
    ///
    /// 格式：
    ///   1. 第一個字元是十六進位數字 n，用來選替換表。
    ///   2. 其餘每個字元換成它在替換表中的索引，再以十六進位字元輸出（normalized hex）。
    ///   3. normalized hex 位移 n+1 起的 8 個字元就是 DES key（ASCII）。
    ///   4. 拿掉那 8 個字元，剩下的就是密文 hex。
    ///   5. DES-ECB、無 padding，解完去掉尾端的 \0。
    ///   6. 明文是 key=value，以 &amp; 串接，遇到 ; 之後都是尾綴。
    ///
    /// 3~5 跟舊 OTP 信封（8 字元 ASCII key + hex 密文）是同一套，所以直接沿用
    /// <see cref="WCDESComp.DecryStrHex"/>。
    ///
    /// 解碼演算法由 @takidog 發表於 pungin/Beanfun#368。
    /// </summary>
    public static class LaunchData
    {
        /// <summary>
        /// 啟動器 Command.DecryptParam() 裡硬編碼的替換表。每一張都是 16 個
        /// 十六進位字元的排列，第 2 步才可逆。
        /// </summary>
        private static readonly string[] Tables =
        {
            "bac987d65e432f10",
            "3bc4d5e6f2a79108",
            "cdbeaf9012456378",
            "4e6fb81a3c5d7092",
            "bdef1246789ac530",
            "5f82cb4093e71d6a",
            "df1468ace0357b92",
            "b50c61a4f93e82d7",
        };

        private const string HexDigits = "0123456789abcdef";
        private const int KeyLen = 8;

        /// <summary>
        /// 解出 blob 帶的是哪一種 payload；兩種都不是就回 null。
        /// </summary>
        public static LaunchPayload Decode(string data)
        {
            string plaintext = DecodeRaw(data);
            if (plaintext == null)
                return null;

            var fields = ParseFields(plaintext);

            string ticket;
            if (fields.TryGetValue("LaunchTicket", out ticket) && ticket.Length > 0)
            {
                // 有沒有決定路線，長度不決定。釘死在目前看到的 64 字元，就是
                // 讓 ppppp 那批遊戲全掛的那種過窄判斷。
                return new LaunchPayload { LaunchTicket = ticket };
            }

            string ppppp,
                serviceCode,
                serviceRegion,
                serviceAccount,
                createTime;
            if (
                fields.TryGetValue("ppppp", out ppppp)
                && fields.TryGetValue("ServiceCode", out serviceCode)
                && fields.TryGetValue("ServiceRegion", out serviceRegion)
                && fields.TryGetValue("ServiceAccount", out serviceAccount)
                && fields.TryGetValue("CreateTime", out createTime)
            )
            {
                return new LaunchPayload
                {
                    Legacy = new LegacyOtpParams
                    {
                        ppppp = ppppp,
                        ServiceCode = serviceCode,
                        ServiceRegion = serviceRegion,
                        ServiceAccount = serviceAccount,
                        CreateTime = createTime,
                    },
                };
            }

            return null;
        }

        /// <summary>
        /// 把明文切成 key=value。分隔符實際上是 &amp;&amp;&amp;&amp;，但用單一
        /// &amp; 切再丟掉空段，兩種形式都吃得下。第一個 ; 之後是尾綴不是欄位。
        /// </summary>
        private static Dictionary<string, string> ParseFields(string plaintext)
        {
            var fields = new Dictionary<string, string>(StringComparer.Ordinal);
            string body = plaintext.Split(';')[0];
            foreach (string segment in body.Split('&'))
            {
                if (segment.Length == 0)
                    continue;
                int eq = segment.IndexOf('=');
                if (eq <= 0)
                    continue;
                string key = segment.Substring(0, eq);
                if (!fields.ContainsKey(key))
                    fields[key] = segment.Substring(eq + 1);
            }
            return fields;
        }

        /// <summary>
        /// 還原替換層並解密，回傳去掉尾端 \0 的明文。
        ///
        /// 選字元對應哪張表其實沒定案：n % 4 對目前看過的樣本都解得開，但表有
        /// 八張，單一樣本分不出是 n % 4 還是表的排序跟啟動器不同。與其押一個
        /// 規則然後對某些帳號猜錯，不如每張都試到解出帶 LaunchTicket= 或
        /// ppppp= 的明文為止 — 錯的表只會給出雜訊，雜訊不會剛好拼出欄位名。
        /// </summary>
        private static string DecodeRaw(string data)
        {
            if (string.IsNullOrEmpty(data))
                return null;

            int selector = HexDigits.IndexOf(char.ToLowerInvariant(data[0]));
            if (selector < 0)
                return null;

            string body = data.Substring(1);

            var order = new List<int> { selector % 4, selector % Tables.Length };
            for (int i = 0; i < Tables.Length; i++)
                order.Add(i);

            var tried = new HashSet<int>();
            foreach (int tableIndex in order)
            {
                if (!tried.Add(tableIndex))
                    continue;
                string plaintext = DecodeWith(body, selector, tableIndex);
                if (
                    plaintext != null
                    && (
                        plaintext.Contains("LaunchTicket=")
                        || plaintext.Contains("ppppp=")
                    )
                )
                {
                    return plaintext;
                }
            }
            return null;
        }

        /// <summary>已經選好替換表的單次嘗試。</summary>
        private static string DecodeWith(string body, int selector, int tableIndex)
        {
            string table = Tables[tableIndex];
            var normalized = new StringBuilder(body.Length);
            foreach (char c in body)
            {
                int idx = table.IndexOf(c);
                if (idx < 0)
                    return null;
                normalized.Append(HexDigits[idx]);
            }

            int offset = selector + 1;
            if (normalized.Length < offset + KeyLen)
                return null;

            string hex = normalized.ToString();
            string key = hex.Substring(offset, KeyLen);
            string cipherHex = hex.Substring(0, offset) + hex.Substring(offset + KeyLen);
            // DES 區塊是 8 bytes = 16 個十六進位字元；不整除就是這張表不對，
            // 先擋掉免得 DecryStrHex 為了錯的表噴一次例外。
            if (cipherHex.Length == 0 || cipherHex.Length % 16 != 0)
                return null;

            string plaintext = WCDESComp.DecryStrHex(cipherHex, key);
            return plaintext == null ? null : plaintext.Trim('\0');
        }
    }
}
