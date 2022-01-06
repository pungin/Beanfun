using System.Windows;
using System.Windows.Controls;
using System.Windows.Documents;
using System.Windows.Media;
using System.Xml;

namespace Beanfun
{
    class TextBlockHelper
    {
        #region FormattedText Attached dependency property

        public static string GetFormattedText(DependencyObject obj)
        {
            return (string)obj.GetValue(FormattedTextProperty);
        }

        public static void SetFormattedText(DependencyObject obj, string value)
        {
            obj.SetValue(FormattedTextProperty, value);
        }

        public static readonly DependencyProperty FormattedTextProperty =
            DependencyProperty.RegisterAttached("FormattedText",
            typeof(string),
            typeof(TextBlockHelper),
            new UIPropertyMetadata("", FormattedTextChanged));

        private static void FormattedTextChanged(DependencyObject sender, DependencyPropertyChangedEventArgs e)
        {
            string value = e.NewValue as string;

            TextBlock textBlock = sender as TextBlock;

            if (textBlock != null)
            {
                textBlock.Inlines.Clear();
                textBlock.Inlines.Add(Process(value));
            }
        }

        #endregion

        static Inline Process(string value)
        {
            XmlDocument doc = new XmlDocument();
            doc.LoadXml(value);

            Span span = new Span();
            InternalProcess(span, doc.FirstChild);

            return span;
        }

        private static void InternalProcess(Span span, XmlNode xmlNode)
        {
            foreach (XmlNode child in xmlNode)
            {
                Span spanItem = new Span();
                if (child is XmlElement) InternalProcess(spanItem, child);
                switch (child.Name.ToUpper())
                {
                    case "B":
                    case "BOLD":
                        Bold bold = new Bold(spanItem);
                        span.Inlines.Add(bold);
                        break;
                    case "I":
                    case "ITALIC":
                        Italic italic = new Italic(spanItem);
                        span.Inlines.Add(italic);
                        break;
                    case "U":
                    case "UNDERLINE":
                        Underline underline = new Underline(spanItem);
                        span.Inlines.Add(underline);
                        break;
                    case "L":
                    case "LINEBREAK":
                        span.Inlines.Add(new LineBreak());
                        break;
                    case "R":
                    case "RUN":
                        Run run = new Run(child.InnerText);
                        if (child.Attributes != null)
                            foreach (XmlNode att in child.Attributes)
                            {
                                switch (att.Name.ToUpper())
                                {
                                    case "FOREGROUND":
                                        run.Foreground = new SolidColorBrush((Color)ColorConverter.ConvertFromString(att.Value));
                                        break;
                                    case "BACKGROUND":
                                        run.Background = new SolidColorBrush((Color)ColorConverter.ConvertFromString(att.Value));
                                        break;
                                }
                            }
                        span.Inlines.Add(run);
                        break;
                    default:
                        if (child is XmlText) span.Inlines.Add(new Run(child.InnerText));
                        break;
                }
            }
        }
    }
}
