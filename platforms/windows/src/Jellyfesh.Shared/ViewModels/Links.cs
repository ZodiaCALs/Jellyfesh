using Jellyfesh.Core;
using Jellyfesh.Formatting;

namespace Jellyfesh.ViewModels;

/// <summary>
/// Item links and remote sources (docs/ui.md, "Links and remote playback"; ADR-0018). An item with
/// a link offers Open in {Service}, the source's name, and an item that cannot play here opens its
/// link as its main action. A remote source's songs play on the person's own devices, so Play
/// Next, Play Last and Add to Playlist are not offered for them. The core decides all of this; these
/// helpers only read its slices.
/// </summary>
public static class ItemLinks
{
    /// <summary>The service's name for Open in {Service}: the source's name, else its id.</summary>
    public static string ServiceName(SourcesState sources, string? plugin) =>
        string.IsNullOrEmpty(plugin) ? string.Empty : sources.Plugins.FirstOrDefault(p => p.Id == plugin)?.Name ?? plugin;

    /// <summary>"Open in QQ Music".</summary>
    public static string OpenInLabel(ILocalizer l, string service) => l.Format("Link_OpenIn", service);

    /// <summary>
    /// "Content from Spotify" for a source whose terms ask for credit (PluginSummary.attribution),
    /// shown in its views; null for every other source. No logos until they are approved.
    /// </summary>
    public static string? Attribution(SourcesState sources, string? plugin, ILocalizer l) =>
        sources.Plugins.FirstOrDefault(p => p.Id == plugin) is { Attribution: not null } source
            ? l.Format("Source_Attribution", source.Name)
            : null;

    /// <summary>
    /// Whether a link can be opened: web addresses only (the core sends https links filled in from the
    /// plugin's templates); anything else a plugin might put there is ignored.
    /// </summary>
    public static bool CanOpen(string? link) =>
        Uri.TryCreate(link, UriKind.Absolute, out var uri) && (uri.Scheme == Uri.UriSchemeHttps || uri.Scheme == Uri.UriSchemeHttp);

    /// <summary>The link when it can be opened, otherwise null.</summary>
    public static string? Usable(string? link) => CanOpen(link) ? link : null;

    /// <summary>Opens the link in the browser (or the service's app); false when there is none to open.</summary>
    public static bool Open(ShellServices s, string? link)
    {
        if (!CanOpen(link))
        {
            return false;
        }

        s.Platform.OpenUrl(link!);
        return true;
    }

    /// <summary>Whether the source plays on the person's own devices (PluginSummary.remote_playback).</summary>
    public static bool IsRemoteSource(SourcesState sources, string? plugin) =>
        !string.IsNullOrEmpty(plugin) && sources.Plugins.Any(p => p.Id == plugin && p.RemotePlayback);

    /// <summary>Whether the song is a remote source's: it starts a remote session and never joins the queue.</summary>
    public static bool IsRemote(SourcesState sources, TrackRef? track) =>
        track is TrackRef.Source s && IsRemoteSource(sources, s.Track.Plugin);
}
