using Jellyfesh.Core;
using Jellyfesh.Tests.Support;
using Xunit;

namespace Jellyfesh.Tests;

/// <summary>How the Windows app applies the Look slice (ADR-0020 §2).</summary>
public class LookTests
{
    [Fact]
    public void The_navigation_pane_follows_the_slices_places_in_order()
    {
        Assert.Equal(["home", "albums", "artists", "songs", "playlists", "favorites", "search", "sources"], LookRules.SidebarTags(LookState.Default));
        var custom = LookState.Default with { Sidebar = [Place.Songs, Place.Home, Place.Search, Place.Songs] };
        Assert.Equal(["songs", "home", "search"], LookRules.SidebarTags(custom));
        Assert.True(LookRules.IsLibraryTag("favorites"));
        Assert.False(LookRules.IsLibraryTag("sources"));
    }

    [Fact]
    public void Faces_are_the_systems_own_and_the_aurora_scales_down()
    {
        Assert.Equal("Georgia", LookRules.FontFamily(Typeface.Serif));
        Assert.StartsWith("Cascadia Mono", LookRules.FontFamily(Typeface.Mono), StringComparison.Ordinal);
        Assert.Equal(LookRules.FontFamily(Typeface.System), LookRules.FontFamily(Typeface.Rounded)); // no rounded face on Windows
        Assert.Equal(1, LookRules.AuroraOpacity(LookState.Default));
        Assert.Equal(0.5, LookRules.AuroraOpacity(LookState.Default with { Aurora = 0.375f }), 6);
        Assert.Equal(1, LookRules.AuroraOpacity(LookState.Default with { Aurora = 1f }));
    }

    [Fact]
    public void Glass_scales_the_transparency_setting_and_solid_stays_solid()
    {
        Assert.Equal(0.5f, LookRules.GlassTransparency(LookState.Default, 0.5f), 6);
        Assert.Equal(0.25f, LookRules.GlassTransparency(LookState.Default with { Glass = 0.5f }, 0.5f), 6);
        Assert.Equal(0f, LookRules.GlassTransparency(LookState.Default with { Glass = 0f }, 1f)); // a look without glass is solid
        Assert.Equal(0f, LookRules.GlassTransparency(LookState.Default with { Glass = 1f }, 0f)); // the setting still wins
        Assert.Equal(0.5f, LookRules.GlassTransparency(LookState.Default with { Glass = float.NaN }, 0.5f), 6);
        Assert.Equal(1f, LookRules.GlassTransparency(LookState.Default with { Glass = 3f }, 1.5f));
    }

    [Fact]
    public void Lyrics_take_the_looks_weight_highlight_and_glow()
    {
        Assert.Equal(700, LookRules.LyricsFontWeight(LookState.Default)); // Bold, as in v0.0.6
        Assert.Equal(400, LookRules.LyricsFontWeight(LookState.Default with { LyricsWeight = LyricsWeight.Regular }));
        Assert.Equal(800, LookRules.LyricsFontWeight(LookState.Default with { LyricsWeight = LyricsWeight.Heavy }));
        Assert.Equal((HighlightInk.Ink, 0u), LookRules.Highlight(LookState.Default));
        Assert.Equal((HighlightInk.Accent, 0u), LookRules.Highlight(LookState.Default with { LyricsHighlight = new LyricsHighlight.Accent() }));
        Assert.Equal((HighlightInk.Custom, 0x12ABEFu), LookRules.Highlight(LookState.Default with { LyricsHighlight = new LyricsHighlight.Custom(0xFF12ABEF) }));

        // v0.0.6 had no glow: the default look draws none although its glow is 0.6.
        Assert.Equal(0, LookRules.LyricsGlow(LookState.Default));
        Assert.Equal(0.6, LookRules.LyricsGlow(LookState.Default with { Customized = true }), 6);
        Assert.Equal(1, LookRules.LyricsGlow(LookState.Default with { Customized = true, LyricsGlow = 7f }));
    }

    [Fact]
    public void Now_playing_is_arranged_by_the_layout_and_the_pane()
    {
        Assert.Equal(NowPlayingArrangement.Stage, LookRules.Arrangement(NowPlayingLayout.Stage, lyricsPane: true));
        Assert.Equal(NowPlayingArrangement.Cover, LookRules.Arrangement(NowPlayingLayout.Artwork, lyricsPane: true));
        Assert.Equal(NowPlayingArrangement.Stage, LookRules.Arrangement(NowPlayingLayout.Artwork, lyricsPane: false)); // the queue keeps the stage
        Assert.Equal(NowPlayingArrangement.WidePane, LookRules.Arrangement(NowPlayingLayout.Lyrics, lyricsPane: true));
        Assert.Equal(NowPlayingArrangement.WidePane, LookRules.Arrangement(NowPlayingLayout.Lyrics, lyricsPane: false));
    }

    [Fact]
    public void The_mini_player_style_picks_the_bar_or_the_capsule()
    {
        Assert.True(LookRules.ShowsPlayerBar(MiniPlayerStyle.Bar, wide: true));
        Assert.False(LookRules.ShowsPlayerBar(MiniPlayerStyle.Bar, wide: false)); // narrow windows: the capsule, as in v0.0.6
        Assert.False(LookRules.ShowsPlayerBar(MiniPlayerStyle.Compact, wide: true));
        Assert.False(LookRules.ShowsPlayerBar(MiniPlayerStyle.Floating, wide: true));
        Assert.True(LookRules.StretchesCapsule(MiniPlayerStyle.Compact));
        Assert.False(LookRules.StretchesCapsule(MiniPlayerStyle.Floating));
    }

    [Fact]
    public void Home_shows_the_sections_the_look_lists_in_its_order()
    {
        using var h = new AppHarness().Started();
        var home = h.App.CreateHome();
        Assert.True(home.ShowsRecentlyAdded && home.ShowsRecentlyPlayed && home.ShowsMostPlayed && home.ShowsSummary);
        var moved = 0;
        home.SectionsChanged += () => moved++;
        home.Activate();
        h.Emit(new CoreEvent.Look(LookState.Default with { Version = 1, Customized = true, Home = [HomeSection.MostPlayed, HomeSection.Summary] }));
        Assert.Equal([HomeSection.MostPlayed, HomeSection.Summary], home.Sections);
        Assert.False(home.ShowsRecentlyAdded);
        Assert.True(home.ShowsMostPlayed);
        Assert.Equal(1, moved);
        Assert.True(h.App.Look.Customized);
    }
}
