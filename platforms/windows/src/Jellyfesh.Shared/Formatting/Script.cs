namespace Jellyfesh.Formatting;

/// <summary>Writing-system checks the views need (line height for CJK, Misty TYP-08).</summary>
public static class Script
{
    /// <summary>True when the text contains Han, Kana or Hangul.</summary>
    public static bool HasCjk(string? text)
    {
        if (string.IsNullOrEmpty(text))
        {
            return false;
        }

        foreach (var rune in text.EnumerateRunes())
        {
            var v = rune.Value;
            if ((v >= 0x3040 && v <= 0x30FF) // Hiragana, Katakana
                || (v >= 0x3400 && v <= 0x4DBF) // CJK Extension A
                || (v >= 0x4E00 && v <= 0x9FFF) // CJK Unified Ideographs
                || (v >= 0xAC00 && v <= 0xD7AF) // Hangul syllables
                || (v >= 0x1100 && v <= 0x11FF) // Hangul Jamo
                || (v >= 0xF900 && v <= 0xFAFF) // CJK Compatibility Ideographs
                || (v >= 0x20000 && v <= 0x3134F)) // Extensions B–G
            {
                return true;
            }
        }

        return false;
    }

    /// <summary>True for language tags whose script is CJK (ja, zh, ko, yue, …).</summary>
    public static bool IsCjkLanguage(string? bcp47)
    {
        if (string.IsNullOrEmpty(bcp47))
        {
            return false;
        }

        var primary = bcp47.Split('-', '_')[0].ToLowerInvariant();
        return primary is "ja" or "zh" or "ko" or "yue" or "cmn" or "wuu" or "hak" or "nan";
    }
}
