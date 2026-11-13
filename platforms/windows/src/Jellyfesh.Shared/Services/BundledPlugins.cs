namespace Jellyfesh.Services;

/// <summary>
/// The official plugins shipped inside the app (the Subsonic and Jellyfin sources). The build
/// script copies <c>dist/plugins/*.jfpkg</c> into the app's <see cref="FolderName"/> folder; the
/// app passes their paths to the core (Config.bundled_plugins), which installs them on first start
/// and updates them with the app. The same packages are also offered as separate downloads.
/// </summary>
public static class BundledPlugins
{
    public const string FolderName = "Plugins";

    public const string Extension = ".jfpkg";

    /// <summary>The packages in <paramref name="appFolder"/>'s plugin folder, in a stable order; none when it is missing.</summary>
    public static IReadOnlyList<string> Find(string appFolder)
    {
        var folder = Path.Combine(appFolder, FolderName);
        if (!Directory.Exists(folder))
        {
            return [];
        }

        try
        {
            return Directory.EnumerateFiles(folder, "*" + Extension, SearchOption.TopDirectoryOnly)
                .Where(p => p.EndsWith(Extension, StringComparison.OrdinalIgnoreCase))
                .Order(StringComparer.OrdinalIgnoreCase)
                .ToArray();
        }
        catch (Exception e) when (e is IOException or UnauthorizedAccessException)
        {
            return [];
        }
    }
}
