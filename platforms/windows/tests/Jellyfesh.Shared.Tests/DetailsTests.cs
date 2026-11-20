using System.Globalization;
using Jellyfesh.Core;
using Jellyfesh.Formatting;
using Jellyfesh.Tests.Support;
using Jellyfesh.ViewModels;
using Xunit;

namespace Jellyfesh.Tests;

/// <summary>Get Info and the Info pane (docs/ui.md, "Details").</summary>
public class DetailsTests
{
    private static readonly ILocalizer English = new Words(new()
    {
        ["Info_Title"] = "Title", ["Info_Artist"] = "Artist", ["Info_Album"] = "Album", ["Info_Version"] = "Version",
        ["Info_Year"] = "Year", ["Info_Genre"] = "Genre", ["Info_Composer"] = "Composer", ["Info_Lyricist"] = "Lyricist",
        ["Info_Label"] = "Label", ["Info_Length"] = "Length", ["Info_Quality"] = "Quality", ["Info_Format"] = "Format",
        ["Info_Bitrate"] = "Bitrate", ["Info_Kbps"] = "{0} kbps", ["Info_Size"] = "Size", ["Info_ReplayGain"] = "ReplayGain",
        ["Info_File"] = "File", ["Info_Source"] = "Source", ["Info_Bits"] = "{0}-bit", ["Info_KHz"] = "{0} kHz",
        ["Info_Mono"] = "Mono", ["Info_Stereo"] = "Stereo", ["Info_Channels"] = "{0} channels",
        ["Info_TrackGain"] = "Track {0}", ["Info_AlbumGain"] = "Album {0}", ["Quality_HiResLossless"] = "Hi-Res Lossless",
        ["Size_MB"] = "{0} MB", ["Size_GB"] = "{0} GB", ["Fact_Genre"] = "Genre", ["Fact_Released"] = "Released",
    });

    private static DetailsPage Page(ItemRef item, ulong request = 1) => new(
        request, item, "Lantern", "Sample Artist", "Invented notes.", null,
        [new Fact(FactKind.Version, "Live"), new Fact(FactKind.Genre, "Ambient"), new Fact(FactKind.Released, "2024"), new Fact(FactKind.Composer, "A. Writer"), new Fact(FactKind.Other, "Night", "Mood")],
        new TechnicalInfo(new AudioFormat("flac", 96_000, 24, 2, 3_100, true, QualityLabel.HiResLossless), "flac", 71_234_567, 184_000,
            new ReplayGainValues(-6.5f, 0.98f, -7f, 1f), "/music/Night Swim/01 Lantern.flac"),
        3, null, null);

    [Fact]
    public void A_song_lists_about_and_audio_rows_with_values_only()
    {
        var page = Page(new ItemRef.Track(42));
        var known = new KnownDetails(Title: "Lantern (row)", Album: "Night Swim");
        Assert.Equal(
            [("Title", "Lantern"), ("Artist", "Sample Artist"), ("Album", "Night Swim"), ("Version", "Live"), ("Year", "2024"), ("Genre", "Ambient"), ("Composer", "A. Writer"), ("Length", "3:04")],
            DetailsRows.Song(page, known, English).Select(r => (r.Label, r.Value)));
        Assert.Equal(
            [("Quality", "Hi-Res Lossless"), ("Format", "FLAC · 24-bit · 96 kHz · Stereo"), ("Bitrate", "3100 kbps"), ("Size", "71.2 MB"), ("ReplayGain", "Track −6.50 dB · Album −7.00 dB"), ("File", "/music/Night Swim/01 Lantern.flac")],
            DetailsRows.Audio(page, known, English).Select(r => (r.Label, r.Value)));

        // Before the core answers: what the row knows; a source's song names its source.
        Assert.Equal([("Title", "Lantern (row)"), ("Album", "Night Swim")], DetailsRows.Song(null, known, English).Select(r => (r.Label, r.Value)));
        Assert.Equal([("Source", "Navidrome")], DetailsRows.Audio(null, new KnownDetails(Source: "Navidrome"), English).Select(r => (r.Label, r.Value)));

        // What is decoded wins.
        var decoded = new AudioFormat("mp3", 44_100, null, 1, 320, false, null);
        Assert.Equal("MP3 · 44.1 kHz · Mono", DetailsRows.Audio(page, known, English, decoded).Single(r => r.Label == "Format").Value);
    }

    [Fact]
    public void An_albums_facts_keep_the_cores_order_and_the_services_own_labels()
    {
        Assert.Equal(
            [("Version", "Live"), ("Genre", "Ambient"), ("Released", "2024"), ("Composer", "A. Writer"), ("Mood", "Night")],
            DetailsRows.Facts(Page(new ItemRef.Album(7)), new Words(new() { ["Fact_Version"] = "Version", ["Fact_Genre"] = "Genre", ["Fact_Released"] = "Released", ["Fact_Composer"] = "Composer" }))
                .Select(r => (r.Label, r.Value)));
        Assert.Equal("6 channels", DetailsRows.FormatLine(new AudioFormat(null, null, null, 6, null, null, null), English));
        Assert.Null(DetailsRows.FormatLine(new AudioFormat(null, null, null, null, null, null, null), English));
        Assert.Equal("Opus", DetailsRows.CodecName("opus"));
        Assert.Equal("Track +1.00 dB", DetailsRows.ReplayGainLine(new ReplayGainValues(1f, null, null, null), English));
    }

    [Fact]
    public async Task Get_info_asks_the_core_and_fills_in_its_answer()
    {
        using var h = new AppHarness().Started();
        var item = new ItemRef.Source("org.x", ItemKind.Track, "t1");
        var info = new DetailsViewModel(h.S, item, InfoKind.Song, new KnownDetails(Title: "Lantern (row)", Source: "Example Source"));
        Assert.Equal("Lantern (row)", info.Title);
        var load = info.LoadAsync();
        Assert.True(info.IsLoading);
        Assert.Equal(new CoreCommand.Details(item), h.Core.Sent[^1]);
        var request = (ulong)h.Core.Sent.Count;
        h.Emit(new CoreEvent.Details(Page(new ItemRef.Track(1), request: request + 5))); // another request's answer
        Assert.False(load.IsCompleted);
        h.Emit(new CoreEvent.Details(Page(item, request) with { Attribution = "Example Service" }));
        await load;
        Assert.False(info.IsLoading);
        Assert.Equal("Lantern", info.Title);
        Assert.Equal("Invented notes.", info.Description);
        Assert.Contains(info.About, r => r.Label == "Info_Version" && r.Value == "Live");
        Assert.Contains(info.Audio, r => r.Label == "Info_Quality" && r.Value == "Quality_HiResLossless");
        Assert.Equal("Source_Attribution", info.Attribution);
    }

    [Fact]
    public async Task A_refused_request_shows_why()
    {
        using var h = new AppHarness().Started();
        var info = new DetailsViewModel(h.S, new ItemRef.Album(7), InfoKind.Album, new KnownDetails(Title: "Night Swim"));
        var load = info.LoadAsync();
        h.Emit(new CoreEvent.CommandFailed((ulong)h.Core.Sent.Count, "the source gave no details"));
        await load;
        Assert.Equal("the source gave no details", info.Error);
        Assert.Equal("Night Swim", info.Title);
        Assert.Empty(h.App.Notifications.Items); // shown in Get Info, not as a notice
    }

    private sealed class Words(Dictionary<string, string> table) : ILocalizer
    {
        public CultureInfo Culture => CultureInfo.InvariantCulture;

        public string Get(string key) => table.TryGetValue(key, out var value) ? value : key;
    }
}
