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
