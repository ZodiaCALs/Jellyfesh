using System.Globalization;
using System.Xml.Linq;
using Jellyfesh.Formatting;

namespace Jellyfesh.Tests.Support;

/// <summary>The app's own English strings (Strings/en-US/Resources.resw), for tests that check the words.</summary>
internal sealed class English : ILocalizer
{
    private readonly Dictionary<string, string> _table;

    private English()
    {
        var path = Contract.PathOf("platforms", "windows", "src", "Jellyfesh.App", "Strings", "en-US", "Resources.resw");
        _table = XDocument.Load(path).Root!.Elements("data")
            .ToDictionary(d => (string)d.Attribute("name")!, d => (string?)d.Element("value") ?? string.Empty, StringComparer.Ordinal);
    }

    public static English Instance { get; } = new();

    public CultureInfo Culture => CultureInfo.GetCultureInfo("en-US");

    public string Get(string key) => _table.TryGetValue(key, out var value) ? value : key;
}
