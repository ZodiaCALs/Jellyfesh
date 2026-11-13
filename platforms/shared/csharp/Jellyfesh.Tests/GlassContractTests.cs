using System.Text.Json;
using Jellyfesh.Shared;
using Xunit;

namespace Jellyfesh.Tests;

/// <summary>The glass-alpha port replays tests/contract/glass-alpha.json, recorded from jf_palette.</summary>
public class GlassContractTests
{
    [Fact]
    public void GlassAlphaContract()
    {
        using JsonDocument contract = Contract.Read("glass-alpha.json");
        Assert.Equal(1, contract.RootElement.GetProperty("version").GetInt32());
        double tolerance = contract.RootElement.GetProperty("tolerance").GetDouble();
        int checks = 0;
        foreach (JsonElement testCase in contract.RootElement.GetProperty("cases").EnumerateArray())
        {
            float alpha = testCase[0].GetSingle();
            float transparency = testCase[1].GetSingle();
            double expected = testCase[2].GetDouble();
            float actual = Glass.Alpha(alpha, transparency);
            Assert.True(System.Math.Abs(actual - expected) <= tolerance, $"glass alpha ({alpha}, {transparency}) = {actual}, expected {expected}");
            checks++;
        }
        Assert.True(checks >= 50, $"only {checks} cases");
    }

    [Fact]
    public void GlassAlphaKeepsTheTokensAtTheDefaultAndClampsItsInputs()
    {
        foreach (float a in new[] { 0f, 0.3f, 0.42f, 0.62f, 1f })
        {
            Assert.Equal(a, Glass.Alpha(a, Glass.DefaultTransparency));
        }
        Assert.Equal(1f, Glass.Alpha(0.42f, -1f));
        Assert.Equal(0.42f, Glass.Alpha(0.42f, float.NaN));
        Assert.Equal(0.75f, Glass.Alpha(float.NaN, 0.75f));
        Assert.Equal(1f, Glass.Alpha(1.5f, 0.5f));
    }
}
