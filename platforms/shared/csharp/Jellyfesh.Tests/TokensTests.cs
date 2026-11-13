#if JF_TOKENS
using System.Linq;
using Jellyfesh.Design;
using Xunit;

namespace Jellyfesh.Tests;

/// <summary>The tokens `cargo xtask tokens --target csharp` writes compile and carry the token file's values.</summary>
public class TokensTests
{
    [Fact]
    public void GeneratedTokens()
    {
        Assert.Equal(250.0, Tokens.DefaultHue);
        Assert.Equal(16.0, Tokens.Space.S4);
        Assert.Equal(80, Tokens.Motion.Dur0Ms);
        Assert.Equal("ink.ink-90", Tokens.Ink.Ink90.Name);
        Assert.Equal(new[] { "clear", "mist", "veil" }, Tokens.Glass.Grades.Select(g => g.Name));
        Assert.Equal("glass.tint-mist", Tokens.Glass.TintMist.Name);
        Assert.Equal("elevation.shadow-1.0", Tokens.Elevation.Shadow1.Light[0].Color.Name);
        Assert.Equal(700, Tokens.TypeRoles.NowPlayingTitle.Weight);
    }
}
#endif
