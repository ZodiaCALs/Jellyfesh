using System.Globalization;
using System.Text.Json;
using Jellyfesh.Appearance;
using Jellyfesh.Core;
using Jellyfesh.Drawing;
using Jellyfesh.Formatting;
using Jellyfesh.Tests.Support;
using Xunit;

namespace Jellyfesh.Tests;

/// <summary>
/// The v0.0.4 design pass (docs/design-controls.md): the straight seek bar's geometry, the player
/// bar's breakpoint, Now Playing's motion numbers, the accent swatches and the new settings.
/// </summary>
public class DesignTests
{
    [Fact]
    public void Geometry_constants_are_the_tokens()
    {
        using var tokens = JsonDocument.Parse(File.ReadAllText(Contract.PathOf("design", "tokens.json")));
        double Dim(string group, string key) => tokens.RootElement.GetProperty(group).GetProperty(key).GetProperty("$value").GetProperty("value").GetDouble();
        double Num(string group, string key) => tokens.RootElement.GetProperty(group).GetProperty(key).GetProperty("$value").GetDouble();
        Assert.Equal(SeekGeometry.Track, Dim("player", "scrubber-track"));
        Assert.Equal(SeekGeometry.TrackHover, Dim("player", "scrubber-track-hover"));
        Assert.Equal(SeekGeometry.TrackDrag, Dim("player", "scrubber-track-drag"));
        Assert.Equal(SeekGeometry.TrackTouch, Dim("player", "scrubber-track-touch"));
        Assert.Equal(SeekGeometry.TrackTouchDrag, Dim("player", "scrubber-track-touch-drag"));
        Assert.Equal(SeekGeometry.Thumb, Dim("player", "scrubber-thumb"));
        Assert.Equal(SeekGeometry.MiniProgress, Dim("player", "mini-progress"));
        Assert.Equal(SeekGeometry.MiniRadius, Dim("player", "mini-radius"));
        Assert.Equal(SeekGeometry.BarHeight, Dim("player", "bar-height"));
        Assert.Equal(NowPlayingMotion.ArtworkPausedScale, Num("player", "artwork-paused-scale"));
        Assert.Equal(NowPlayingMotion.InactiveLineScale, Num("lyrics", "scale-inactive"));
        Assert.Equal(NowPlayingMotion.SmallLyrics, Num("lyrics", "scale-small"));
        Assert.Equal(NowPlayingMotion.LargeLyrics, Num("lyrics", "scale-large"));
        Assert.Equal(NowPlayingMotion.Stagger.TotalMilliseconds, Dim("lyrics", "stagger"));
    }

    [Fact]
    public void The_seek_line_thickens_on_hover_and_drag_and_shows_a_thumb_on_pointer_screens_only()
    {
        Assert.Equal(4, SeekGeometry.Thickness(SeekBarState.Rest, touch: false));
        Assert.Equal(6, SeekGeometry.Thickness(SeekBarState.Hover, touch: false));
        Assert.Equal(8, SeekGeometry.Thickness(SeekBarState.Drag, touch: false));
        Assert.Equal(6, SeekGeometry.Thickness(SeekBarState.Rest, touch: true));
        Assert.Equal(12, SeekGeometry.Thickness(SeekBarState.Drag, touch: true));
        Assert.False(SeekGeometry.ShowsThumb(SeekBarState.Rest, touch: false));
        Assert.True(SeekGeometry.ShowsThumb(SeekBarState.Hover, touch: false));
        Assert.True(SeekGeometry.ShowsThumb(SeekBarState.Drag, touch: false));
        Assert.False(SeekGeometry.ShowsThumb(SeekBarState.Drag, touch: true));
    }

    [Fact]
    public void The_track_is_inset_so_the_thumb_stays_inside()
    {
        // A 214 px bar: 7 px at each end for the 14 px thumb, a 200 px line.
        Assert.Equal(200, SeekGeometry.TrackLength(214, touch: false));
        Assert.Equal(0, SeekGeometry.FractionAt(0, 214, touch: false, rightToLeft: false));
        Assert.Equal(0.25, SeekGeometry.FractionAt(57, 214, touch: false, rightToLeft: false));
        Assert.Equal(1, SeekGeometry.FractionAt(500, 214, touch: false, rightToLeft: false));
        Assert.Equal(0.75, SeekGeometry.FractionAt(57, 214, touch: false, rightToLeft: true));
        Assert.Equal(50, SeekGeometry.PlayedLength(0.25, 214, touch: false));
        Assert.Equal(57, SeekGeometry.ThumbCentre(0.25, 214, touch: false, rightToLeft: false));
        Assert.Equal(157, SeekGeometry.ThumbCentre(0.25, 214, touch: false, rightToLeft: true));
        Assert.Equal(0, SeekGeometry.FractionAt(double.NaN, 214, touch: false, rightToLeft: false));
        Assert.Equal(0, SeekGeometry.FractionAt(5, 10, touch: false, rightToLeft: false)); // narrower than the thumb
    }

    [Fact]
    public void The_bar_redraws_once_per_device_pixel_of_its_line()
    {
        Assert.Equal(200, SeekGeometry.Steps(214, 1, touch: false));
        Assert.Equal(300, SeekGeometry.Steps(214, 1.5, touch: false));
        Assert.Equal(200, SeekGeometry.Steps(214, double.NaN, touch: false));
        // The mini-player's line runs between the capsule's corners: 360 − 2 × 16.
        Assert.Equal(328, SeekGeometry.MiniTrackLength(360));
        Assert.Equal(82, SeekGeometry.MiniProgressLength(0.25, 360));
        Assert.Equal(656, SeekGeometry.MiniSteps(360, 2));
        // The compact player's line spans its glass strip.
        Assert.Equal(77, SeekGeometry.LineLength(0.25, 308));
        Assert.Equal(0, SeekGeometry.LineLength(double.NaN, 308));
        Assert.Equal(308, SeekGeometry.LineLength(2, 308));
        Assert.Equal(616, SeekGeometry.LineSteps(308, 2));
        Assert.Equal(0, SeekGeometry.LineSteps(-5, 2));
    }

    [Theory]
    [InlineData(SeekKey.Back, 60, 55)]
    [InlineData(SeekKey.Forward, 60, 65)]
    [InlineData(SeekKey.PageBack, 60, 30)]
    [InlineData(SeekKey.PageForward, 60, 90)]
    [InlineData(SeekKey.Start, 60, 0)]
    [InlineData(SeekKey.End, 60, 225)]
    [InlineData(SeekKey.Back, 2, 0)]
    [InlineData(SeekKey.PageForward, 210, 225)]
    public void Keys_step_five_and_thirty_seconds_within_the_track(SeekKey key, double from, double expected) =>
        Assert.Equal(expected, SeekGeometry.KeyTarget(key, from, 225));

    [Fact]
    public void The_player_bar_takes_over_at_900_pixels() =>
        Assert.Equal([false, true, true], new[] { 899.5, 900, 1280 }.Select(SeekGeometry.UsesPlayerBar));

    [Fact]
    public void The_artwork_rests_smaller_while_paused_except_with_reduced_motion()
    {
        Assert.Equal(1, NowPlayingMotion.ArtworkScale(playing: true, reduceMotion: false));
        Assert.Equal(0.88, NowPlayingMotion.ArtworkScale(playing: false, reduceMotion: false));
        Assert.Equal(1, NowPlayingMotion.ArtworkScale(playing: false, reduceMotion: true));
    }

    [Fact]
    public void Lyric_lines_take_their_role_scale_and_size_from_the_active_line()
    {
        Assert.Equal(LyricLineRole.Sung, NowPlayingMotion.Role(2, 3));
        Assert.Equal(LyricLineRole.Active, NowPlayingMotion.Role(3, 3));
        Assert.Equal(LyricLineRole.Upcoming, NowPlayingMotion.Role(4, 3));
        Assert.Equal(LyricLineRole.Upcoming, NowPlayingMotion.Role(0, -1)); // before the first line
        Assert.Equal(1, NowPlayingMotion.LineScale(LyricLineRole.Active));
        Assert.Equal(0.96, NowPlayingMotion.LineScale(LyricLineRole.Sung));
        Assert.Equal(0.8, NowPlayingMotion.LyricsScale(LyricsSize.Small));
        Assert.Equal(1, NowPlayingMotion.LyricsScale(LyricsSize.Medium));
        Assert.Equal(1.2, NowPlayingMotion.LyricsScale(LyricsSize.Large));
    }

    [Fact]
    public void Each_following_line_starts_thirty_milliseconds_later()
    {
        Assert.Equal(TimeSpan.Zero, NowPlayingMotion.CascadeDelay(2, 4));
        Assert.Equal(TimeSpan.Zero, NowPlayingMotion.CascadeDelay(4, 4));
        Assert.Equal(TimeSpan.FromMilliseconds(30), NowPlayingMotion.CascadeDelay(5, 4));
        Assert.Equal(TimeSpan.FromMilliseconds(90), NowPlayingMotion.CascadeDelay(7, 4));
        Assert.Equal(TimeSpan.FromMilliseconds(240), NowPlayingMotion.CascadeDelay(40, 4)); // capped at eight lines
        Assert.Equal(TimeSpan.Zero, NowPlayingMotion.CascadeDelay(5, -1));
    }

    [Fact]
    public void The_cascade_lag_turns_one_scroll_into_a_later_start()
    {
        var fluid = new CubicBezierCurve(0.22, 1, 0.36, 1);
        var duration = TimeSpan.FromMilliseconds(380);
        var delay = TimeSpan.FromMilliseconds(90);
        var frames = NowPlayingMotion.CascadeLag(60, delay, duration, fluid);
        Assert.Equal(13, frames.Count);
        Assert.Equal(0, frames[0].Offset);
        Assert.Equal(0, frames[^1].Offset);
        Assert.All(frames, f => Assert.InRange(f.Offset, 0, 60));
        Assert.Contains(frames, f => f.Offset > 10);
        // Where the line is drawn (scroll minus lag) is the scroll started `delay` later.
        var total = duration.TotalMilliseconds + delay.TotalMilliseconds;
        foreach (var f in frames.Take(frames.Count - 1))
        {
            var t = f.Progress * total;
            var moved = (60 * fluid.Ease(t / duration.TotalMilliseconds)) - f.Offset;
            Assert.Equal(60 * fluid.Ease((t - delay.TotalMilliseconds) / duration.TotalMilliseconds), moved, 9);
        }

        Assert.All(NowPlayingMotion.CascadeLag(60, TimeSpan.Zero, duration, fluid), f => Assert.Equal(0, f.Offset, 9));
    }

    [Fact]
    public void The_oklch_port_reproduces_the_token_colours()
    {
        // Each token's hex in design/tokens.json at hue 250, light.
        using var tokens = JsonDocument.Parse(File.ReadAllText(Contract.PathOf("design", "tokens.json")));
        var checkedColours = 0;
        foreach (var group in new[] { "accent", "semantic", "field" })
        {
            foreach (var token in tokens.RootElement.GetProperty(group).EnumerateObject())
            {
                if (token.Value.ValueKind != JsonValueKind.Object || !token.Value.TryGetProperty("$value", out var value)
                    || value.ValueKind != JsonValueKind.Object || !value.TryGetProperty("components", out var c))
                {
                    continue;
                }

                var rgba = Oklch.ToRgba(c[0].GetDouble(), c[1].GetDouble(), c[2].GetDouble());
                var hex = string.Create(CultureInfo.InvariantCulture, $"#{rgba.R:x2}{rgba.G:x2}{rgba.B:x2}");
                Assert.Equal(value.GetProperty("hex").GetString(), hex);
                checkedColours++;
            }
        }

        Assert.True(checkedColours >= 10, $"only {checkedColours} colours");
        Assert.Equal(new Rgba(0x00, 0x59, 0xA7, 0xFF), AccentChoices.Accent(250, dark: false));
        Assert.Equal(AccentChoices.Accent(10, dark: true), AccentChoices.Accent(370, dark: true));
    }

    [Fact]
    public void Swatches_match_fixed_hues_and_nothing_else()
    {
        Assert.Equal([250f, 210, 180, 150, 120, 60, 30, 0, 330, 290], AccentChoices.SwatchHues);
        Assert.Equal(0, AccentChoices.SwatchIndex(new HueSource.Fixed(250)));
        Assert.Equal(7, AccentChoices.SwatchIndex(new HueSource.Fixed(359.8f))); // 0° round the wheel
        Assert.Equal(8, AccentChoices.SwatchIndex(new HueSource.Fixed(-30)));
        Assert.Equal(-1, AccentChoices.SwatchIndex(new HueSource.Fixed(212.5f)));
        Assert.Equal(-1, AccentChoices.SwatchIndex(new HueSource.Artwork()));
        Assert.Equal(13, AccentChoices.HueTrack(dark: false).Count);
        Assert.Equal(AccentChoices.HueTrack(dark: false)[0].Color, AccentChoices.HueTrack(dark: false)[^1].Color);
    }

    [Fact]
    public void Album_lengths_read_in_words()
    {
        var l = new TableLocalizer(new Dictionary<string, string>
        {
            ["Count_Minutes_One"] = "{0} minute",
            ["Count_Minutes_Other"] = "{0} minutes",
            ["Count_Hours_One"] = "{0} hour",
            ["Count_Hours_Other"] = "{0} hours",
            ["Time_HoursAndMinutes"] = "{0} {1}",
        });
        Assert.Equal("48 minutes", TimeFormat.LongTotal(48 * 60_000 + 10_000, l));
        Assert.Equal("1 minute", TimeFormat.LongTotal(50_000, l));
        Assert.Equal("1 hour", TimeFormat.LongTotal(60 * 60_000, l));
        Assert.Equal("2 hours 1 minute", TimeFormat.LongTotal(121 * 60_000, l));
    }

    [Fact]
    public void Appearance_settings_reach_the_core_and_follow_its_slice()
    {
        using var h = new AppHarness();
        h.Started();
        var settings = h.App.CreateSettings(_ => null);
        settings.Activate(); // a page follows the slices while it is shown
        Assert.Equal(10, settings.Swatches.Count);
        Assert.Equal(50, settings.Transparency);
        Assert.Equal(1, settings.LyricsSizeIndex);
        Assert.Empty(h.Core.Sent);

        settings.ChooseSwatchCommand.Execute(settings.Swatches[2]);
        Assert.Equal(new HueSource.Fixed(180), h.Core.LastSent<CoreCommand.SetSettings>().Settings.HueSource);
        Assert.Equal(2, settings.SwatchIndex);
        Assert.False(settings.HueFromArtwork);

        settings.FixedHue = 200;
        Assert.Equal(new HueSource.Fixed(200), h.Core.LastSent<CoreCommand.SetSettings>().Settings.HueSource);
        Assert.Equal(-1, settings.SwatchIndex); // a custom hue

        settings.Transparency = 25;
        Assert.Equal(0.25f, h.Core.LastSent<CoreCommand.SetSettings>().Settings.Transparency);
        settings.LyricsSizeIndex = 2;
        Assert.Equal(LyricsSize.Large, h.Core.LastSent<CoreCommand.SetSettings>().Settings.LyricsSize);
        settings.LyricsAlignIndex = 1;
        Assert.Equal(LyricsAlign.Center, h.Core.LastSent<CoreCommand.SetSettings>().Settings.LyricsAlign);

        var sent = h.Core.Sent.Count;
        h.Emit(new CoreEvent.Settings(AppSettings.Default with
        {
            HueSource = new HueSource.Fixed(330),
            Transparency = 0.8f,
            LyricsSize = LyricsSize.Small,
            LyricsAlign = LyricsAlign.Center,
        }));
        Assert.Equal(8, settings.SwatchIndex);
        Assert.Equal(0, settings.LyricsSizeIndex);
        Assert.Equal(1, settings.LyricsAlignIndex);
        Assert.Equal(sent, h.Core.Sent.Count); // following the slice sends nothing
        settings.Deactivate();
    }

    [Fact]
    public async Task The_album_page_marks_the_playing_row_and_sums_up_its_songs()
    {
        using var h = new AppHarness();
        h.Core.TrackRows.Add(Samples.Track(1, "One", albumId: 7, durationMs: 1_500_000));
        h.Core.TrackRows.Add(Samples.Track(2, "Two", albumId: 7, durationMs: 1_380_000));
        h.Core.ArtistRows.Add(new Artist(9, "Artist", 2));
        h.Started();
        var album = h.App.CreateAlbum(new Album(7, "Night Swim", "Artist", 2024, 2, null));
        album.Activate();
        await album.LoadAsync();
        Assert.Equal(2, album.Tracks.Count);
        Assert.Equal("Album_Summary", album.Footer); // the key localizer returns keys
        Assert.Contains("2024", album.Details, StringComparison.Ordinal);
        Assert.DoesNotContain(album.Tracks, t => t.IsCurrent);

        h.Emit(new CoreEvent.NowPlaying(new NowPlayingInfo(5, new TrackRef.Library(2), null, "Two", "Artist", "Night Swim", 1_380_000, null, null)));
        Assert.False(album.Tracks[0].IsCurrent);
        Assert.True(album.Tracks[1].IsCurrent);

        Artist? opened = null;
        album.ArtistRequested += a => opened = a;
        await album.OpenArtistCommand.ExecuteAsync(null);
        Assert.Equal(9, opened?.Id);
    }

    private sealed class TableLocalizer(Dictionary<string, string> table) : ILocalizer
    {
        public CultureInfo Culture => CultureInfo.InvariantCulture;

        public string Get(string key) => table.TryGetValue(key, out var value) ? value : key;
    }
}
