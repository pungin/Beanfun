using System;
using System.Collections.Generic;
using System.Windows;

namespace Beanfun
{
    /// <summary>
    /// EquipCalculator.xaml 的交互逻辑
    /// </summary>
    public partial class EquipCalculator : Window
    {
        private class Scroll
        {
            public ScrollStat Weapon { get; set; }
            public ScrollStat Armor { get; set; }
            public ScrollStat Accessory { get; set; }
        }

        private class ScrollStat
        {
            public byte StatMin { get; set; }
            public byte StatMax { get; set; }
            public byte Stat { get { return RandomType == 0 ? StatMin : RandomType == 1 ? (byte)((StatMin + StatMax) / 2) : StatMax; } }

            public byte AtkMin { get; set; }
            public byte AtkMax { get; set; }
            public byte Atk { get { return RandomType == 0 ? AtkMin : RandomType == 1 ? (byte)((AtkMin + AtkMax) / 2) : AtkMax; } }

            public bool IsRandom { get { return StatMin == StatMax && AtkMin == AtkMax; } }

            public byte RandomType { get; set; }

            public ScrollStat(byte stat, byte atk)
            {
                StatMin = stat;
                StatMax = stat;
                AtkMin = atk;
                AtkMax = atk;
            }

            public ScrollStat(byte statMin, byte statMax, byte atkMin, byte atkMax)
            {
                StatMin = statMin;
                StatMax = statMax;
                AtkMin = atkMin;
                AtkMax = atkMax;
            }
        }

        static class Scrolls
        {
            public static Scroll Destiny = new Scroll();
            public static Scroll Glory = new Scroll();
            public static Scroll Black = new Scroll();
            public static Scroll V = new Scroll();
            public static Scroll X = new Scroll();
            public static Scroll Red = new Scroll();
            public static Scroll JD = new Scroll();
            public static Scroll SM = new Scroll();
            public static Scroll BM = new Scroll();

            static Scrolls()
            {
                Destiny.Weapon = new ScrollStat(14, 20, 14, 20);
                Destiny.Armor = new ScrollStat(0, 0, 9, 15);
                Destiny.Accessory = new ScrollStat(0, 0, 9, 15);
                Destiny.Weapon.RandomType = 1;
                Destiny.Accessory.RandomType = Destiny.Weapon.RandomType;
                Destiny.Armor.RandomType = Destiny.Weapon.RandomType;

                Glory.Weapon = new ScrollStat(10, 20, 10, 20);
                Glory.Armor = new ScrollStat(0, 0, 5, 15);
                Glory.Accessory = new ScrollStat(0, 0, 5, 15);
                Glory.Weapon.RandomType = 1;
                Glory.Accessory.RandomType = Glory.Weapon.RandomType;
                Glory.Armor.RandomType = Glory.Weapon.RandomType;

                Black.Weapon = new ScrollStat(14, 14);
                Black.Armor = new ScrollStat(2, 9);
                Black.Accessory = new ScrollStat(0, 9);

                V.Weapon = new ScrollStat(11, 13);
                V.Armor = new ScrollStat(0, 8);
                V.Accessory = new ScrollStat(0, 8);

                X.Weapon = new ScrollStat(10, 12);
                X.Armor = new ScrollStat(0, 7);
                X.Accessory = new ScrollStat(0, 7);

                Red.Weapon = new ScrollStat(8, 10);
                Red.Armor = new ScrollStat(0, 5);
                Red.Accessory = new ScrollStat(0, 5);

                JD.Weapon = new ScrollStat(5, 9);
                JD.Armor = new ScrollStat(0, 4);
                JD.Accessory = new ScrollStat(0, 4);

                SM.Weapon = new ScrollStat(5, 7);
                SM.Armor = new ScrollStat(5, 1);
                SM.Accessory = new ScrollStat(5, 1);

                BM.Weapon = new ScrollStat(4, 7);
                BM.Armor = new ScrollStat(5, 0);
                BM.Accessory = new ScrollStat(5, 0);
            }
        }

        bool InitFinish = false;
        public EquipCalculator()
        {
            InitializeComponent();
            InitFinish = true;
        }

        private void Window_MouseLeftButtonDown(object sender, System.Windows.Input.MouseButtonEventArgs e)
        {
            this.DragMove();
        }

        private void rb_EqpTyp_IsCheckedChanged(object sender, RoutedEventArgs e)
        {
            if (!InitFinish) return;
            cb_Superior.IsChecked = false;
            cb_Superior.Visibility = (bool)rb_Lv150.IsChecked && ((bool)rb_Glove.IsChecked || (bool)rb_Armor.IsChecked) ? Visibility.Visible : Visibility.Collapsed;
            lbl_HeartNotice.Visibility = (bool)rb_Heart.IsChecked ? Visibility.Visible : Visibility.Collapsed;
            calcStat();
        }

        private void rb_ReqLev_IsCheckedChanged(object sender, RoutedEventArgs e)
        {
            if (!InitFinish) return;
            if (!(bool)rb_Lv150.IsChecked || (!(bool)rb_Glove.IsChecked && !(bool)rb_Armor.IsChecked))
            {
                cb_Superior.IsChecked = false;
                cb_Superior.Visibility = Visibility.Collapsed;
            }
            else cb_Superior.Visibility = Visibility.Visible;
            calcStat();
        }

        private void rb_DestinyType_IsCheckedChanged(object sender, RoutedEventArgs e)
        {
            if (!InitFinish) return;
            Scrolls.Destiny.Weapon.RandomType = (byte)((bool)rb_DestinyMin.IsChecked ? 0 : (bool)rb_DestinyAverage.IsChecked ? 1 : 2);
            Scrolls.Destiny.Accessory.RandomType = Scrolls.Destiny.Weapon.RandomType;
            Scrolls.Destiny.Armor.RandomType = Scrolls.Destiny.Weapon.RandomType;
            calcStat();
        }

        private void rb_GloryType_IsCheckedChanged(object sender, RoutedEventArgs e)
        {
            if (!InitFinish) return;
            Scrolls.Glory.Weapon.RandomType = (byte)((bool)rb_GloryMin.IsChecked ? 0 : (bool)rb_GloryAverage.IsChecked ? 1 : 2);
            Scrolls.Glory.Accessory.RandomType = Scrolls.Glory.Weapon.RandomType;
            Scrolls.Glory.Armor.RandomType = Scrolls.Glory.Weapon.RandomType;
            calcStat();
        }

        private void cb_Superior_IsCheckedChanged(object sender, RoutedEventArgs e)
        {
            if (!InitFinish) return;
            lbl_StarForceMax.Content = (bool)cb_Superior.IsChecked ? "15" : "25";
            if ((bool)cb_Superior.IsChecked)
            {
                rb_Lv160.Visibility = Visibility.Collapsed;
                rb_Lv200.Visibility = Visibility.Collapsed;
                if (!(bool)rb_Lv150.IsChecked)
                {
                    rb_Lv150.IsChecked = true;
                    return;
                }
            }
            else
            {
                rb_Lv160.Visibility = Visibility.Visible;
                rb_Lv200.Visibility = Visibility.Visible;
            }
            calcStat();
        }

        private void calcStat_TextChanged(object sender, System.Windows.Controls.TextChangedEventArgs e)
        {
            calcStat();
        }

        private void t_BaseStat_GotFocus(object sender, RoutedEventArgs e)
        {
            t_BaseStat.Text = "";
        }

        private void t_FlameStat_GotFocus(object sender, RoutedEventArgs e)
        {
            t_FlameStat.Text = "";
        }

        private void t_BaseATK_GotFocus(object sender, RoutedEventArgs e)
        {
            t_BaseATK.Text = "";
        }

        private void t_FlameATK_GotFocus(object sender, RoutedEventArgs e)
        {
            t_FlameATK.Text = "";
        }

        private void t_StarForce_GotFocus(object sender, RoutedEventArgs e)
        {
            t_StarForce.Text = "";
        }

        private void t_DestinyNum_GotFocus(object sender, RoutedEventArgs e)
        {
            t_DestinyNum.Text = "";
        }

        private void t_GloryNum_GotFocus(object sender, RoutedEventArgs e)
        {
            t_GloryNum.Text = "";
        }

        private void t_BlackNum_GotFocus(object sender, RoutedEventArgs e)
        {
            t_BlackNum.Text = "";
        }

        private void t_VNum_GotFocus(object sender, RoutedEventArgs e)
        {
            t_VNum.Text = "";
        }

        private void t_XNum_GotFocus(object sender, RoutedEventArgs e)
        {
            t_XNum.Text = "";
        }

        private void t_RedNum_GotFocus(object sender, RoutedEventArgs e)
        {
            t_RedNum.Text = "";
        }

        private void t_JDNum_GotFocus(object sender, RoutedEventArgs e)
        {
            t_JDNum.Text = "";
        }

        private void t_SMNum_GotFocus(object sender, RoutedEventArgs e)
        {
            t_SMNum.Text = "";
        }

        private void t_BMNum_GotFocus(object sender, RoutedEventArgs e)
        {
            t_BMNum.Text = "";
        }

        private void t_ScrollStat_GotFocus(object sender, RoutedEventArgs e)
        {
            t_ScrollStat.Text = "";
        }

        private void t_ScrollATK_GotFocus(object sender, RoutedEventArgs e)
        {
            t_ScrollATK.Text = "";
        }

        private void calcStat()
        {
            if (!InitFinish) return;
            byte eqpTyp = (byte)((bool)rb_Weapon.IsChecked ? 0 : (bool)rb_Glove.IsChecked ? 1 : (bool)rb_Armor.IsChecked ? 2 : (bool)rb_Accessory.IsChecked ? 3 : 4);
            short reqLev = (short)((bool)rb_Lv200.IsChecked ? 200 : (bool)rb_Lv160.IsChecked ? 160 : 150);
            bool superior = (bool)cb_Superior.IsChecked && cb_Superior.Visibility == Visibility.Visible;

            int baseStat;
            try
            {
                baseStat = int.Parse(t_BaseStat.Text);
            }
            catch
            {
                baseStat = 0;
            }

            byte flameStat;
            try
            {
                flameStat = byte.Parse(t_FlameStat.Text);
            }
            catch
            {
                flameStat = 0;
            }

            int baseATK;
            try
            {
                baseATK = int.Parse(t_BaseATK.Text);
            }
            catch
            {
                baseATK = 0;
            }

            byte flameATK;
            try
            {
                flameATK = byte.Parse(t_FlameATK.Text);
            }
            catch
            {
                flameATK = 0;
            }

            byte starForce;
            try
            {
                starForce = byte.Parse(t_StarForce.Text);
            }
            catch
            {
                starForce = 0;
            }

            byte destinyNum;
            try
            {
                destinyNum = byte.Parse(t_DestinyNum.Text);
            }
            catch
            {
                destinyNum = 0;
            }

            byte gloryNum;
            try
            {
                gloryNum = byte.Parse(t_GloryNum.Text);
            }
            catch
            {
                gloryNum = 0;
            }

            byte blackNum;
            try
            {
                blackNum = byte.Parse(t_BlackNum.Text);
            }
            catch
            {
                blackNum = 0;
            }

            byte vNum;
            try
            {
                vNum = byte.Parse(t_VNum.Text);
            }
            catch
            {
                vNum = 0;
            }

            byte xNum;
            try
            {
                xNum = byte.Parse(t_XNum.Text);
            }
            catch
            {
                xNum = 0;
            }

            byte redNum;
            try
            {
                redNum = byte.Parse(t_RedNum.Text);
            }
            catch
            {
                redNum = 0;
            }

            byte jdNum;
            try
            {
                jdNum = byte.Parse(t_JDNum.Text);
            }
            catch
            {
                jdNum = 0;
            }

            byte smNum;
            try
            {
                smNum = byte.Parse(t_SMNum.Text);
            }
            catch
            {
                smNum = 0;
            }

            byte bmNum;
            try
            {
                bmNum = byte.Parse(t_BMNum.Text);
            }
            catch
            {
                bmNum = 0;
            }

            int scrollStat;
            try
            {
                scrollStat = int.Parse(t_ScrollStat.Text);
            }
            catch
            {
                scrollStat = 0;
            }

            int scrollTK;
            try
            {
                scrollTK = int.Parse(t_ScrollATK.Text);
            }
            catch
            {
                scrollTK = 0;
            }
            
            int atk = baseATK
            + destinyNum * (eqpTyp == 0 || eqpTyp == 4 ? Scrolls.Destiny.Weapon.Atk : (eqpTyp == 3 ? Scrolls.Destiny.Accessory.Atk : Scrolls.Destiny.Armor.Atk))
            + gloryNum * (eqpTyp == 0 || eqpTyp == 4 ? Scrolls.Glory.Weapon.Atk : (eqpTyp == 3 ? Scrolls.Glory.Accessory.Atk : Scrolls.Glory.Armor.Atk))
            + blackNum * (eqpTyp == 0 || eqpTyp == 4 ? Scrolls.Black.Weapon.Atk : (eqpTyp == 3 ? Scrolls.Black.Accessory.Atk : Scrolls.Black.Armor.Atk))
            + vNum * (eqpTyp == 0 || eqpTyp == 4 ? Scrolls.V.Weapon.Atk : (eqpTyp == 3 ? Scrolls.V.Accessory.Atk : Scrolls.V.Armor.Atk))
            + xNum * (eqpTyp == 0 || eqpTyp == 4 ? Scrolls.X.Weapon.Atk : (eqpTyp == 3 ? Scrolls.X.Accessory.Atk : Scrolls.X.Armor.Atk))
            + redNum * (eqpTyp == 0 || eqpTyp == 4 ? Scrolls.Red.Weapon.Atk : (eqpTyp == 3 ? Scrolls.Red.Accessory.Atk : Scrolls.Red.Armor.Atk))
            + jdNum * (eqpTyp == 0 || eqpTyp == 4 ? Scrolls.JD.Weapon.Atk : (eqpTyp == 3 ? Scrolls.JD.Accessory.Atk : Scrolls.JD.Armor.Atk))
            + smNum * (eqpTyp == 0 || eqpTyp == 4 ? Scrolls.SM.Weapon.Atk : (eqpTyp == 3 ? Scrolls.SM.Accessory.Atk : Scrolls.SM.Armor.Atk))
            + bmNum * (eqpTyp == 0 || eqpTyp == 4 ? Scrolls.BM.Weapon.Atk : (eqpTyp == 3 ? Scrolls.BM.Accessory.Atk : Scrolls.BM.Armor.Atk))
            + scrollTK
            ;

            int stat = baseStat
            + destinyNum * (eqpTyp == 0 || eqpTyp == 4 ? Scrolls.Destiny.Weapon.Stat : (eqpTyp == 3 ? Scrolls.Destiny.Accessory.Stat : Scrolls.Destiny.Armor.Stat))
            + gloryNum * (eqpTyp == 0 || eqpTyp == 4 ? Scrolls.Glory.Weapon.Stat : (eqpTyp == 3 ? Scrolls.Glory.Accessory.Stat : Scrolls.Glory.Armor.Stat))
            + blackNum * (eqpTyp == 0 || eqpTyp == 4 ? Scrolls.Black.Weapon.Stat : (eqpTyp == 3 ? Scrolls.Black.Accessory.Stat : Scrolls.Black.Armor.Stat))
            + vNum * (eqpTyp == 0 || eqpTyp == 4 ? Scrolls.V.Weapon.Stat : (eqpTyp == 3 ? Scrolls.V.Accessory.Stat : Scrolls.V.Armor.Stat))
            + xNum * (eqpTyp == 0 || eqpTyp == 4 ? Scrolls.X.Weapon.Stat : (eqpTyp == 3 ? Scrolls.X.Accessory.Stat : Scrolls.X.Armor.Stat))
            + redNum * (eqpTyp == 0 || eqpTyp == 4 ? Scrolls.Red.Weapon.Stat : (eqpTyp == 3 ? Scrolls.Red.Accessory.Stat : Scrolls.Red.Armor.Stat))
            + jdNum * (eqpTyp == 0 || eqpTyp == 4 ? Scrolls.JD.Weapon.Stat : (eqpTyp == 3 ? Scrolls.JD.Accessory.Stat : Scrolls.JD.Armor.Stat))
            + smNum * (eqpTyp == 0 || eqpTyp == 4 ? Scrolls.SM.Weapon.Stat : (eqpTyp == 3 ? Scrolls.SM.Accessory.Stat : Scrolls.SM.Armor.Stat))
            + bmNum * (eqpTyp == 0 || eqpTyp == 4 ? Scrolls.BM.Weapon.Stat : (eqpTyp == 3 ? Scrolls.BM.Accessory.Stat : Scrolls.BM.Armor.Stat))
            + scrollStat
            ;

            Dictionary<int, int> echantStats;
            for (byte i = 0; i < starForce; i++)
            {
                echantStats = getStarForceStats(superior, eqpTyp, i, atk, reqLev);
                stat += echantStats[1];
                atk += echantStats[2];
            }

            lbl_AddedStat.Content = stat - baseStat;
            lbl_TotalStat.Content = stat + flameStat;
            lbl_AddedATK.Content = atk - baseATK;
            lbl_TotalATK.Content = atk + flameATK;
        }

        private Dictionary<int, int> getStarForceStats(bool superior, byte eqpTyp, byte starForce, int atk, short reqLev)
        {
            Dictionary<int, int> stats = new Dictionary<int, int>();
            stats.Add(1, 0);
            stats.Add(2, 0);
            if (superior)
            {
                // 尊貴裝
                switch (starForce)
                {
                    case 0:
                        stats.Remove(1);
                        stats.Add(1, 19);
                        break;
                    case 1:
                        stats.Remove(1);
                        stats.Add(1, 20);
                        break;
                    case 2:
                        stats.Remove(1);
                        stats.Add(1, 22);
                        break;
                    case 3:
                        stats.Remove(1);
                        stats.Add(1, 25);
                        break;
                    case 4:
                        stats.Remove(1);
                        stats.Add(1, 29);
                        break;
                    case 5:
                    case 6:
                    case 7:
                    case 8:
                    case 9:
                        stats.Remove(2);
                        stats.Add(2, starForce + 4);
                        break;
                    case 10:
                    case 11:
                    case 12:
                    case 13:
                    case 14:
                        stats.Remove(2);
                        stats.Add(2, 15 + 2 * (starForce - 10));
                        break;
                }
            }
            else if (eqpTyp == 0)
            {
                // 武器

                // 屬性
                int allStats;
                if (starForce >= 0 && starForce < 5)
                {
                    allStats = 2;
                }
                else if (starForce >= 5 && starForce < 15)
                {
                    allStats = 3;
                }
                else if(starForce < 22)
                {
                    if (reqLev >= 200)
                        allStats = 15;
                    else if (reqLev >= 160)
                        allStats = 13;
                    else
                        allStats = 11;
                }
                else
                {
                    allStats = 0;
                }
                stats.Remove(1);
                stats.Add(1, allStats);

                // 攻擊力
                if (starForce < 15)
                {
                    stats.Remove(2);
                    stats.Add(2, (int)Math.Floor(atk / 50.0D) + 1);
                }
                else
                {
                    int value = 0;
                    switch (starForce)
                    {
                        case 15:
                            if (reqLev >= 200)
                                value = 13;
                            else if (reqLev >= 160)
                                value = 9;
                            else
                                value = 8;
                            break;
                        case 16:
                            if (reqLev >= 200)
                                value = 13;
                            else
                                value = 9;
                            break;
                        case 17:
                            if (reqLev >= 200)
                                value = 14;
                            else if (reqLev >= 160)
                                value = 10;
                            else
                                value = 9;
                            break;
                        case 18:
                            if (reqLev >= 200)
                                value = 14;
                            else if (reqLev >= 160)
                                value = 11;
                            else
                                value = 10;
                            break;
                        case 19:
                            if (reqLev >= 200)
                                value = 15;
                            else if (reqLev >= 160)
                                value = 12;
                            else
                                value = 11;
                            break;
                        case 20:
                            if (reqLev >= 200)
                                value = 16;
                            else if (reqLev >= 160)
                                value = 13;
                            else
                                value = 12;
                            break;
                        case 21:
                            if (reqLev >= 200)
                                value = 17;
                            else if (reqLev >= 160)
                                value = 14;
                            else
                                value = 13;
                            break;
                        case 22:
                            if (reqLev >= 200)
                                value = 34;
                            else if (reqLev >= 160)
                                value = 32;
                            else
                                value = 31;
                            break;
                        case 23:
                            if (reqLev >= 200)
                                value = 35;
                            break;
                    }
                    stats.Remove(2);
                    stats.Add(2, value);
                }
            }
            else
            {
                // 其他裝備
                int allStats;
                if (starForce >= 0 && starForce < 5)
                {
                    allStats = 2;
                }
                else if (starForce >= 5 && starForce < 15)
                {
                    allStats = 3;
                }
                else if (starForce < 22)
                {
                    if (reqLev >= 200)
                        allStats = 15;
                    else if (reqLev >= 160)
                        allStats = 13;
                    else
                        allStats = 11;
                }
                else
                {
                    allStats = 0;
                }
                stats.Remove(1);
                stats.Add(1, allStats);

                if (starForce >= 15)
                {
                    int value = 0;
                    switch (starForce)
                    {
                        case 15:
                            if (reqLev >= 200)
                                value = 12;
                            else if (reqLev >= 160)
                                value = 10;
                            else
                                value = 9;
                            break;
                        case 16:
                            if (reqLev >= 200)
                                value = 13;
                            else if (reqLev >= 160)
                                value = 11;
                            else
                                value = 10;
                            break;
                        case 17:
                            if (reqLev >= 200)
                                value = 14;
                            else if (reqLev >= 160)
                                value = 12;
                            else
                                value = 11;
                            break;
                        case 18:
                            if (reqLev >= 200)
                                value = 15;
                            else if (reqLev >= 160)
                                value = 13;
                            else
                                value = 12;
                            break;
                        case 19:
                            if (reqLev >= 200)
                                value = 16;
                            else if (reqLev >= 160)
                                value = 14;
                            else
                                value = 13;
                            break;
                        case 20:
                            if (reqLev >= 200)
                                value = 17;
                            else if (reqLev >= 160)
                                value = 15;
                            else
                                value = 14;
                            break;
                        case 21:
                            if (reqLev >= 200)
                                value = 19;
                            else if (reqLev >= 160)
                                value = 17;
                            else
                                value = 16;
                            break;
                        case 22:
                            if (reqLev >= 200)
                                value = 21;
                            else if (reqLev >= 160)
                                value = 19;
                            else
                                value = 18;
                            break;
                        case 23:
                            if (reqLev >= 200)
                                value = 23;
                            else if (reqLev >= 160)
                                value = 21;
                            else
                                value = 20;
                            break;
                        case 24:
                            if (reqLev >= 200)
                                value = 25;
                            else if (reqLev >= 160)
                                value = 23;
                            else
                                value = 22;
                            break;
                    }
                    stats.Remove(2);
                    stats.Add(2, value);
                }
                else if (eqpTyp == 1)
                {
                    int value = 0;
                    switch (starForce)
                    {
                        case 4:
                        case 6:
                        case 8:
                        case 10:
                        case 12:
                            value = 1;
                            break;
                        case 13:
                            if (reqLev >= 200)
                                value = 1;
                            break;
                        case 14:
                            if (reqLev >= 200)
                                value = 1;
                            else
                                value = 2;
                            break;
                    }
                    stats.Remove(2);
                    stats.Add(2, value);
                }
            }
            return stats;
        }
    }
}
