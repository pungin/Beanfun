using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using System.Resources;
using System.Collections;
using System.Xml;
using System.IO;

namespace ResourcesManager
{
    internal class Program
    {
        static void Main(string[] args)
        {
            Console.WriteLine(@"start auto edit resource md5");
            if(args.Length == 0)
            {
                Console.WriteLine(@"usage: ResourcesManager.exe <resources file path>");
                return;
            }
            string file = args[0];
            if (!File.Exists(file))
            {
                Console.WriteLine($@"file {file} not found!");
                return;
            }
            string dir = Path.GetDirectoryName(file);
            Directory.SetCurrentDirectory(dir);
            XmlDocument document = new XmlDocument();
            document.Load(file);
            XmlNodeList nodeList = document.SelectNodes(@"/root/data");
            Dictionary<string, string> map = new Dictionary<string, string>();

            foreach(XmlNode node1 in nodeList)
            {
                string type = node1.Attributes["type"]?.Value ?? string.Empty;
                if (type.StartsWith("System.Resources.ResXFileRef"))
                {
                    string name = node1.Attributes["name"]?.Value ?? string.Empty;
                    if (name != string.Empty)
                    {
                        XmlNode node2 = node1.SelectSingleNode("value");
                        if (node2 != null)
                        {
                            string value = node2.InnerText ?? string.Empty;
                            if (value.Contains(";"))
                            {
                                string path = value.Split(';')[0];
                                if (path != string.Empty && File.Exists(path))
                                {
                                    map[name + "_md5"] = path;
                                }
                            }
                        }
                    }
                }
            }
            bool onChange = false;
            foreach (XmlNode node1 in nodeList)
            {
                string space = node1.Attributes["xml:space"]?.Value ?? string.Empty;
                string name = node1.Attributes["name"]?.Value ?? string.Empty;
                if (space == "preserve" && name != string.Empty && map.ContainsKey(name))
                {
                    XmlNode node2 = node1.SelectSingleNode("value");
                    if (node2 != null)
                    {
                        string value = node2.InnerText ?? string.Empty;
                        string md5 = GetMD5HashFromFile(map[name]);
                        if (md5 != string.Empty && md5 != value)
                        {
                            Console.WriteLine($@"auto edit resource md5,file:{map[name]}, old:{value}, new:{md5}");
                            node2.InnerText = md5;
                            onChange = true;
                        }
                    }
                }
            }
            if (onChange)
            {
                document.Save(file);
            }
            else
            {
                Console.WriteLine($@"all md5 is correct");
            }
        }

        private static string GetMD5HashFromFile(string fileName)
        {
            try
            {
                FileStream file = new FileStream(fileName, FileMode.Open, FileAccess.Read, FileShare.Read);
                System.Security.Cryptography.MD5 md5 = new System.Security.Cryptography.MD5CryptoServiceProvider();
                byte[] retVal = md5.ComputeHash(file);
                file.Close();
                StringBuilder sb = new StringBuilder();
                for (int i = 0; i < retVal.Length; i++)
                {
                    sb.Append(retVal[i].ToString("x2"));
                }
                return sb.ToString().ToUpper();
            }
            catch (Exception ex)
            {
                throw new Exception("GetMD5HashFromFile() fail,error:" + ex.Message);
            }
        }
    }
}
