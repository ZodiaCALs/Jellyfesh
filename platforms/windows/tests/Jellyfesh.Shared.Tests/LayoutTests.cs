using System.Text.Json;
using Jellyfesh.Core;
using Jellyfesh.Drawing;
using Jellyfesh.Formatting;
using Jellyfesh.Tests.Support;
using Jellyfesh.ViewModels;
using Xunit;

namespace Jellyfesh.Tests;

/// <summary>
/// The one layout (ADR-0023, docs/ui.md "Navigation"): Jellyfesh Web's breakpoints and measures, the
/// phone tabs from the look's places, and Open Files….
/// </summary>
public class LayoutTests
{
    [Fact]
    public void The_sidebar_shows_from_761_epx_and_the_phone_tabs_below()
    {
        Assert.False(ShellLayout.For(760, hasTrack: false, MiniPlayerStyle.Bar).Sidebar);
        Assert.True(ShellLayout.For(761, hasTrack: false, MiniPlayerStyle.Bar).Sidebar);
        Assert.True(ShellLayout.Initial.Sidebar);
        Assert.Equal(0, ShellLayout.Initial.Floor); // nothing plays yet, so nothing floats
    }

    [Fact]
    public void The_player_bar_shows_from_900_epx_for_the_bar_style()
    {
        Assert.False(ShellLayout.For(899, hasTrack: true, MiniPlayerStyle.Bar).Bar);
        Assert.True(ShellLayout.For(900, hasTrack: true, MiniPlayerStyle.Bar).Bar);
        Assert.False(ShellLayout.For(1600, hasTrack: true, MiniPlayerStyle.Compact).Bar);
        Assert.False(ShellLayout.For(1600, hasTrack: true, MiniPlayerStyle.Floating).Bar);

        // A narrow bar keeps its mute button and drops the volume line (the web's 1040 px).
        Assert.False(ShellLayout.For(1040, hasTrack: true, MiniPlayerStyle.Bar).VolumeLine);
        Assert.True(ShellLayout.For(1041, hasTrack: true, MiniPlayerStyle.Bar).VolumeLine);
    }

    /// <summary>v0.0.9: every part of the bar keeps to its own column at any width, so nothing in it overlaps.</summary>
    [Fact]
    public void The_player_bar_keeps_every_part_in_its_own_column()
    {
        // The web's container queries: the volume line from 900 epx, Lyrics and Queue from 760, mute from 640.
        Assert.Equal(new PlayerBarLayout(VolumeLine: true, Panes: true, Mute: true), PlayerBarLayout.For(968));
        Assert.Equal(new PlayerBarLayout(VolumeLine: false, Panes: true, Mute: true), PlayerBarLayout.For(899));
        Assert.Equal(new PlayerBarLayout(VolumeLine: false, Panes: false, Mute: true), PlayerBarLayout.For(759));
        Assert.Equal(new PlayerBarLayout(VolumeLine: false, Panes: false, Mute: false), PlayerBarLayout.For(639));

        // A 1280 epx window's bar (968 beside the sidebar) with every tool: the middle at its widest.
        Assert.Equal(340, PlayerBarLayout.MiddleWidth(968, 452));
        Assert.Equal(120, PlayerBarLayout.TitleWidth(968, 452));

        // Narrower, the title keeps its 112 and the middle gives way.
        Assert.Equal(271, PlayerBarLayout.MiddleWidth(760, 321));
        Assert.Equal(112, PlayerBarLayout.TitleWidth(760, 321));

        // The narrowest bar (a 900 epx window) with a long quality badge: the transport keeps its 244.
        Assert.Equal(244, PlayerBarLayout.MiddleWidth(588, 212));
        Assert.Equal(76, PlayerBarLayout.TitleWidth(588, 212));

        // The columns add up to the bar: none reaches into another.
        foreach (var (width, tools) in new[] { (1280.0, 452.0), (968.0, 452.0), (760.0, 321.0), (588.0, 170.0), (588.0, 212.0) })
        {
            var used = PlayerBarLayout.Padding + (2 * PlayerBarLayout.Gap) + PlayerBarLayout.TitleWidth(width, tools) + PlayerBarLayout.MiddleWidth(width, tools) + tools;
            Assert.Equal(width, used, 6);
        }
    }

    [Fact]
    public void Pages_scroll_clear_of_the_floating_capsules()
    {
        // Wide: the player bar (80) 16 above the bottom; nothing floats without a song.
        var wide = ShellLayout.For(1280, hasTrack: true, MiniPlayerStyle.Bar);
        Assert.Equal(96, wide.Floor);
        Assert.Equal(96 + 48 + 32, wide.BottomPadding);
        Assert.Equal(16, wide.DockBottom);
        Assert.Equal(0, ShellLayout.For(1280, hasTrack: false, MiniPlayerStyle.Bar).Floor);
        Assert.Equal(0, ShellLayout.For(1280, hasTrack: false, MiniPlayerStyle.Bar).FadeHeight);

        // Phones: the tab bar (68) 24 above the bottom, and the capsule (64) 12 above it.
        var phone = ShellLayout.For(400, hasTrack: true, MiniPlayerStyle.Bar);
        Assert.Equal(68 + 24 + 12 + 64, phone.Floor);
        Assert.Equal(68 + 24 + 12, phone.DockBottom);
        Assert.Equal(14, phone.DockSide);
        Assert.Equal(68 + 24, ShellLayout.For(400, hasTrack: false, MiniPlayerStyle.Bar).Floor);
    }

    [Fact]
    public void Gutters_and_titles_follow_the_web()
    {
        Assert.Equal(32, ShellLayout.For(1280, hasTrack: false, MiniPlayerStyle.Bar).Gutter);
        Assert.Equal(30.44, ShellLayout.For(761, hasTrack: false, MiniPlayerStyle.Bar).Gutter, 6); // 4 % of the window
        Assert.Equal(16, ShellLayout.For(500, hasTrack: false, MiniPlayerStyle.Bar).Gutter);
        Assert.Equal(48, ShellLayout.For(900, hasTrack: false, MiniPlayerStyle.Bar).TitleSize);
        Assert.Equal(30.4, ShellLayout.For(899, hasTrack: false, MiniPlayerStyle.Bar).TitleSize);
    }

    [Fact]
    public void The_measures_are_the_tokens()
    {
        using var tokens = JsonDocument.Parse(File.ReadAllText(Contract.PathOf("design", "tokens.json")));
        JsonElement Token(string group, string key) => tokens.RootElement.GetProperty(group).GetProperty(key);
        double Px(string group, string key)
        {
            var value = Token(group, key).GetProperty("$value");
            var number = value.GetProperty("value").GetDouble();
            return value.GetProperty("unit").GetString() == "rem" ? number * 16 : number;
        }

        Assert.Equal(ShellLayout.S3, Px("space", "s3"));
        Assert.Equal(ShellLayout.S4, Px("space", "s4"));
        Assert.Equal(ShellLayout.S6, Px("space", "s6"));
        Assert.Equal(ShellLayout.S7, Px("space", "s7"));
        Assert.Equal(ShellLayout.CapsuleHeight, Px("player", "mini-height"));
        Assert.Equal(ShellLayout.Fade, Px("edge-blur", "bottom-height"));
        Assert.Equal(ShellLayout.TitleWide, Px("type", "text-h1"));
        Assert.Equal(ShellLayout.TitleNarrow, Px("type", "text-h2"), 6);
        var fluid = Token("layout", "gutter").GetProperty("$extensions").GetProperty("org.jellyfesh").GetProperty("fluid");
        Assert.Equal(ShellLayout.GutterMin, fluid.GetProperty("min").GetDouble() * 16);
        Assert.Equal(ShellLayout.GutterMax, fluid.GetProperty("max").GetDouble() * 16);
        Assert.Equal(4.0, fluid.GetProperty("vw").GetDouble());
    }

    [Fact]
    public void Album_grids_have_four_large_covers_on_a_1280_epx_window()
    {
        // 1280 − the sidebar's 248 − two 32 epx gutters = 968 epx of content.
        var wide = Drawing.CoverGrid.For(968, phone: false);
        Assert.Equal(4, wide.Columns);
        Assert.Equal(221, wide.Cover);
        Assert.Equal(28, wide.Spacing); // the 12 epx gap and two 8 epx card edges
        Assert.True((wide.Columns * wide.Slot) - wide.Spacing <= 968);

        // Columns never fall under 196 epx; wider windows get more of them.
        Assert.Equal(5, Drawing.CoverGrid.For(1200, phone: false).Columns);
        Assert.Equal(1, Drawing.CoverGrid.For(100, phone: false).Columns);

        // Phones: 150 epx columns, 4 apart.
        var phone = Drawing.CoverGrid.For(328, phone: true);
        Assert.Equal(2, phone.Columns);
        Assert.Equal(154, phone.Cover);
        Assert.Equal(20, phone.Spacing);
    }

    [Fact]
    public void The_phone_tabs_put_the_library_in_one_tab_and_settings_last()
    {
        Assert.Equal(["home", "library", "search", "sources", "settings"], LookRules.PhoneTabs(LookState.Default));
        Assert.Equal(["albums", "artists", "songs", "playlists", "favorites", "downloads"], LookRules.LibraryTags(LookState.Default));

        // A customized look orders them; the Library tab stands where its first place is.
        var custom = LookState.Default with { Sidebar = [Place.Search, Place.Songs, Place.Home, Place.Albums] };
        Assert.Equal(["search", "library", "home", "settings"], LookRules.PhoneTabs(custom));
        Assert.Equal(["songs", "albums", "downloads"], LookRules.LibraryTags(custom));
        Assert.Equal("library", LookRules.PhoneTab("favorites"));
        Assert.Equal("library", LookRules.PhoneTab("downloads"));
        Assert.Equal("settings", LookRules.PhoneTab("settings"));
    }

    [Fact]
    public void Settings_lists_the_shared_keyboard_shortcuts_in_uimd_order()
    {
        var keys = KeyboardShortcuts.Shared.Select(s => s.Keys).ToArray();
        Assert.Equal(["Space", "Ctrl + →", "Ctrl + ←", "Shift + →", "Shift + ←", "Ctrl + ↑", "Ctrl + ↓", "Ctrl + F", "Ctrl + M", "Ctrl + ,", "Esc"], keys);
        var shown = KeyboardShortcuts.All(KeyLocalizer.Instance);
        Assert.Equal("Shortcut_MiniPlayer", shown[8].Action); // the localizer's words for each key
        using var h = new AppHarness().Started();
        Assert.Equal(11, h.App.CreateSettings(_ => null).Shortcuts.Count);
    }

    [Fact]
    public void Open_files_plays_the_files_picked_without_adding_them()
    {
        using var h = new AppHarness().Started();
        h.Platform.Files.AddRange(["C:/Music/a.flac", "C:/Music/b.mp3"]);
        h.App.OpenFilesCommand.Execute(null);
        h.Ui.RunAll();
        var play = Assert.Single(h.Core.Sent.OfType<CoreCommand.PlayTracks>());
        Assert.Equal([new TrackRef.File("C:/Music/a.flac"), new TrackRef.File("C:/Music/b.mp3")], play.Tracks);
        Assert.Equal(0u, play.Start);
        Assert.DoesNotContain(h.Core.Sent, c => c is CoreCommand.AddRoot);

        // Cancelled: nothing is sent.
        h.Platform.Files.Clear();
        h.App.OpenFilesCommand.Execute(null);
        h.Ui.RunAll();
        Assert.Single(h.Core.Sent.OfType<CoreCommand.PlayTracks>());
    }
}
