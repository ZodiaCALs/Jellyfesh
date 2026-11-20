using System.Globalization;
using System.Text.RegularExpressions;
using System.Xml.Linq;
using Jellyfesh.Tests.Support;
using Xunit;

namespace Jellyfesh.Tests;

/// <summary>
/// The app's .resw files (English, Simplified Chinese, Traditional Chinese, Japanese) hold the same
/// keys, every key the code reads exists, and every message formats with the same placeholders in
/// every language.
/// </summary>
public partial class LocalizationTests
{
    private static readonly string[] Languages = ["en-US", "zh-Hans", "zh-Hant", "ja"];

    /// <summary>Keys the code builds at run time (plurals, enum names).</summary>
    private static readonly string[] Dynamic =
    [
        "Count_Songs_One", "Count_Songs_Other", "Count_Albums_One", "Count_Albums_Other", "Count_Artists_One", "Count_Artists_Other",
        "Count_Minutes_One", "Count_Minutes_Other", "Count_Hours_One", "Count_Hours_Other",
        "Kind_Track", "Kind_Album", "Kind_Artist", "Kind_Playlist", "Kind_Folder",
        "Sort_Title", "Sort_Artist", "Sort_Album", "Sort_Year", "Sort_Duration", "Sort_Path", "Sort_Added",
        "Quality_HiResLossless", "Quality_Lossless", "Quality_High", "Quality_Standard",
        "Fact_Version", "Fact_Genre", "Fact_Language", "Fact_Released", "Fact_Label", "Fact_Composer", "Fact_Lyricist",
        "Fact_Arranger", "Fact_Producer", "Fact_Popularity", "Fact_Quality", "Fact_Isrc", "Fact_Disc", "Fact_Track",
        "Shelf_for_you", "Shelf_recently_added", "Shelf_most_played", "Shelf_rediscover",
    ];

    [Fact]
    public void Every_language_has_the_same_keys()
    {
        var english = Read("en-US");
        Assert.True(english.Count > 250, "the English table is complete");
        foreach (var lang in Languages.Skip(1))
        {
            var other = Read(lang);
            Assert.Equal(english.Keys.Order(StringComparer.Ordinal), other.Keys.Order(StringComparer.Ordinal));
            Assert.All(other, kv => Assert.False(string.IsNullOrWhiteSpace(kv.Value), $"{lang} {kv.Key} is empty"));
        }
    }

    [Fact]
    public void Every_key_the_code_reads_exists()
    {
        var english = Read("en-US");
        var src = Contract.PathOf("platforms", "windows", "src");
        var used = Directory.GetFiles(src, "*.cs", SearchOption.AllDirectories)
            .Where(f => !f.Contains($"{Path.DirectorySeparatorChar}obj{Path.DirectorySeparatorChar}", StringComparison.Ordinal))
            .SelectMany(f => KeyLiteral().Matches(NativeEntryPoint().Replace(File.ReadAllText(f), string.Empty)).Select(m => m.Groups[1].Value))
            .Where(k => k is not ("Count_Songs" or "Count_Albums" or "Count_Artists" or "Count_Minutes" or "Count_Hours")) // read through Plural
            .Where(k => k != k.ToUpperInvariant()) // environment variables (JF_LOG_DIR), not strings
            .Concat(Dynamic)
            .ToHashSet(StringComparer.Ordinal);
        var missing = used.Where(k => !english.ContainsKey(k)).Order(StringComparer.Ordinal).ToList();
        Assert.Empty(missing);
    }

    [Fact]
    public void Messages_format_with_the_same_placeholders_in_every_language()
    {
        var tables = Languages.ToDictionary(l => l, Read);
        foreach (var (key, english) in tables["en-US"])
        {
            var holes = Holes(english);
            var args = Enumerable.Range(0, holes.Count == 0 ? 0 : holes.Max() + 1).Select(i => (object)(i + 1)).ToArray();
            foreach (var lang in Languages)
            {
                var text = tables[lang][key];
                Assert.True(holes.SetEquals(Holes(text)), $"{lang} {key}: placeholders differ from English");
                if (holes.Count > 0)
                {
                    _ = string.Format(CultureInfo.InvariantCulture, text, args); // throws on a broken format
                }
            }
        }
    }

    private static HashSet<int> Holes(string text) =>
        Placeholder().Matches(text).Select(m => int.Parse(m.Groups[1].Value, CultureInfo.InvariantCulture)).ToHashSet();

    private static Dictionary<string, string> Read(string lang)
    {
        var path = Contract.PathOf("platforms", "windows", "src", "Jellyfesh.App", "Strings", lang, "Resources.resw");
        return XDocument.Load(path).Root!.Elements("data")
            .ToDictionary(d => (string)d.Attribute("name")!, d => (string?)d.Element("value") ?? string.Empty, StringComparer.Ordinal);
    }

    [GeneratedRegex("\"([A-Z][A-Za-z]+_[A-Za-z0-9_]+)\"")]
    private static partial Regex KeyLiteral();

    /// <summary>Native functions' names (LibraryImport's EntryPoint), which are not strings.</summary>
    [GeneratedRegex("EntryPoint\\s*=\\s*\"[^\"]*\"")]
    private static partial Regex NativeEntryPoint();

    [GeneratedRegex(@"\{(\d+)(?::[^}]*)?\}")]
    private static partial Regex Placeholder();
}
