using System.Collections.ObjectModel;
using System.Globalization;
using CommunityToolkit.Mvvm.ComponentModel;
using Jellyfesh.Core;
using Jellyfesh.Formatting;

namespace Jellyfesh.ViewModels;

/// <summary>One row of Get Info or the Info pane: its label and its value.</summary>
public sealed record InfoRow(string Label, string Value);

/// <summary>What a row or the player already knows about an item, shown until the core's details come.</summary>
public sealed record KnownDetails(
    string? Title = null,
    string? Artist = null,
    string? Album = null,
    string? Year = null,
    ulong? DurationMs = null,
    string? Path = null,
    string? Source = null);

/// <summary>
/// The rows of Get Info and the Info pane (docs/ui.md, "Details"): About This Song (Title, Artist,
/// Album, Version, Year, Genre, Composer, Lyricist, Label, Length) and Audio (Quality, Format,
/// Bitrate, Size, ReplayGain, File or Source); for an album or an artist, the facts in the core's
/// order. Rows without a value are left out; what is decoded wins over what the file says.
/// </summary>
public static class DetailsRows
{
    public static IReadOnlyList<InfoRow> Song(DetailsPage? page, KnownDetails known, ILocalizer l)
    {
        string? Fact(FactKind kind) => page?.Facts.FirstOrDefault(f => f.Kind == kind)?.Value;
        var length = page?.Technical?.DurationMs ?? known.DurationMs;
        return Rows(l,
        [
            ("Info_Title", page?.Title ?? known.Title),
            ("Info_Artist", page?.Subtitle ?? known.Artist),
            ("Info_Album", known.Album),
            ("Info_Version", Fact(FactKind.Version)),
            ("Info_Year", Fact(FactKind.Released) ?? known.Year),
            ("Info_Genre", Fact(FactKind.Genre)),
            ("Info_Composer", Fact(FactKind.Composer)),
            ("Info_Lyricist", Fact(FactKind.Lyricist)),
            ("Info_Label", Fact(FactKind.Label)),
            ("Info_Length", length is > 0 and { } ms ? TimeFormat.ClockMs(ms) : null),
        ]);
    }

    /// <summary>Audio; <paramref name="decoded"/> (Now Playing's) is preferred over what the details read.</summary>
    public static IReadOnlyList<InfoRow> Audio(DetailsPage? page, KnownDetails known, ILocalizer l, AudioFormat? decoded = null)
    {
        var technical = page?.Technical;
        var format = decoded ?? technical?.Format;
        var path = technical?.Path ?? known.Path;
        var source = known.Source is not null && path is null;
        return Rows(l,
        [
            ("Info_Quality", format?.Quality is { } q ? Quality(q, l) : null),
            ("Info_Format", format is null ? null : FormatLine(format, l)),
            ("Info_Bitrate", format?.BitrateKbps is > 0 and { } kbps ? l.Format("Info_Kbps", kbps) : null),
            ("Info_Size", technical?.FileSizeBytes is > 0 and { } bytes ? Size(bytes, l) : null),
            ("Info_ReplayGain", ReplayGainLine(technical?.ReplayGain, l)),
            (source ? "Info_Source" : "Info_File", source ? known.Source : path),
        ]);
    }

    /// <summary>An album's or an artist's facts, in the core's order, worded by kind.</summary>
    public static IReadOnlyList<InfoRow> Facts(DetailsPage? page, ILocalizer l) =>
        (page?.Facts ?? [])
            .Where(f => !string.IsNullOrWhiteSpace(f.Value))
            .Select(f => new InfoRow(f.Kind == FactKind.Other ? f.Label ?? string.Empty : l.Get("Fact_" + f.Kind), f.Value.Trim()))
            .Where(r => r.Label.Length > 0)
            .ToArray();

    /// <summary>The core's label, worded: "Hi-Res Lossless", "Lossless", "High", "Standard".</summary>
    public static string Quality(QualityLabel label, ILocalizer l) => l.Get("Quality_" + label);

    /// <summary>"FLAC · 24-bit · 96 kHz · Stereo".</summary>
    public static string? FormatLine(AudioFormat format, ILocalizer l)
    {
        var parts = new List<string>();
        if (format.Codec?.Trim() is { Length: > 0 } codec)
        {
            parts.Add(CodecName(codec));
        }

        if (format.BitDepth is > 0 and { } bits)
        {
            parts.Add(l.Format("Info_Bits", bits));
        }

        if (format.SampleRateHz is > 0 and { } hz)
        {
            parts.Add(l.Format("Info_KHz", (hz / 1000.0).ToString(hz % 1000 == 0 ? "0" : "0.#", l.Culture)));
        }

        if (format.Channels is > 0 and { } channels)
        {
            parts.Add(channels switch
            {
                1 => l.Get("Info_Mono"),
                2 => l.Get("Info_Stereo"),
                _ => l.Format("Info_Channels", channels),
            });
        }

        return parts.Count == 0 ? null : string.Join(" · ", parts);
    }

    /// <summary>"Track −6.20 dB · Album −7.00 dB".</summary>
    public static string? ReplayGainLine(ReplayGainValues? gain, ILocalizer l)
    {
        if (gain is null)
        {
            return null;
        }

        var parts = new List<string>();
        if (gain.TrackGainDb is { } track && float.IsFinite(track))
        {
            parts.Add(l.Format("Info_TrackGain", Decibels(track, l.Culture)));
        }

        if (gain.AlbumGainDb is { } album && float.IsFinite(album))
        {
            parts.Add(l.Format("Info_AlbumGain", Decibels(album, l.Culture)));
        }

        return parts.Count == 0 ? null : string.Join(" · ", parts);
    }

    /// <summary>"FLAC", "MP3", "AAC", "Opus": short codec names in capitals.</summary>
    public static string CodecName(string codec) => codec.ToLowerInvariant() switch
    {
        "opus" => "Opus",
        "vorbis" => "Vorbis",
        _ => codec.Length <= 4 ? codec.ToUpperInvariant() : codec,
    };

    private static string Size(ulong bytes, ILocalizer l) => bytes >= 1_000_000_000
        ? l.Format("Size_GB", (bytes / 1e9).ToString("0.##", l.Culture))
        : l.Format("Size_MB", (bytes / 1e6).ToString("0.#", l.Culture));

    private static string Decibels(float value, CultureInfo culture) =>
        (value > 0 ? "+" : value < 0 ? "−" : string.Empty) + Math.Abs(value).ToString("0.00", culture) + " dB";

    private static IReadOnlyList<InfoRow> Rows(ILocalizer l, IEnumerable<(string Key, string? Value)> pairs) =>
        pairs.Where(p => !string.IsNullOrWhiteSpace(p.Value)).Select(p => new InfoRow(l.Get(p.Key), p.Value!.Trim())).ToArray();
}

/// <summary>What Get Info shows: About This Song and Audio for a song; the facts for an album or an artist.</summary>
public enum InfoKind
{
    Song,
    Album,
    Artist,
}

/// <summary>
/// Get Info (docs/ui.md, "Details"): it asks the core for the item's details (Details, answered by
/// request id) and shows what the row already knows meanwhile. The rows come from DetailsRows; the
/// picture, when there is one, comes through the core like every source picture.
/// </summary>
public sealed partial class DetailsViewModel : ObservableObject
{
    /// <summary>How long Get Info waits for the core before it keeps what it knows.</summary>
    public static readonly TimeSpan Wait = TimeSpan.FromSeconds(20);

    private readonly ShellServices _s;
    private readonly KnownDetails _known;
    private DetailsPage? _page;

    [ObservableProperty]
    private bool _isLoading;

    [ObservableProperty]
    private string _title = string.Empty;

    [ObservableProperty]
    private string _description = string.Empty;

    [ObservableProperty]
    private string _error = string.Empty;

    public DetailsViewModel(ShellServices services, ItemRef item, InfoKind kind, KnownDetails known)
    {
        _s = services;
        Item = item;
        Kind = kind;
        _known = known;
        Show();
    }

    public ItemRef Item { get; }

    public InfoKind Kind { get; }

    /// <summary>About This Song, or an album's or artist's facts.</summary>
    public ObservableCollection<InfoRow> About { get; } = [];

    /// <summary>Audio (songs).</summary>
    public ObservableCollection<InfoRow> Audio { get; } = [];

    /// <summary>"Content from {Service}", when the service asks for credit.</summary>
    public string Attribution => _page?.Attribution is { Length: > 0 } a ? _s.L.Format("Source_Attribution", a) : string.Empty;

    /// <summary>Asks the core and fills the rows with its answer (or keeps what is known when it does not come).</summary>
    public async Task LoadAsync()
    {
        IsLoading = true;
        var answer = new TaskCompletionSource<DetailsPage>(TaskCreationOptions.RunContinuationsAsynchronously);
        ulong request = 0;
        void OnNotification(CoreEvent e)
        {
            if (e is CoreEvent.Details d && (d.Page.Request == request || (request == 0 && d.Page.Item == Item)))
            {
                answer.TrySetResult(d.Page);
            }
            else if (e is CoreEvent.CommandFailed f && f.Id == request)
            {
                answer.TrySetException(new InvalidOperationException(f.Message));
            }
        }

        _s.Hub.Notification += OnNotification;
        try
        {
            request = _s.SendHandled(new CoreCommand.Details(Item));
            _page = await answer.Task.WaitAsync(Wait);
            Error = _page.Error ?? string.Empty;
        }
        catch (TimeoutException)
        {
            // What the row knows stays.
        }
        catch (InvalidOperationException e)
        {
            Error = e.Message;
        }
        finally
        {
            _s.Hub.Notification -= OnNotification;
            IsLoading = false;
        }

        Show();
    }

    private void Show()
    {
        Title = _page?.Title ?? _known.Title ?? string.Empty;
        Description = _page?.Description?.Trim() ?? string.Empty;
        About.Clear();
        Audio.Clear();
        var about = Kind == InfoKind.Song ? DetailsRows.Song(_page, _known, _s.L) : DetailsRows.Facts(_page, _s.L);
        foreach (var row in about)
        {
            About.Add(row);
        }

        if (Kind == InfoKind.Song)
        {
            foreach (var row in DetailsRows.Audio(_page, _known, _s.L))
            {
                Audio.Add(row);
            }
        }

        OnPropertyChanged(nameof(Attribution));
    }
}
