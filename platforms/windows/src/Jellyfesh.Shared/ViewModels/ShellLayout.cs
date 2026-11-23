using Jellyfesh.Core;
using Jellyfesh.Drawing;

namespace Jellyfesh.ViewModels;

/// <summary>
/// The one layout of every app (ADR-0023, docs/ui.md "Navigation"), with Jellyfesh Web's measures
/// (platforms/web/src/App.svelte): the floating sidebar panel from 761 epx wide and the phone tabs
/// below; the player bar from 900 epx (for the look's Bar style) and the capsule below; how far the
/// floating capsules reach up from the window's bottom, so that a page's last row scrolls clear of
/// them and the content fades out just above them; and the content's gutters and title size. Pure:
/// the window gives it its width and draws what it says.
/// </summary>
/// <param name="Sidebar">The floating sidebar panel; otherwise the phone tabs.</param>
/// <param name="Bar">The full player bar; otherwise the capsule.</param>
/// <param name="VolumeLine">The bar's volume line; narrower bars keep only the mute button (Ctrl+Up and Ctrl+Down still work).</param>
/// <param name="Gutter">The content's side padding (layout.gutter-fluid wide, s4 on phones).</param>
/// <param name="Top">The content's top padding below the title bar.</param>
/// <param name="Floor">How far the floating capsules (the player, the phone tabs) reach up from the bottom.</param>
/// <param name="TitleSize">The page titles: type.text-h1 from 900 epx, type.text-h2 below.</param>
public sealed record ShellLayout(bool Sidebar, bool Bar, bool VolumeLine, double Gutter, double Top, double Floor, double TitleSize)
{
    /// <summary>The sidebar panel from this window width (effective pixels); the phone tabs below it.</summary>
    public const double SidebarMinWidth = 761;

    /// <summary>The sidebar's column: the panel, 12 epx (s3) from the window's edge.</summary>
    public const double SidebarColumn = 248;

    /// <summary>The bar keeps its volume line in windows wider than this (the web's 1040 px).</summary>
    public const double VolumeLineMinWidth = 1041;

    /// <summary>space.s3: the panel's distance from the window's edges, and the capsule's above the tabs.</summary>
    public const double S3 = 12;

    /// <summary>space.s4: the player's distance from the bottom, and the phones' gutter.</summary>
    public const double S4 = 16;

    /// <summary>space.s6: the phone tab bar's distance from the bottom.</summary>
    public const double S6 = 24;

    /// <summary>space.s7: the room below a page's last row on wide windows.</summary>
    public const double S7 = 32;

    /// <summary>The phone tab bar's height (the web's 68 px).</summary>
    public const double TabsHeight = 68;

    /// <summary>The phone tabs' and the capsule's side margins (the web's 14 px).</summary>
    public const double PhoneMargin = 14;

    /// <summary>player.mini-height: the capsule.</summary>
    public const double CapsuleHeight = 64;

    /// <summary>edge-blur.bottom-height: the content fades out over this height, just above the floor.</summary>
    public const double Fade = 48;

    /// <summary>layout.gutter-fluid: clamp(18, 4 % of the window, 32).</summary>
    public const double GutterMin = 18;

    /// <inheritdoc cref="GutterMin"/>
    public const double GutterMax = 32;

    /// <summary>type.text-h1, the page titles from 900 epx.</summary>
    public const double TitleWide = 48;

    /// <summary>type.text-h2, the page titles below 900 epx.</summary>
    public const double TitleNarrow = 30.4;

    /// <summary>Before the window has a size: the wide layout without a player.</summary>
    public static ShellLayout Initial { get; } = For(1280, hasTrack: false, MiniPlayerStyle.Bar);

    /// <summary>The layout of a window <paramref name="width"/> epx wide.</summary>
    /// <param name="width">The window's width in effective pixels.</param>
    /// <param name="hasTrack">Something is playing or paused: the player floats at the bottom.</param>
    /// <param name="style">The look's style for the bottom player (ADR-0020).</param>
    public static ShellLayout For(double width, bool hasTrack, MiniPlayerStyle style)
    {
        var sidebar = width >= SidebarMinWidth;
        var bar = LookRules.ShowsPlayerBar(style, SeekGeometry.UsesPlayerBar(width));
        var floor = sidebar
            ? (hasTrack ? SeekGeometry.BarHeight + S4 : 0)
            : TabsHeight + S6 + (hasTrack ? S3 + CapsuleHeight : 0);
        return new ShellLayout(
            sidebar,
            bar,
            VolumeLine: width >= VolumeLineMinWidth,
            Gutter: sidebar ? Math.Clamp(width * 0.04, GutterMin, GutterMax) : S4,
            Top: sidebar ? 24 : S4,
            Floor: floor,
            TitleSize: SeekGeometry.UsesPlayerBar(width) ? TitleWide : TitleNarrow);
    }

    /// <summary>The room below a page's last row: the floor, the fade above it and a margin (s7 wide, s4 on phones).</summary>
    public double BottomPadding => Floor + Fade + (Sidebar ? S7 : S4);

    /// <summary>The fade over the content's bottom: the floor and the fade above it; none without a floor.</summary>
    public double FadeHeight => Floor <= 0 ? 0 : Floor + Fade + 8;

    /// <summary>The bottom player's distance from the window's bottom: above the phone tabs on phones.</summary>
    public double DockBottom => Sidebar ? S4 : TabsHeight + S6 + S3;

    /// <summary>The bottom player's side margins: the content's gutters wide, the tabs' margins on phones.</summary>
    public double DockSide => Sidebar ? Gutter : PhoneMargin;
}
