using System;
using System.Collections.Generic;
using System.Net;
using System.Text.RegularExpressions;

namespace Beanfun
{
    internal static class HtmlInputParser
    {
        private static readonly Regex InputRegex = new Regex(
            @"<input\b[^>]*>",
            RegexOptions.IgnoreCase | RegexOptions.Singleline | RegexOptions.Compiled
        );

        private static readonly Regex AttributeRegex = new Regex(
            @"(?<name>[\w:-]+)\s*=\s*(?:""(?<value>[^""]*)""|'(?<value>[^']*)'|(?<value>[^\s>]+))",
            RegexOptions.IgnoreCase | RegexOptions.Singleline | RegexOptions.Compiled
        );

        public static bool TryGetInputValue(string html, string inputNameOrId, out string value)
        {
            value = null;
            if (string.IsNullOrEmpty(html) || string.IsNullOrEmpty(inputNameOrId))
                return false;

            foreach (Match inputMatch in InputRegex.Matches(html))
            {
                Dictionary<string, string> attributes = ParseAttributes(inputMatch.Value);
                if (!MatchesInput(attributes, inputNameOrId))
                    continue;

                attributes.TryGetValue("value", out string rawValue);
                value = WebUtility.HtmlDecode(rawValue ?? string.Empty);
                return true;
            }

            return false;
        }

        private static Dictionary<string, string> ParseAttributes(string inputTag)
        {
            Dictionary<string, string> attributes = new Dictionary<string, string>(
                StringComparer.OrdinalIgnoreCase
            );

            foreach (Match attributeMatch in AttributeRegex.Matches(inputTag))
            {
                attributes[attributeMatch.Groups["name"].Value] =
                    attributeMatch.Groups["value"].Value;
            }

            return attributes;
        }

        private static bool MatchesInput(
            IReadOnlyDictionary<string, string> attributes,
            string inputNameOrId
        )
        {
            return (
                    attributes.TryGetValue("id", out string id)
                    && string.Equals(id, inputNameOrId, StringComparison.OrdinalIgnoreCase)
                )
                || (
                    attributes.TryGetValue("name", out string name)
                    && string.Equals(name, inputNameOrId, StringComparison.OrdinalIgnoreCase)
                );
        }
    }
}
