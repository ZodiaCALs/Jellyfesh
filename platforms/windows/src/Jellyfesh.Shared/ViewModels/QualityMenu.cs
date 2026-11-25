using Jellyfesh.Core;
using Jellyfesh.Formatting;

namespace Jellyfesh.ViewModels;

/// <summary>
/// The quality badge and its menu (ADR-0026 §3, docs/ui.md "The player bar and the mini player"):
/// the words the app puts on what the core decides. The badge shows the playing quality's label
/// (the core's <see cref="QualityLabel"/>, as in Get Info), else the chosen level's name. The menu
/// shows the format line ("FLAC · 24-bit · 96 kHz", "AAC · 256 kbit/s"), then the levels the core
/// offers for what plays with the chosen one checked (Auto first), or why there are none: "Files
/// play as they are." or "Quality is set in {Service}.". Choosing a level is the same setting as
/// Settings › Playback › Streaming.
/// </summary>
/// <param name="Badge">The badge's words.</param>
/// <param name="FormatLine">What plays, once the engine has opened it.</param>
/// <param name="Levels">The levels to choose from, Auto first; empty when the source offers none.</param>
/// <param name="Chosen">Streaming Quality as set.</param>
/// <param name="Note">Why there are no levels.</param>
public sealed record QualityMenu(string Badge, string? FormatLine, IReadOnlyList<StreamingQuality> Levels, StreamingQuality Chosen, string? Note)
{
    /// <summary>Every level, in Settings' order.</summary>
    public static IReadOnlyList<StreamingQuality> AllLevels { get; } =
        [StreamingQuality.Auto, StreamingQuality.Lossless, StreamingQuality.High, StreamingQuality.Normal, StreamingQuality.Low];

    /// <param name="playing">What plays as decoded (<see cref="NowPlayingInfo.Quality"/>).</param>
    /// <param name="offered">The levels the core offers for what plays (<see cref="NowPlayingInfo.Qualities"/>).</param>
    /// <param name="chosen">Streaming Quality (<see cref="AppSettings.StreamingQuality"/>).</param>
    /// <param name="note">Why the core offers none (<see cref="NowPlayingInfo.QualityNote"/>, or the remote session's).</param>
    public static QualityMenu Of(AudioFormat? playing, IReadOnlyList<StreamingQuality>? offered, StreamingQuality chosen, QualityNote? note, ILocalizer l)
    {
        var levels = (offered ?? []).Where(q => q != StreamingQuality.Auto).Distinct().ToList();
        IReadOnlyList<StreamingQuality> menu = levels.Count == 0 ? [] : [StreamingQuality.Auto, .. levels];
        var why = menu.Count > 0 ? null
            : note is QualityNote.SetInService { Service: { Length: > 0 } service } ? l.Format("Quality_SetIn", service)
            : l.Get("Quality_FilesAsTheyAre");
        return new(
            playing?.Quality is { } label ? DetailsRows.Quality(label, l) : LevelName(chosen, l),
            playing is null ? null : Line(playing, l),
            menu,
            chosen,
            why);
    }

    /// <summary>A level's name: Auto, Lossless, High, Normal, Low.</summary>
    public static string LevelName(StreamingQuality level, ILocalizer l) => level switch
    {
        StreamingQuality.Lossless => l.Get("StreamingQuality_Lossless"),
        StreamingQuality.High => l.Get("StreamingQuality_High"),
        StreamingQuality.Normal => l.Get("StreamingQuality_Normal"),
        StreamingQuality.Low => l.Get("StreamingQuality_Low"),
        _ => l.Get("StreamingQuality_Auto"),
    };

    /// <summary>The format line: the codec, then the bit depth and the sample rate for lossless audio, the bitrate for the rest.</summary>
    public static string? Line(AudioFormat format, ILocalizer l)
    {
        var parts = new List<string>();
        if (format.Codec?.Trim() is { Length: > 0 } codec)
        {
            parts.Add(DetailsRows.CodecName(codec));
        }

        if (format.Lossless ?? (format.BitDepth is not null))
        {
            if (format.BitDepth is > 0 and { } bits)
            {
                parts.Add(l.Format("Info_Bits", bits));
            }

            if (format.SampleRateHz is > 0 and { } hz)
            {
                parts.Add(l.Format("Info_KHz", (hz / 1000.0).ToString(hz % 1000 == 0 ? "0" : "0.#", l.Culture)));
            }
        }
        else if (format.BitrateKbps is > 0 and { } kbps)
        {
            parts.Add(l.Format("Quality_Kbits", kbps.ToString("N0", l.Culture)));
        }

        return parts.Count == 0 ? null : string.Join(" · ", parts);
    }
}
