using Jellyfesh.Core;
using Jellyfesh.Tests.Support;
using Jellyfesh.ViewModels;
using Xunit;

namespace Jellyfesh.Tests;

/// <summary>The quality badge and its menu, and Settings › Playback › Streaming (ADR-0026 §1, §3).</summary>
public class QualityMenuTests
{
    private static readonly AudioFormat HiRes = new("flac", 96_000, 24, 2, 4_608, true, QualityLabel.HiResLossless);
    private static readonly AudioFormat Aac = new("aac", 44_100, null, 2, 256, false, QualityLabel.High);

    [Fact]
    public void The_badge_shows_the_playing_quality_else_the_chosen_level()
    {
        var menu = QualityMenu.Of(HiRes, [StreamingQuality.Auto, StreamingQuality.Lossless, StreamingQuality.High], StreamingQuality.Lossless, null, English.Instance);
        Assert.Equal("Hi-Res Lossless", menu.Badge);
        Assert.Equal("FLAC · 24-bit · 96 kHz", menu.FormatLine);
        Assert.Equal([StreamingQuality.Auto, StreamingQuality.Lossless, StreamingQuality.High], menu.Levels);
        Assert.Equal(StreamingQuality.Lossless, menu.Chosen);
        Assert.Null(menu.Note);

        // Before the engine has opened the song: the chosen level, no format line.
        var opening = QualityMenu.Of(null, [StreamingQuality.Auto, StreamingQuality.High], StreamingQuality.High, null, English.Instance);
        Assert.Equal(("High", null), (opening.Badge, opening.FormatLine));
    }

    [Fact]
    public void Lossy_audio_shows_its_bitrate_and_lossless_its_depth_and_rate()
    {
        Assert.Equal("AAC · 256 kbit/s", QualityMenu.Line(Aac, English.Instance));
        Assert.Equal("FLAC · 16-bit · 44.1 kHz", QualityMenu.Line(new AudioFormat("flac", 44_100, 16, 2, null, null, null), English.Instance));
        Assert.Equal("MP3", QualityMenu.Line(new AudioFormat("mp3", 44_100, null, 2, null, false, null), English.Instance));
        Assert.Null(QualityMenu.Line(new AudioFormat(null, null, null, null, null, null, null), English.Instance));
    }

    [Fact]
    public void Levels_put_Auto_first_and_keep_the_cores_order()
    {
        var levels = QualityMenu.Of(null, [StreamingQuality.Lossless, StreamingQuality.Auto, StreamingQuality.Low, StreamingQuality.Lossless], StreamingQuality.Auto, null, English.Instance).Levels;
        Assert.Equal([StreamingQuality.Auto, StreamingQuality.Lossless, StreamingQuality.Low], levels);

        // Auto alone is no choice: the menu says why instead.
        var alone = QualityMenu.Of(null, [StreamingQuality.Auto], StreamingQuality.Auto, null, English.Instance);
        Assert.Equal(([], "Files play as they are."), (alone.Levels, alone.Note));
    }

    [Fact]
    public void With_no_levels_the_menu_says_why()
    {
        Assert.Equal("Files play as they are.", QualityMenu.Of(HiRes, [], StreamingQuality.Auto, new QualityNote.FilesPlayAsTheyAre(), English.Instance).Note);
        Assert.Equal("Quality is set in Jellyfin.", QualityMenu.Of(Aac, null, StreamingQuality.Auto, new QualityNote.SetInService("Jellyfin"), English.Instance).Note);
        Assert.Equal("Files play as they are.", QualityMenu.Of(Aac, null, StreamingQuality.Auto, new QualityNote.SetInService(""), English.Instance).Note);
    }

    [Fact]
    public void The_player_shows_the_badge_while_a_song_plays_and_chooses_a_level()
    {
        using var h = new AppHarness(English.Instance).Started();
        var player = h.App.Player;
        Assert.False(player.HasQualityBadge);

        h.Emit(new CoreEvent.NowPlaying(new NowPlayingInfo(
            1, new TrackRef.Library(7), "/music/a.flac", "Lantern", "Artist", null, 180_000, null, null,
            Quality: HiRes, Qualities: [StreamingQuality.Auto, StreamingQuality.Lossless, StreamingQuality.High])));
        Assert.True(player.HasQualityBadge);
        Assert.Equal("Hi-Res Lossless", player.QualityBadge);
        Assert.Equal("Quality: Hi-Res Lossless", player.QualityBadgeLabel);
        Assert.Equal(StreamingQuality.Auto, player.Quality!.Chosen);

        // A level is the same setting as Settings › Playback › Streaming; the chosen one sends nothing.
        var sent = h.Core.Sent.Count;
        player.ChooseQualityCommand.Execute(StreamingQuality.Auto);
        Assert.Equal(sent, h.Core.Sent.Count);
        player.ChooseQualityCommand.Execute(StreamingQuality.High);
        var set = Assert.IsType<CoreCommand.SetSettings>(h.Core.Sent[^1]);
        Assert.Equal(h.S.Hub.State.Settings with { StreamingQuality = StreamingQuality.High }, set.Settings);

        // The slice coming back checks it.
        h.Emit(new CoreEvent.Settings(AppSettings.Default with { StreamingQuality = StreamingQuality.High }));
        Assert.Equal(StreamingQuality.High, player.Quality!.Chosen);

        h.Emit(new CoreEvent.NowPlaying(null));
        Assert.False(player.HasQualityBadge);
        Assert.Equal(string.Empty, player.QualityBadgeLabel);
    }

    [Fact]
    public void A_remote_session_shows_the_services_note()
    {
        using var h = new AppHarness(English.Instance).Started();
        h.Emit(new CoreEvent.Remote(RemoteState.Initial with
        {
            Version = 1,
            Source = "org.spotify",
            SourceName = "Spotify",
            Status = PlaybackStatus.Playing,
            QualityNote = new QualityNote.SetInService("Spotify"),
        }));
        var quality = h.App.Player.Quality!;
        Assert.Equal(([], "Quality is set in Spotify.", "Auto"), (quality.Levels, quality.Note, quality.Badge));
    }

    [Fact]
    public void Settings_offer_every_level_and_follow_the_slice()
    {
        using var h = new AppHarness().Started();
        var settings = h.App.CreateSettings(_ => null);
        settings.Activate();
        h.Ui.RunAll();
        Assert.Equal(0, settings.StreamingQualityIndex);

        h.Emit(new CoreEvent.Settings(AppSettings.Default with { StreamingQuality = StreamingQuality.Normal }));
        Assert.Equal(3, settings.StreamingQualityIndex);
        Assert.DoesNotContain(h.Core.Sent, c => c is CoreCommand.SetSettings); // no echo

        settings.StreamingQualityIndex = 1;
        var set = Assert.IsType<CoreCommand.SetSettings>(h.Core.Sent[^1]);
        Assert.Equal(StreamingQuality.Lossless, set.Settings.StreamingQuality);
        Assert.Equal(
            [StreamingQuality.Auto, StreamingQuality.Lossless, StreamingQuality.High, StreamingQuality.Normal, StreamingQuality.Low],
            QualityMenu.AllLevels);
    }
}
