using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Markup;

namespace Jellyfesh.App.Theming;

/// <summary>What a token becomes in XAML.</summary>
public enum TokenKind
{
    Number,
    Corner,
    Thickness,
    Pixels,
}

/// <summary>
/// A Misty token in XAML: <c>CornerRadius="{th:Token Key=Radius.Lg, Kind=Corner}"</c>,
/// <c>FontSize="{th:Token Key=Type.ListTitle}"</c>. Values come from the generated C# tokens
/// (TokenValues), so styles never hard-code a radius, size or spacing.
/// </summary>
[MarkupExtensionReturnType(ReturnType = typeof(object))]
public sealed partial class Token : MarkupExtension
{
    public string Key { get; set; } = string.Empty;

    public TokenKind Kind { get; set; } = TokenKind.Number;

    /// <summary>For Kind=Thickness: which sides (any of L, T, R, B); all when empty.</summary>
    public string Sides { get; set; } = string.Empty;

    protected override object ProvideValue()
    {
        var v = TokenValues.Number(Key);
        return Kind switch
        {
            TokenKind.Corner => new CornerRadius(v),
            TokenKind.Thickness => Sides.Length == 0
                ? new Thickness(v)
                : new Thickness(Has('L') ? v : 0, Has('T') ? v : 0, Has('R') ? v : 0, Has('B') ? v : 0),
            TokenKind.Pixels => new GridLength(v),
            _ => v,
        };
    }

    private bool Has(char side) => Sides.Contains(side, StringComparison.OrdinalIgnoreCase);
}
