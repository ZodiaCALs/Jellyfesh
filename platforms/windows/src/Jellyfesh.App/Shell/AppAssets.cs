using Jellyfesh.Appearance;

namespace Jellyfesh.App.Shell;

/// <summary>Files shipped next to the executable (file URIs work packaged and unpackaged).</summary>
internal static class AppAssets
{
    public static string Folder { get; } = Path.Combine(AppContext.BaseDirectory, "Assets");

    /// <summary>One of the ten icons as a PNG (16, 24, 32, 48, 64 or 256 px), for the picker and the title bar.</summary>
    public static Uri IconPng(string id, int size) =>
        new(Path.Combine(Folder, "Icons", AppIcons.IsKnown(id) && id != AppIcons.Automatic ? id : AppIcons.DefaultLight, $"{size}.png"));

    /// <summary>The picker's preview for an icon choice (Automatic shows Ice and Steel together).</summary>
    public static string? IconPreview(string id) =>
        id == AppIcons.Automatic
            ? new Uri(Path.Combine(Folder, "Icons", "automatic-64.png")).AbsoluteUri
            : IconPng(id, 64).AbsoluteUri;
}
