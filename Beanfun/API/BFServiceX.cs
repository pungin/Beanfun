using BFService;

namespace Beanfun
{
    public class BFServiceX
    {
        private BFServiceXClass objAdapter;

        public BFServiceX()
        {
            objAdapter = new BFServiceXClass();
        }

        public uint Initialize2()
        {
            try
            {
                uint initRes = objAdapter.Initialize2("HK;Production", "", "", 0, "");
                if (initRes != 0) return Initialize2();
                return initRes;
            }
            catch { return Initialize2(); }
        }

        private string _token;
        public string Token
        {
            get {
                try
                {
                    objAdapter.SaveData("Seed", "0");
                    objAdapter.SaveData("Token", _token);
                    string token = objAdapter.LoadData("Token");
                    if (token == "Failure")
                    {
                        Initialize2();
                        return Token;
                    }
                    //System.Diagnostics.Debug.WriteLine(token);
                    return token;
                } catch {
                    return Token;
                }
            }
            set { _token = value; }
        }
    }
}