using Jellyfesh.Formatting;

namespace Jellyfesh.Services;

/// <summary>
/// What Settings › About shows (docs/ui.md, "About"; docs/parity.md N28): the version from the
/// build, never typed in, with the core's only when it differs, and the website with the
/// third-party licences it publishes, where the build knows the website. The same rules as the
/// macOS and Android apps' AboutInfo.
/// </summary>
public static class AboutInfo
{
    /// <summary>The page the website publishes the third-party licences on (tools/release/build_site.py).</summary>
    public const string NoticesPage = "THIRD_PARTY_NOTICES.html";

    /// <summary>The website and its licences page.</summary>
    public sealed record Links(Uri Website, Uri Licences);

    /// <summary>
    /// The links for the address the build gave (the JfWebsite assembly metadata), with a slash added at
    /// its end; null without an address, or for one that is not a web address.
    /// </summary>
    public static Links? LinksFor(string? website)
    {
        var site = (website ?? string.Empty).Trim();
        if (!site.StartsWith("https://", StringComparison.OrdinalIgnoreCase) && !site.StartsWith("http://", StringComparison.OrdinalIgnoreCase))
        {
            return null;
        }

        var root = site.EndsWith('/') ? site : site + "/";
        return Uri.TryCreate(root, UriKind.Absolute, out var home) && home.Host.Length > 0 && Uri.TryCreate(home, NoticesPage, out var licences)
            ? new Links(home, licences)
            : null;
    }

    /// <summary>
    /// The version line: "Version 0.0.7", and the core's version after it only when it differs (they
    /// are released together, so normally one number shows). Build metadata after a "+" is left out.
    /// </summary>
    public static string Version(string? app, string? core, ILocalizer l)
    {
        var version = Plain(app);
        var engine = Plain(core);
        if (engine.Length == 0 || engine == version)
        {
            return l.Format("About_VersionOnly", version.Length == 0 ? "?" : version);
        }

        return version.Length == 0 ? l.Format("About_VersionOnly", engine) : l.Format("About_Version", version, engine);
    }

    private static string Plain(string? version)
    {
        var text = (version ?? string.Empty).Trim();
        var plus = text.IndexOf('+', StringComparison.Ordinal);
        return plus >= 0 ? text[..plus] : text;
    }
}
