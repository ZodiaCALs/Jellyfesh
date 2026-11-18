using System.Globalization;
using Jellyfesh.Formatting;
using Jellyfesh.Services;
using Xunit;

namespace Jellyfesh.Tests;

/// <summary>Settings › About: the version from the build and the website's links (docs/parity.md N28).</summary>
public class AboutTests
{
    private static readonly TableLocalizer English = new(new Dictionary<string, string>
    {
        ["About_VersionOnly"] = "Version {0}",
        ["About_Version"] = "Version {0} · core {1}",
    });

    [Fact]
    public void The_website_and_its_licences_page_come_from_the_build()
    {
        var links = AboutInfo.LinksFor(" https://example.github.io/jellyfesh ");
        Assert.NotNull(links);
        Assert.Equal("https://example.github.io/jellyfesh/", links.Website.AbsoluteUri);
        Assert.Equal("https://example.github.io/jellyfesh/THIRD_PARTY_NOTICES.html", links.Licences.AbsoluteUri);
        Assert.Equal("https://jellyfesh.example/THIRD_PARTY_NOTICES.html", AboutInfo.LinksFor("https://jellyfesh.example/")!.Licences.AbsoluteUri);
    }

    [Theory]
    [InlineData(null)]
    [InlineData("")]
    [InlineData("   ")]
    [InlineData("ftp://example.com/")]
    [InlineData("javascript:alert(1)")]
    [InlineData("example.com")]
    public void Without_a_web_address_there_are_no_links(string? website)
    {
        Assert.Null(AboutInfo.LinksFor(website));
    }

    [Theory]
    [InlineData("0.0.7", "0.0.7", "Version 0.0.7")]
    [InlineData("0.0.7+3f2a9c1", "0.0.7", "Version 0.0.7")]
    [InlineData("0.0.7", null, "Version 0.0.7")]
    [InlineData("0.0.7", "0.0.6", "Version 0.0.7 · core 0.0.6")]
    [InlineData(null, "0.0.6", "Version 0.0.6")]
    public void The_version_shows_the_core_only_when_it_differs(string? app, string? core, string expected)
    {
        Assert.Equal(expected, AboutInfo.Version(app, core, English));
    }

    private sealed class TableLocalizer(Dictionary<string, string> table) : ILocalizer
    {
        public CultureInfo Culture => CultureInfo.InvariantCulture;

        public string Get(string key) => table.TryGetValue(key, out var value) ? value : key;
    }
}
