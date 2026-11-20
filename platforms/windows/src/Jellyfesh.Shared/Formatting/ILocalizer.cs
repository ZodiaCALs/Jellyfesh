using System.Globalization;

namespace Jellyfesh.Formatting;

/// <summary>
/// Localised strings. The app reads the .resw files (English, Simplified Chinese, Traditional
/// Chinese, Japanese); keys are the resource names. Whole messages, never concatenated fragments
/// (Misty INT-10).
/// </summary>
public interface ILocalizer
{
    /// <summary>The string for <paramref name="key"/>, or the key itself when missing.</summary>
    string Get(string key);

    CultureInfo Culture { get; }
}

public static class LocalizerExtensions
{
    /// <summary>Formats the string for <paramref name="key"/> with composite-format arguments.</summary>
    public static string Format(this ILocalizer l, string key, params object?[] args) =>
        string.Format(l.Culture, l.Get(key), args);

    /// <summary>
    /// A count message: <c>key_One</c> for exactly one, otherwise <c>key_Other</c>, formatted with
    /// the count. Languages without plural forms give both keys the same text.
    /// </summary>
    public static string Plural(this ILocalizer l, string key, long count) =>
        string.Format(l.Culture, l.Get(count == 1 ? key + "_One" : key + "_Other"), count.ToString("N0", l.Culture));

    /// <summary>
    /// The language the app's words are in, as a BCP 47 tag (en, zh-Hans, zh-Hant, ja): the
    /// strings' own Language_Tag, so plugins' settings follow the language the app shows.
    /// </summary>
    public static string LanguageTag(this ILocalizer l) =>
        l.Get("Language_Tag") is { Length: > 0 } tag && tag != "Language_Tag" ? tag : "en";
}

/// <summary>Returns keys (and arguments) as they are: for tests and before resources load.</summary>
public sealed class KeyLocalizer : ILocalizer
{
    public static KeyLocalizer Instance { get; } = new();

    public CultureInfo Culture => CultureInfo.InvariantCulture;

    public string Get(string key) => key switch
    {
        _ when key.EndsWith("_One", StringComparison.Ordinal) || key.EndsWith("_Other", StringComparison.Ordinal) => key + ":{0}",
        _ => key,
    };
}
