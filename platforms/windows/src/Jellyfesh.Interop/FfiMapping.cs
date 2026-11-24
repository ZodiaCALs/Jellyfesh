// Maps the Jellyfesh.Ffi mirror types (core/jf-ffi/csharp, docs/ffi.md) to the shell's model in
// Jellyfesh.Core and back. Every name of the binding the app depends on is in this file and in
// FfiCoreClient.cs, so a renamed binding member is fixed here only.

using System;
using System.Collections.Generic;
using System.Linq;
using F = Jellyfesh.Ffi;
using M = Jellyfesh.Core;

namespace Jellyfesh.Interop;

public static class FfiMapping
{
    // ---- from the core ------------------------------------------------------------------------

    public static M.CoreSnapshot ToModel(F.Snapshot s) => new(
        ToModel(s.Playback),
        ToModel(s.Queue),
        s.NowPlaying is null ? null : ToModel(s.NowPlaying),
        s.Lyrics is null ? null : ToModel(s.Lyrics),
        s.Theme is null ? null : ToModel(s.Theme),
        s.LibraryRevision,
        s.Scanning,
        ToModel(s.Settings),
        ToModel(s.Devices),
        ToModel(s.Sources),
        s.Muted,
        s.Remote is null ? M.RemoteState.Initial : ToModel(s.Remote),
        s.AiLyrics is null ? M.AiLyricsState.Initial : ToModel(s.AiLyrics),
        s.Look is null ? M.LookState.Default : ToModel(s.Look),
        s.Pro is null ? M.ProState.Initial : ToModel(s.Pro),
        s.Update is null ? M.UpdateState.Initial : ToModel(s.Update),
        s.SourceLibrary is null ? M.SourceLibraryState.Initial : ToModel(s.SourceLibrary),
        s.Downloads is null ? M.DownloadsState.Initial : ToModel(s.Downloads));

    public static M.CoreEvent ToModel(F.Event e) => e switch
    {
        F.Event.Playback p => new M.CoreEvent.Playback(ToModel(p.Snapshot)),
        F.Event.Queue q => new M.CoreEvent.Queue(ToModel(q.Snapshot)),
        F.Event.NowPlaying n => new M.CoreEvent.NowPlaying(n.Snapshot is null ? null : ToModel(n.Snapshot)),
        F.Event.Lyrics l => new M.CoreEvent.Lyrics(ToModel(l.Snapshot)),
        F.Event.Theme t => new M.CoreEvent.Theme(ToModel(t.Snapshot)),
        F.Event.Scan s => new M.CoreEvent.Scan(ToModel(s.Status)),
        F.Event.LibraryChanged lc => new M.CoreEvent.LibraryChanged(lc.Revision),
        F.Event.PlaylistCreated pc => new M.CoreEvent.PlaylistCreated(pc.Request, pc.Id),
        F.Event.Settings st => new M.CoreEvent.Settings(ToModel(st.Snapshot)),
        F.Event.Devices d => new M.CoreEvent.Devices(ToModel(d.Snapshot)),
        F.Event.Sources so => new M.CoreEvent.Sources(ToModel(so.Snapshot)),
        F.Event.Remote r => new M.CoreEvent.Remote(ToModel(r.Snapshot)),
        F.Event.Auth a => new M.CoreEvent.Auth(ToModel(a.Update)),
        F.Event.SearchResults sr => new M.CoreEvent.SearchResults(ToModel(sr.Update)),
        F.Event.BrowsePage bp => new M.CoreEvent.BrowsePage(ToModel(bp.Update)),
        F.Event.Buffering b => new M.CoreEvent.Buffering(b.Uid, b.Value),
        F.Event.CommandFailed cf => new M.CoreEvent.CommandFailed(cf.Id, cf.Message),
        F.Event.Error err => new M.CoreEvent.Error(err.Message),
        F.Event.AiLyrics ai => new M.CoreEvent.AiLyrics(ToModel(ai.Snapshot)),
        F.Event.Look lk => new M.CoreEvent.Look(ToModel(lk.Snapshot)),
        F.Event.Pro pr => new M.CoreEvent.Pro(ToModel(pr.Snapshot)),
        F.Event.UpdateStatus up => new M.CoreEvent.UpdateStatus(ToModel(up.Snapshot)),
        F.Event.Discover dp => new M.CoreEvent.Discover(ToModel(dp.Page)),
        F.Event.Details dt => new M.CoreEvent.Details(ToModel(dt.Page)),
        F.Event.Comments cm => new M.CoreEvent.Comments(ToModel(cm.Page)),
        F.Event.Artwork aw => new M.CoreEvent.Artwork(new M.ArtworkReady(aw.Ready.Url, aw.Ready.SizePx, aw.Ready.Path)),
        F.Event.SearchPage sp => new M.CoreEvent.SearchPage(ToModel(sp.Page)),
        F.Event.SourceLibrary sl => new M.CoreEvent.SourceLibrary(ToModel(sl.Snapshot)),
        F.Event.SourceLibraryPage lp => new M.CoreEvent.SourceLibraryPage(ToModel(lp.Page)),
        F.Event.Downloads dl => new M.CoreEvent.Downloads(ToModel(dl.Snapshot)),
        _ => new M.CoreEvent.Error($"unknown core event {e.GetType().Name}"),
    };

    public static M.PlaybackState ToModel(F.PlaybackSnapshot p) => new(
        p.Version, ToModel(p.Status), ToModel(p.Anchor), p.Volume, ToModel(p.Eq), ToModel(p.ReplaygainMode), p.PreampDb);

    public static M.PlaybackAnchor ToModel(F.PlaybackAnchor a) =>
        new(a.TrackGeneration, a.PositionUs, a.AnchorMonoNs, a.Rate, a.DurationUs);

    public static M.PlaybackStatus ToModel(F.Status s) => s switch
    {
        F.Status.Playing => M.PlaybackStatus.Playing,
        F.Status.Paused => M.PlaybackStatus.Paused,
        F.Status.Ended => M.PlaybackStatus.Ended,
        _ => M.PlaybackStatus.Stopped,
    };

    public static M.QueueState ToModel(F.QueueSnapshot q) => new(
        q.Version, q.Len, q.Position, q.Current is null ? null : ToModel(q.Current), q.Shuffle, ToModel(q.Repeat),
        List(q.UpNext, ToModel));

    public static M.QueueItem ToModel(F.QueueItem i) => new(i.Uid, ToModel(i.Track));

    public static M.HistoryEntry ToModel(F.HistoryEntry h) => new(ToModel(h.Track), h.PlayedAtMs);

    public static M.TrackRef ToModel(F.TrackRef t) => t switch
    {
        F.TrackRef.Library l => new M.TrackRef.Library(l.Id),
        F.TrackRef.File f => new M.TrackRef.File(f.Path),
        F.TrackRef.Source s => new M.TrackRef.Source(ToModel(s.Track)),
        _ => throw new ArgumentOutOfRangeException(nameof(t), t, "unknown track reference"),
    };

    public static M.SourceTrack ToModel(F.SourceTrack s) =>
        new(s.Plugin, s.Id, s.Title, s.Artist, s.Album, s.DurationMs, s.ArtworkUrl, s.Isrc);

    public static M.RepeatMode ToModel(F.RepeatMode r) => r switch
    {
        F.RepeatMode.All => M.RepeatMode.All,
        F.RepeatMode.One => M.RepeatMode.One,
        _ => M.RepeatMode.Off,
    };

    public static M.ReplayGainMode ToModel(F.ReplayGainMode r) => r switch
    {
        F.ReplayGainMode.Track => M.ReplayGainMode.Track,
        F.ReplayGainMode.Album => M.ReplayGainMode.Album,
        _ => M.ReplayGainMode.Off,
    };

    public static M.EqSettings ToModel(F.EqSettings e) => new(e.Enabled, e.PreampDb, List(e.Bands, ToModel));

    public static M.EqPreset ToModel(F.EqPreset p) => new(p.Id, ToModel(p.Settings));

    public static M.EqBand ToModel(F.EqBand b) => new(ToModel(b.Kind), b.FreqHz, b.GainDb, b.Q, b.Enabled);

    public static M.EqBandKind ToModel(F.BandKind k) => k switch
    {
        F.BandKind.LowShelf => M.EqBandKind.LowShelf,
        F.BandKind.HighShelf => M.EqBandKind.HighShelf,
        _ => M.EqBandKind.Peaking,
    };

    public static M.NowPlayingInfo ToModel(F.NowPlaying n) => new(
        n.Uid, ToModel(n.Track), n.Path, n.Title, n.Artist, n.Album, n.DurationMs,
        n.Artwork is null ? null : ToModel(n.Artwork),
        n.Format is null ? null : new M.TrackFormat(
            n.Format.Codec, n.Format.SampleRate, n.Format.Channels, n.Format.DurationUs, n.Format.Gapless, n.Format.Bitrate,
            n.Format.BitsPerSample),
        n.Link,
        n.Quality is null ? null : ToModel(n.Quality),
        n.Qualities is null ? null : List(n.Qualities, ToModel),
        n.QualityNote is null ? null : ToModel(n.QualityNote),
        n.QualityLevel is { } level ? ToModel(level) : null);

    public static M.ArtworkRef ToModel(F.ArtworkRef a) => a switch
    {
        F.ArtworkRef.File f => new M.ArtworkRef.File(f.Path),
        F.ArtworkRef.Url u => new M.ArtworkRef.Url(u.Value),
        _ => throw new ArgumentOutOfRangeException(nameof(a), a, "unknown artwork reference"),
    };

    public static M.LyricsState ToModel(F.LyricsSnapshot l) => new(
        l.Uid,
        l.Doc is null ? null : ToModel(l.Doc),
        l.Timeline is null ? null : ToModel(l.Timeline),
        l.Source switch
        {
            F.LyricsSource.Sidecar s => new M.LyricsOrigin.Sidecar(s.Path),
            F.LyricsSource.Embedded => new M.LyricsOrigin.Embedded(),
            F.LyricsSource.Plugin p => new M.LyricsOrigin.Plugin(p.Id),
            F.LyricsSource.Made m => new M.LyricsOrigin.Made(m.PluginId, m.Engine),
            _ => null,
        });

    public static M.LyricDocument ToModel(F.LyricDoc d) => new(
        d.Language,
        d.Timing switch
        {
            F.Timing.Line => M.LyricTiming.Line,
            F.Timing.Word => M.LyricTiming.Word,
            _ => M.LyricTiming.Unsynced,
        },
        d.OffsetMs,
        List(d.Lines, ToModel),
        List(d.Tracks, ToModel),
        d.Source.Format.ToString(),
        d.Source.Origin);

    public static M.LyricLine ToModel(F.Line l) => new(
        l.Id, l.StartMs, l.EndMs, l.Agent, l.Text, List(l.Words, ToModel), List(l.Background, ToModel),
        l.Direction switch
        {
            F.Direction.Ltr => M.TextDirection.Ltr,
            F.Direction.Rtl => M.TextDirection.Rtl,
            _ => M.TextDirection.Auto,
        });

    public static M.LyricWord ToModel(F.Word w) => new(w.StartMs, w.EndMs, w.Text, w.JoinsNext);

    public static M.AuxTrack ToModel(F.AuxTrack t) => new(
        t.Kind == F.AuxKind.Romanization ? M.AuxKind.Romanization : M.AuxKind.Translation,
        t.Language,
        t.Lines.ToDictionary(kv => kv.Key, kv => new M.AuxLine(kv.Value.Text, List(kv.Value.Words, ToModel))));

    public static M.LyricTimelineData ToModel(F.LyricTimeline t) => new(
        t.LineStarts.ToArray(), t.LineEnds.ToArray(), t.SegmentStarts.ToArray(), t.SegmentLines.ToArray(),
        t.WordOffsets.ToArray(), t.WordStarts.ToArray(), t.WordEnds.ToArray());

    public static M.ThemeState ToModel(F.ThemeSnapshot t) => new(
        t.Uid, t.Hue, t.FromArtwork, ToModel(t.Light), ToModel(t.Dark), t.BackdropLight, t.BackdropDark);

    public static M.ThemePalette ToModel(F.Theme t)
    {
        var colors = new Dictionary<string, M.Rgba>(t.Colors.Count, StringComparer.Ordinal);
        foreach (var c in t.Colors)
        {
            colors[c.Name] = M.Rgba.FromPacked(c.Rgba);
        }

        return new M.ThemePalette(t.Hue, t.Appearance == F.Appearance.Dark ? M.Appearance.Dark : M.Appearance.Light, colors);
    }

    public static M.AppSettings ToModel(F.Settings s) => new(
        s.Appearance switch
        {
            F.AppearancePref.Light => M.AppearancePreference.Light,
            F.AppearancePref.Dark => M.AppearancePreference.Dark,
            _ => M.AppearancePreference.System,
        },
        s.Style == F.BackdropStyle.Misty ? M.BackdropStyle.Misty : M.BackdropStyle.Artwork,
        s.AppIcon,
        s.HueSource is F.HueSource.Fixed f ? new M.HueSource.Fixed(f.Hue) : new M.HueSource.Artwork(),
        s.ReduceEffects,
        s.Transparency,
        s.LyricsSize switch
        {
            F.LyricsSize.Small => M.LyricsSize.Small,
            F.LyricsSize.Large => M.LyricsSize.Large,
            _ => M.LyricsSize.Medium,
        },
        s.LyricsAlign == F.LyricsAlign.Center ? M.LyricsAlign.Center : M.LyricsAlign.Start,
        s.ShowTranslation,
        s.ShowRomanization,
        new M.CrossfadeSettings(s.Crossfade.Enabled, s.Crossfade.DurationMs, s.Crossfade.AlbumContinuity),
        ToModel(s.ReplaygainMode),
        s.PreampDb,
        ToModel(s.Eq),
        s.OutputDevice,
        s.ResumeOnLaunch,
        s.WatchFolders,
        s.AllowCommunityPlugins,
        s.CheckForUpdates,
        s.ShowLyricsBar,
        s.DesktopLyrics,
        ToModel(s.StreamingQuality),
        s.Downloads is null ? M.DownloadSettings.Default : ToModel(s.Downloads),
        s.MiniPlayer is null ? M.MiniPlayerSettings.Default : ToModel(s.MiniPlayer));

    public static M.DevicesState ToModel(F.DevicesSnapshot d) => new(d.Version, List(d.Devices, ToModel), d.Selected);

    public static M.DeviceInfo ToModel(F.DeviceInfo d) => new(d.Id, d.Name, d.IsDefault, d.SampleRates.ToArray());

    public static M.SourcesState ToModel(F.SourcesSnapshot s) =>
        new(s.Version, List(s.Plugins, ToModel), List(s.Presets, ToModel), s.AllowCommunity, s.CanInstallPlugins);

    public static M.SourcePreset ToModel(F.SourcePreset p) => new(
        p.Id, p.Name, p.Plugin, List(p.AuthMethods, ToModel), p.NeedsAddress, p.Address, p.AddressExample, p.Account);

    public static M.PluginInfo ToModel(F.PluginSummary p) => new(
        p.Id, p.Name, p.Version, p.Author,
        p.Badge == F.PluginBadge.Official ? M.PluginBadge.Official : M.PluginBadge.Community,
        p.VendorPlayback, p.Capabilities.ToArray(), List(p.AuthMethods, ToModel), p.Enabled, p.Account,
        p.NeedsDisclosure, p.SettingsSchemaJson, p.SettingsJson, p.Icon, p.RemotePlayback,
        string.IsNullOrWhiteSpace(p.Attribution) ? null : p.Attribution,
        p.Kind == F.PluginKind.Extension ? M.PluginKind.Extension : M.PluginKind.Source,
        p.RequiresPro,
        p.Permissions?.ToArray() ?? [],
        p.Discover,
        p.Details,
        p.Comments,
        string.IsNullOrWhiteSpace(p.Description) ? null : p.Description,
        p.Library,
        p.Download,
        p.Qualities is null ? null : List(p.Qualities, ToModel),
        p.LocalNetwork);

    public static M.AuthMethod ToModel(F.AuthMethod a) => a switch
    {
        F.AuthMethod.Qr => M.AuthMethod.Qr,
        F.AuthMethod.Oauth => M.AuthMethod.Oauth,
        F.AuthMethod.CookieImport => M.AuthMethod.CookieImport,
        F.AuthMethod.PasswordExchange => M.AuthMethod.PasswordExchange,
        _ => M.AuthMethod.ApiKey,
    };

    public static M.AuthUpdate ToModel(F.AuthUpdate u) => new(u.Plugin, u.Flow, u.Step switch
    {
        F.AuthStep.ShowQr q => new M.AuthStep.ShowQr(q.Data, q.ExpiresAtMs, q.Hint),
        F.AuthStep.ShowCode c => new M.AuthStep.ShowCode(c.Code, c.Hint, c.ExpiresAtMs),
        F.AuthStep.OpenUrl o => new M.AuthStep.OpenUrl(o.Url),
        F.AuthStep.AskFields a => new M.AuthStep.AskFields(List(a.Fields, ToModel)),
        F.AuthStep.Waiting w => new M.AuthStep.Waiting(w.Message),
        F.AuthStep.Done d => new M.AuthStep.Done(d.Account),
        F.AuthStep.Failed f => new M.AuthStep.Failed(f.Message),
        _ => new M.AuthStep.Failed("unknown sign-in step"),
    });

    public static M.AuthField ToModel(F.AuthField f) => new(
        f.Name,
        f.Label,
        f.Secret,
        f.Placeholder,
        f.Kind switch
        {
            F.AuthFieldKind.Email => M.AuthFieldKind.Email,
            F.AuthFieldKind.Url => M.AuthFieldKind.Url,
            F.AuthFieldKind.Number => M.AuthFieldKind.Number,
            F.AuthFieldKind.CookieHeader => M.AuthFieldKind.CookieHeader,
            _ => M.AuthFieldKind.Text,
        },
        f.Required,
        f.Help);

    public static M.SearchUpdate ToModel(F.SearchUpdate u) =>
        new(u.Request, u.Query, u.Source, List(u.Items, ToModel), u.Error, u.Done, u.Merged,
            u.Groups is null ? null : List(u.Groups, g => new M.SearchGroup(ToModel(g.Kind), List(g.Items, ToModel), g.Next, g.Total)));

    public static M.SearchMorePage ToModel(F.SearchPage p) =>
        new(p.Request, p.Query, p.Source, ToModel(p.Kind), List(p.Items, ToModel), p.Next, p.Error);

    public static M.BrowseUpdate ToModel(F.BrowseUpdate u) =>
        new(u.Request, u.Plugin, u.Node, List(u.Items, ToModel), u.NextPage, u.Error);

    public static M.SourceItem ToModel(F.SourceItem i) => new(
        i.Source,
        ToModel(i.Kind),
        i.Id, i.Title, i.Subtitle, i.Album, i.DurationMs,
        i.Artwork is null ? null : ToModel(i.Artwork),
        i.Playable, i.UnplayableReason,
        i.Track is null ? null : ToModel(i.Track),
        i.AlsoIn.ToArray(),
        i.Link,
        string.IsNullOrEmpty(i.ArtistId) ? null : i.ArtistId,
        string.IsNullOrEmpty(i.AlbumId) ? null : i.AlbumId,
        i.Downloaded);

    public static M.ItemKind ToModel(F.ItemKind k) => k switch
    {
        F.ItemKind.Album => M.ItemKind.Album,
        F.ItemKind.Artist => M.ItemKind.Artist,
        F.ItemKind.Playlist => M.ItemKind.Playlist,
        F.ItemKind.Folder => M.ItemKind.Folder,
        _ => M.ItemKind.Track,
    };

    public static M.RemoteState ToModel(F.RemoteSnapshot r) => new(
        r.Version,
        r.Source,
        r.SourceName,
        ToModel(r.Status),
        r.Item is null ? null : ToModel(r.Item),
        r.Context is null ? null : ToModel(r.Context),
        ToModel(r.Anchor),
        r.Shuffle,
        ToModel(r.Repeat),
        r.Device is null ? null : ToModel(r.Device),
        List(r.Devices, ToModel),
        r.Error,
        r.QualityNote is null ? null : ToModel(r.QualityNote));

    public static M.RemoteDevice ToModel(F.RemoteDevice d) => new(
        d.Id,
        d.Name,
        d.Kind switch
        {
            F.RemoteDeviceKind.Computer => M.RemoteDeviceKind.Computer,
            F.RemoteDeviceKind.Smartphone => M.RemoteDeviceKind.Smartphone,
            F.RemoteDeviceKind.Speaker => M.RemoteDeviceKind.Speaker,
            F.RemoteDeviceKind.Tv => M.RemoteDeviceKind.Tv,
            F.RemoteDeviceKind.Car => M.RemoteDeviceKind.Car,
            _ => M.RemoteDeviceKind.Other,
        },
        d.Active,
        d.VolumePercent);

    public static M.ScanStatus ToModel(F.ScanStatus s) => s switch
    {
        F.ScanStatus.Progress p => new M.ScanStatus.Progress(new M.ScanProgress(p.Value.FilesSeen, p.Value.FilesChanged, p.Value.Current)),
        F.ScanStatus.Finished f => new M.ScanStatus.Finished(new M.ScanReport(
            f.Report.Added, f.Report.Updated, f.Report.Removed, f.Report.Unchanged,
            List(f.Report.Errors, e => new M.ScanError(e.Path, e.Message)))),
        F.ScanStatus.Failed x => new M.ScanStatus.Failed(x.Message),
        _ => new M.ScanStatus.Failed("unknown scan status"),
    };

    public static M.Track ToModel(F.Track t) => new(
        t.Id, t.Path, t.Title, t.Artist, t.Album, t.AlbumArtist, t.AlbumId, t.TrackNo, t.DiscNo, t.Year, t.Genre,
        t.DurationMs, t.Codec, t.ArtworkHash);

    public static M.Album ToModel(F.Album a) => new(a.Id, a.Title, a.AlbumArtist, a.Year, a.TrackCount, a.ArtworkHash);

    public static M.Artist ToModel(F.Artist a) => new(a.Id, a.Name, a.TrackCount);

    public static M.TrackRow ToModel(F.TrackRow r) => new(
        ToModel(r.Track), r.Title, r.Artist, r.Album, r.DurationMs,
        r.Artwork is null ? null : ToModel(r.Artwork), r.ArtworkHash, r.Favorite, r.Missing, r.Downloaded);

    public static M.PlaylistSummary ToModel(F.PlaylistSummary p) =>
        new(p.Id, p.Name, p.EntryCount, p.DurationMs, p.UpdatedAtMs, p.ArtworkHash);

    public static M.PlaylistEntry ToModel(F.PlaylistEntry e) => new(e.EntryId, e.Position, ToModel(e.Row));

    public static M.SearchResults ToModel(F.SearchResults r) =>
        new(List(r.Tracks, ToModel), List(r.Albums, ToModel), List(r.Artists, ToModel));

    public static M.LibraryStats ToModel(F.LibraryStats s) =>
        new(s.Roots, s.Tracks, s.Albums, s.Artists, s.Artwork, s.TotalDurationMs, s.TotalBytes);

    // ---- v0.0.7: AI Lyrics, the look, Pro, updates ---------------------------------------------

    public static M.AiLyricsState ToModel(F.AiLyricsSnapshot a) => new(
        a.Version,
        a.Available,
        a.Plugin,
        a.Plan is null ? null : new M.AiLyricsPlan(
            a.Plan.Mode == F.AiLyricsMode.Online ? M.AiLyricsMode.Online : M.AiLyricsMode.OnDevice,
            a.Plan.Engine, a.Plan.UploadTo, a.Plan.Automatic),
        a.Need switch
        {
            F.AiLyricsNeed.Nothing => new M.AiLyricsNeed.Nothing(),
            F.AiLyricsNeed.Pro => new M.AiLyricsNeed.Pro(),
            F.AiLyricsNeed.SignIn => new M.AiLyricsNeed.SignIn(),
            F.AiLyricsNeed.Consent c => new M.AiLyricsNeed.Consent(c.Origin),
            F.AiLyricsNeed.Model m => new M.AiLyricsNeed.Model(m.Id, m.Name, m.Bytes),
            F.AiLyricsNeed.Unavailable u => new M.AiLyricsNeed.Unavailable(u.Reason),
            _ => new M.AiLyricsNeed.Unavailable(string.Empty),
        },
        List(a.Jobs, ToModel),
        List(a.Models, m => new M.SpeechModel(m.Id, m.Name, m.Bytes, m.State switch
        {
            F.SpeechModelStatus.Downloading d => new M.SpeechModelStatus.Downloading(d.Fraction),
            F.SpeechModelStatus.Ready => new M.SpeechModelStatus.Ready(),
            F.SpeechModelStatus.Failed f => new M.SpeechModelStatus.Failed(f.Message),
            _ => new M.SpeechModelStatus.NotDownloaded(),
        })),
        List(a.Consents, c => new M.AudioConsent(c.Plugin, c.Origin)));

    public static M.LyricsJob ToModel(F.LyricsJob j) => new(j.Id, j.Title, j.Artist, j.Uid, j.LibraryId, j.Stage switch
    {
        F.LyricsJobStage.ReadingAudio => new M.LyricsJobStage.ReadingAudio(),
        F.LyricsJobStage.Sending s => new M.LyricsJobStage.Sending(s.Origin),
        F.LyricsJobStage.Recognising r => new M.LyricsJobStage.Recognising(r.Fraction),
        F.LyricsJobStage.TimingWords => new M.LyricsJobStage.TimingWords(),
        F.LyricsJobStage.Done => new M.LyricsJobStage.Done(),
        F.LyricsJobStage.Failed f => new M.LyricsJobStage.Failed(f.Message),
        F.LyricsJobStage.Cancelled => new M.LyricsJobStage.Cancelled(),
        _ => new M.LyricsJobStage.Waiting(),
    });

    public static M.LookState ToModel(F.LookSnapshot l) => new(
        l.Version,
        l.CornerScale,
        l.Glass,
        l.Aurora,
        l.Typeface switch
        {
            F.Typeface.Rounded => M.Typeface.Rounded,
            F.Typeface.Serif => M.Typeface.Serif,
            F.Typeface.Mono => M.Typeface.Mono,
            _ => M.Typeface.System,
        },
        l.TextScale,
        l.Density switch
        {
            F.Density.Compact => M.Density.Compact,
            F.Density.Roomy => M.Density.Roomy,
            _ => M.Density.Standard,
        },
        l.LyricsWeight switch
        {
            F.LyricsWeight.Regular => M.LyricsWeight.Regular,
            F.LyricsWeight.Medium => M.LyricsWeight.Medium,
            F.LyricsWeight.Semibold => M.LyricsWeight.Semibold,
            F.LyricsWeight.Heavy => M.LyricsWeight.Heavy,
            _ => M.LyricsWeight.Bold,
        },
        l.LyricsGlow,
        l.LyricsHighlight switch
        {
            F.LyricsHighlight.Accent => new M.LyricsHighlight.Accent(),
            F.LyricsHighlight.Custom c => new M.LyricsHighlight.Custom(c.Rgb & 0xFF_FFFF),
            _ => new M.LyricsHighlight.Artwork(),
        },
        List(l.Sidebar, p => p switch
        {
            F.Place.Albums => M.Place.Albums,
            F.Place.Artists => M.Place.Artists,
            F.Place.Songs => M.Place.Songs,
            F.Place.Playlists => M.Place.Playlists,
            F.Place.Favourites => M.Place.Favourites,
            F.Place.Search => M.Place.Search,
            F.Place.Sources => M.Place.Sources,
            _ => M.Place.Home,
        }),
        List(l.Home, h => h switch
        {
            F.HomeSection.RecentlyAdded => M.HomeSection.RecentlyAdded,
            F.HomeSection.RecentlyPlayed => M.HomeSection.RecentlyPlayed,
            F.HomeSection.MostPlayed => M.HomeSection.MostPlayed,
            F.HomeSection.Favourites => M.HomeSection.Favourites,
            _ => M.HomeSection.Summary,
        }),
        l.NowPlaying switch
        {
            F.NowPlayingLayout.Artwork => M.NowPlayingLayout.Artwork,
            F.NowPlayingLayout.Lyrics => M.NowPlayingLayout.Lyrics,
            _ => M.NowPlayingLayout.Stage,
        },
        l.MiniPlayer switch
        {
            F.MiniPlayerStyle.Compact => M.MiniPlayerStyle.Compact,
            F.MiniPlayerStyle.Floating => M.MiniPlayerStyle.Floating,
            _ => M.MiniPlayerStyle.Bar,
        },
        l.Customized,
        l.Plugin);

    public static M.ProState ToModel(F.ProSnapshot p) => new(
        p.Version,
        p.Active,
        p.Source switch
        {
            F.ProSource.Preview => new M.ProSource.Preview(),
            F.ProSource.Store s => new M.ProSource.Purchased(ToModel(s.Value)),
            F.ProSource.Licence l => new M.ProSource.Licence(l.IdSuffix, l.IssuedAtMs),
            _ => new M.ProSource.None(),
        },
        p.Preview,
        p.LicenceError switch
        {
            null => null,
            F.LicenceError.Malformed => M.LicenceError.Malformed,
            F.LicenceError.UnknownKey => M.LicenceError.UnknownKey,
            F.LicenceError.BadSignature => M.LicenceError.BadSignature,
            F.LicenceError.WrongProduct => M.LicenceError.WrongProduct,
            _ => M.LicenceError.Revoked,
        },
        List(p.Stores, ToModel),
        p.LicenceKeys);

    public static M.ProStore ToModel(F.Store s) => s switch
    {
        F.Store.AppStore => M.ProStore.AppStore,
        F.Store.GooglePlay => M.ProStore.GooglePlay,
        _ => M.ProStore.MicrosoftStore,
    };

    public static M.UpdateState ToModel(F.UpdateSnapshot u) => new(
        u.Version, u.Supported, u.Checking,
        u.Available is null ? null : new M.UpdateInfo(u.Available.Version, u.Available.Url),
        u.LastCheckedMs, u.Auto, u.Error);

    // ---- v0.0.7: Discover, details, comments ------------------------------------------------

    public static M.DiscoverPage ToModel(F.DiscoverPage d) => new(
        d.Request, d.Source,
        List(d.Shelves, s => new M.Shelf(
            s.Id, s.Title,
            s.Style switch
            {
                F.ShelfStyle.Grid => M.ShelfStyle.Grid,
                F.ShelfStyle.Chart => M.ShelfStyle.Chart,
                _ => M.ShelfStyle.Row,
            },
            List(s.Items, ToModel), s.More)),
        d.Error);

    public static M.DetailsPage ToModel(F.DetailsPage d) => new(
        d.Request, ToModel(d.Item), d.Title, d.Subtitle, d.Description,
        d.Image is null ? null : ToModel(d.Image),
        List(d.Facts, ToModel),
        d.Technical is null ? null : new M.TechnicalInfo(
            ToModel(d.Technical.Format), d.Technical.Container, d.Technical.FileSizeBytes, d.Technical.DurationMs,
            d.Technical.Replaygain is null ? null : new M.ReplayGainValues(
                d.Technical.Replaygain.TrackGainDb, d.Technical.Replaygain.TrackPeak,
                d.Technical.Replaygain.AlbumGainDb, d.Technical.Replaygain.AlbumPeak),
            d.Technical.Path),
        d.CommentCount, d.Attribution, d.Error);

    public static M.Fact ToModel(F.Fact f) => f.Key switch
    {
        F.FactKey.Version => new M.Fact(M.FactKind.Version, f.Value),
        F.FactKey.Genre => new M.Fact(M.FactKind.Genre, f.Value),
        F.FactKey.Language => new M.Fact(M.FactKind.Language, f.Value),
        F.FactKey.Released => new M.Fact(M.FactKind.Released, f.Value),
        F.FactKey.Label => new M.Fact(M.FactKind.Label, f.Value),
        F.FactKey.Composer => new M.Fact(M.FactKind.Composer, f.Value),
        F.FactKey.Lyricist => new M.Fact(M.FactKind.Lyricist, f.Value),
        F.FactKey.Arranger => new M.Fact(M.FactKind.Arranger, f.Value),
        F.FactKey.Producer => new M.Fact(M.FactKind.Producer, f.Value),
        F.FactKey.Popularity => new M.Fact(M.FactKind.Popularity, f.Value),
        F.FactKey.Quality => new M.Fact(M.FactKind.Quality, f.Value),
        F.FactKey.Isrc => new M.Fact(M.FactKind.Isrc, f.Value),
        F.FactKey.Disc => new M.Fact(M.FactKind.Disc, f.Value),
        F.FactKey.Track => new M.Fact(M.FactKind.Track, f.Value),
        F.FactKey.Other o => new M.Fact(M.FactKind.Other, f.Value, o.Text),
        _ => new M.Fact(M.FactKind.Other, f.Value, string.Empty),
    };

    public static M.AudioFormat ToModel(F.AudioFormat a) => new(
        a.Codec, a.SampleRateHz, a.BitDepth, a.Channels, a.BitrateKbps, a.Lossless,
        a.Quality switch
        {
            null => null,
            F.QualityLabel.HiResLossless => M.QualityLabel.HiResLossless,
            F.QualityLabel.Lossless => M.QualityLabel.Lossless,
            F.QualityLabel.High => M.QualityLabel.High,
            _ => M.QualityLabel.Standard,
        });

    public static M.CommentsPage ToModel(F.CommentsPage c) => new(
        c.Request, ToModel(c.Item), List(c.Hot, ToModel), List(c.Comments, ToModel), c.Total, c.Next, c.Attribution, c.Error);

    public static M.CommentEntry ToModel(F.CommentEntry c) =>
        new(c.Id, c.Author, c.Avatar is null ? null : ToModel(c.Avatar), c.Text, c.Likes, c.TimeMs, c.Replies);

    public static M.ItemRef ToModel(F.ItemRef i) => i switch
    {
        F.ItemRef.Track t => new M.ItemRef.Track(t.Id),
        F.ItemRef.Album a => new M.ItemRef.Album(a.Id),
        F.ItemRef.Artist a => new M.ItemRef.Artist(a.Id),
        F.ItemRef.File f => new M.ItemRef.File(f.Path),
        F.ItemRef.Source s => new M.ItemRef.Source(s.Plugin, ToModel(s.ItemKind), s.Id),
        _ => throw new ArgumentOutOfRangeException(nameof(i), i, "unknown item reference"),
    };

    // ---- v0.0.8 (ADR-0024 to ADR-0026) -----------------------------------------------------------

    public static M.StreamingQuality ToModel(F.StreamingQuality q) => q switch
    {
        F.StreamingQuality.Lossless => M.StreamingQuality.Lossless,
        F.StreamingQuality.High => M.StreamingQuality.High,
        F.StreamingQuality.Normal => M.StreamingQuality.Normal,
        F.StreamingQuality.Low => M.StreamingQuality.Low,
        _ => M.StreamingQuality.Auto,
    };

    public static M.DownloadQuality ToModel(F.DownloadQuality q) => q switch
    {
        F.DownloadQuality.High => M.DownloadQuality.High,
        F.DownloadQuality.Normal => M.DownloadQuality.Normal,
        F.DownloadQuality.Low => M.DownloadQuality.Low,
        _ => M.DownloadQuality.Original,
    };

    public static M.DownloadSettings ToModel(F.DownloadSettings d) => new(
        d.Folder, ToModel(d.Quality), d.NameTemplate, d.AddMissingCoverAndLyrics, d.LyricsInTags, d.SaveLrcFiles, d.WifiOnly, d.AtOnce);

    public static M.MiniPlayerSettings ToModel(F.MiniPlayerSettings m) =>
        new(m.KeepOnTop, m.OpenWhenWindowClosed, m.ShowLyricsLine, m.AndroidPip);

    public static M.QualityNote ToModel(F.QualityNote n) => n switch
    {
        F.QualityNote.SetInService s => new M.QualityNote.SetInService(s.Service),
        _ => new M.QualityNote.FilesPlayAsTheyAre(),
    };

    public static M.SourceLibraryPart ToModel(F.SourceLibraryPart p) => p switch
    {
        F.SourceLibraryPart.LikedAlbums => M.SourceLibraryPart.LikedAlbums,
        F.SourceLibraryPart.LikedArtists => M.SourceLibraryPart.LikedArtists,
        F.SourceLibraryPart.Playlists => M.SourceLibraryPart.Playlists,
        _ => M.SourceLibraryPart.LikedSongs,
    };

    public static M.SourceLibraryEntry ToModel(F.SourceLibraryEntry e) => new(ToModel(e.Item), e.Owned, e.SongsListed);

    public static M.SourceLibraryState ToModel(F.SourceLibrarySnapshot s) => new(
        s.Version,
        List(s.Sources, source => new M.SourceLibrary(
            source.Plugin,
            source.Name,
            string.IsNullOrWhiteSpace(source.Attribution) ? null : source.Attribution,
            List(source.Lists, l => new M.SourceLibraryList(ToModel(l.Part), List(l.Entries, ToModel), l.Total, l.Next, l.RefreshedAtMs, l.Loading, l.Error)))));

    public static M.SourceLibraryMorePage ToModel(F.SourceLibraryPage p) =>
        new(p.Request, p.Plugin, ToModel(p.Part), List(p.Entries, ToModel), p.Next, p.Error);

    public static M.DownloadsState ToModel(F.DownloadsSnapshot d) => new(
        d.Version,
        d.Folder,
        List(d.Items, i => new M.DownloadItem(i.Id, ToModel(i.Track), ToModel(i.State), i.BytesDone, i.BytesTotal, i.Path, i.LibraryId, i.Original)),
        d.BytesUsed,
        d.Paused switch
        {
            F.DownloadsPaused.Offline => M.DownloadsPaused.Offline,
            F.DownloadsPaused.Metered => M.DownloadsPaused.Metered,
            _ => null,
        });

    public static M.DownloadState ToModel(F.DownloadState s) => s switch
    {
        F.DownloadState.Downloading => new M.DownloadState.Downloading(),
        F.DownloadState.Done => new M.DownloadState.Done(),
        F.DownloadState.Failed f => new M.DownloadState.Failed(f.Error switch
        {
            F.DownloadError.NotAllowed n => new M.DownloadError.NotAllowed(n.Service),
            F.DownloadError.NotOffered n => new M.DownloadError.NotOffered(n.Service),
            F.DownloadError.Network n => new M.DownloadError.Network(n.Message),
            F.DownloadError.Storage n => new M.DownloadError.Storage(n.Message),
            F.DownloadError.Other o => new M.DownloadError.Other(o.Message),
            _ => new M.DownloadError.Other(string.Empty),
        }),
        _ => new M.DownloadState.Queued(),
    };

    public static F.StreamingQuality ToFfi(M.StreamingQuality q) => q switch
    {
        M.StreamingQuality.Lossless => F.StreamingQuality.Lossless,
        M.StreamingQuality.High => F.StreamingQuality.High,
        M.StreamingQuality.Normal => F.StreamingQuality.Normal,
        M.StreamingQuality.Low => F.StreamingQuality.Low,
        _ => F.StreamingQuality.Auto,
    };

    public static F.DownloadQuality ToFfi(M.DownloadQuality q) => q switch
    {
        M.DownloadQuality.High => F.DownloadQuality.High,
        M.DownloadQuality.Normal => F.DownloadQuality.Normal,
        M.DownloadQuality.Low => F.DownloadQuality.Low,
        _ => F.DownloadQuality.Original,
    };

    public static F.DownloadSettings ToFfi(M.DownloadSettings d) => new(
        d.Folder, ToFfi(d.Quality), d.NameTemplate, d.AddMissingCoverAndLyrics, d.LyricsInTags, d.SaveLrcFiles, d.WifiOnly,
        Math.Clamp(d.AtOnce, M.DownloadSettings.MinAtOnce, M.DownloadSettings.MaxAtOnce));

    public static F.MiniPlayerSettings ToFfi(M.MiniPlayerSettings m) =>
        new(m.KeepOnTop, m.OpenWhenWindowClosed, m.ShowLyricsLine, m.AndroidPip);

    public static F.SourceLibraryPart ToFfi(M.SourceLibraryPart p) => p switch
    {
        M.SourceLibraryPart.LikedAlbums => F.SourceLibraryPart.LikedAlbums,
        M.SourceLibraryPart.LikedArtists => F.SourceLibraryPart.LikedArtists,
        M.SourceLibraryPart.Playlists => F.SourceLibraryPart.Playlists,
        _ => F.SourceLibraryPart.LikedSongs,
    };

    public static F.DownloadTarget ToFfi(M.DownloadTarget t) => t switch
    {
        M.DownloadTarget.Track track => new F.DownloadTarget.Track(ToFfi(track.Value)),
        M.DownloadTarget.Collection c => new F.DownloadTarget.Collection(c.Plugin, c.Id, ToFfi(c.Kind)),
        _ => throw new ArgumentOutOfRangeException(nameof(t), t, "unknown download target"),
    };

    // ---- to the core --------------------------------------------------------------------------

    public static F.Command ToFfi(M.CoreCommand c) => c switch
    {
        M.CoreCommand.Play => new F.Command.Play(),
        M.CoreCommand.Pause => new F.Command.Pause(),
        M.CoreCommand.TogglePlayPause => new F.Command.TogglePlayPause(),
        M.CoreCommand.Stop => new F.Command.Stop(),
        M.CoreCommand.Next => new F.Command.Next(),
        M.CoreCommand.Previous => new F.Command.Previous(),
        M.CoreCommand.Seek s => new F.Command.Seek(s.PositionMs),
        M.CoreCommand.SetVolume v => new F.Command.SetVolume(Math.Clamp(v.Volume, 0f, 1f)),
        M.CoreCommand.SetMuted m => new F.Command.SetMuted(m.Muted),
        M.CoreCommand.PlayTracks p => new F.Command.PlayTracks(List(p.Tracks, ToFfi), p.Start),
        M.CoreCommand.Enqueue e => new F.Command.Enqueue(List(e.Tracks, ToFfi), e.PlayNext),
        M.CoreCommand.PlayCollection p => new F.Command.PlayCollection(ToFfi(p.Collection), p.Start, p.Shuffle),
        M.CoreCommand.EnqueueCollection e => new F.Command.EnqueueCollection(ToFfi(e.Collection), e.PlayNext),
        M.CoreCommand.RemoveFromQueue r => new F.Command.RemoveFromQueue(r.Uid),
        M.CoreCommand.MoveInQueue m => new F.Command.MoveInQueue(m.Uid, m.To),
        M.CoreCommand.JumpTo j => new F.Command.JumpTo(j.Uid),
        M.CoreCommand.ClearQueue => new F.Command.ClearQueue(),
        M.CoreCommand.SetShuffle s => new F.Command.SetShuffle(s.On),
        M.CoreCommand.SetRepeat r => new F.Command.SetRepeat(ToFfi(r.Mode)),
        M.CoreCommand.SetSettings s => new F.Command.SetSettings(ToFfi(s.Settings)),
        M.CoreCommand.SetEq e => new F.Command.SetEq(ToFfi(e.Eq)),
        M.CoreCommand.SetReplayGain r => new F.Command.SetReplayGain(ToFfi(r.Mode), r.PreampDb),
        M.CoreCommand.SetCrossfade x => new F.Command.SetCrossfade(ToFfi(x.Crossfade)),
        M.CoreCommand.SetOutputDevice d => new F.Command.SetOutputDevice(d.Name),
        M.CoreCommand.RefreshDevices => new F.Command.RefreshDevices(),
        M.CoreCommand.AddRoot a => new F.Command.AddRoot(a.Path),
        M.CoreCommand.RemoveRoot r => new F.Command.RemoveRoot(r.Path),
        M.CoreCommand.Rescan => new F.Command.Rescan(),
        M.CoreCommand.CreatePlaylist p => new F.Command.CreatePlaylist(p.Name, List(p.Tracks, ToFfi)),
        M.CoreCommand.RenamePlaylist r => new F.Command.RenamePlaylist(r.Id, r.Name),
        M.CoreCommand.DeletePlaylist d => new F.Command.DeletePlaylist(d.Id),
        M.CoreCommand.AddToPlaylist a => new F.Command.AddToPlaylist(a.Id, List(a.Tracks, ToFfi)),
        M.CoreCommand.RemoveFromPlaylist r => new F.Command.RemoveFromPlaylist(r.Id, r.Entries.ToArray()),
        M.CoreCommand.MovePlaylistEntry m => new F.Command.MovePlaylistEntry(m.Id, m.Entry, m.To),
        M.CoreCommand.ImportPlaylist i => new F.Command.ImportPlaylist(i.Path),
        M.CoreCommand.SetFavorite f => new F.Command.SetFavorite(ToFfi(f.Track), f.Favorite),
        M.CoreCommand.InstallPlugin i => new F.Command.InstallPlugin(i.Path),
        M.CoreCommand.UninstallPlugin u => new F.Command.UninstallPlugin(u.Id),
        M.CoreCommand.SetPluginEnabled p => new F.Command.SetPluginEnabled(p.Id, p.Enabled),
        M.CoreCommand.SetPluginSettings p => new F.Command.SetPluginSettings(p.Id, p.SettingsJson),
        M.CoreCommand.AcknowledgeUnofficial a => new F.Command.AcknowledgeUnofficial(a.Id),
        M.CoreCommand.BeginLogin b => new F.Command.BeginLogin(b.Plugin, ToFfi(b.Method)),
        M.CoreCommand.ConnectSource cs => new F.Command.ConnectSource(cs.Preset, cs.Address, ToFfi(cs.Method)),
        M.CoreCommand.SubmitLogin s => new F.Command.SubmitLogin(
            s.Plugin, s.Flow, s.Fields.GroupBy(kv => kv.Key).ToDictionary(g => g.Key, g => g.Last().Value)),
        M.CoreCommand.CancelLogin x => new F.Command.CancelLogin(x.Plugin, x.Flow),
        M.CoreCommand.Logout l => new F.Command.Logout(l.Plugin),
        M.CoreCommand.Search s => new F.Command.Search(s.Query, ToFfi(s.Scope)),
        M.CoreCommand.Browse b => new F.Command.Browse(b.Plugin, b.Node, b.Page),
        M.CoreCommand.Remote r => new F.Command.Remote(ToFfi(r.Action)),
        M.CoreCommand.ResetPluginSettings r => new F.Command.ResetPluginSettings(r.Plugin),
        M.CoreCommand.MakeLyrics m => new F.Command.MakeLyrics(ToFfi(m.Song)),
        M.CoreCommand.CancelLyrics x => new F.Command.CancelLyrics(x.Job),
        M.CoreCommand.DeleteMadeLyrics d => new F.Command.DeleteMadeLyrics(ToFfi(d.Song)),
        M.CoreCommand.AllowAudioUpload a => new F.Command.AllowAudioUpload(a.Plugin, a.Origin),
        M.CoreCommand.RevokeAudioUpload r => new F.Command.RevokeAudioUpload(r.Plugin, r.Origin),
        M.CoreCommand.DownloadSpeechModel d => new F.Command.DownloadSpeechModel(d.Id),
        M.CoreCommand.CancelSpeechModelDownload x => new F.Command.CancelSpeechModelDownload(x.Id),
        M.CoreCommand.DeleteSpeechModel d => new F.Command.DeleteSpeechModel(d.Id),
        M.CoreCommand.ActivateLicence a => new F.Command.ActivateLicence(a.Key),
        M.CoreCommand.RemoveLicence => new F.Command.RemoveLicence(),
        M.CoreCommand.CheckForUpdates => new F.Command.CheckForUpdates(),
        M.CoreCommand.Discover d => new F.Command.Discover(d.Source),
        M.CoreCommand.Details d => new F.Command.Details(ToFfi(d.Item)),
        M.CoreCommand.Comments x => new F.Command.Comments(ToFfi(x.Item), x.Page),
        M.CoreCommand.SetNetworkState n => new F.Command.SetNetworkState(n.Online, n.Metered),
        M.CoreCommand.SearchMore m => new F.Command.SearchMore(m.Plugin, ToFfi(m.Kind), m.Cursor),
        M.CoreCommand.RefreshSourceLibrary r => new F.Command.RefreshSourceLibrary(r.Plugin),
        M.CoreCommand.SourceLibraryMore m => new F.Command.SourceLibraryMore(m.Plugin, ToFfi(m.Part), m.Cursor),
        M.CoreCommand.SavePlaylistCopy s => new F.Command.SavePlaylistCopy(s.Plugin, s.Id, s.Name),
        M.CoreCommand.Download d => new F.Command.Download(ToFfi(d.Target)),
        M.CoreCommand.CancelDownload x => new F.Command.CancelDownload(x.Id),
        M.CoreCommand.RetryDownload r => new F.Command.RetryDownload(r.Id),
        M.CoreCommand.RemoveDownload r => new F.Command.RemoveDownload(r.Id),
        M.CoreCommand.RemoveAllDownloads => new F.Command.RemoveAllDownloads(),
        _ => throw new ArgumentOutOfRangeException(nameof(c), c, "unknown command"),
    };

    public static F.RemoteAction ToFfi(M.RemoteAction a) => a switch
    {
        M.RemoteAction.Refresh => new F.RemoteAction.Refresh(),
        M.RemoteAction.Transfer t => new F.RemoteAction.Transfer(t.Device),
        M.RemoteAction.SetVolume v => new F.RemoteAction.SetVolume(Math.Min(v.Percent, 100u)),
        M.RemoteAction.End => new F.RemoteAction.End(),
        _ => throw new ArgumentOutOfRangeException(nameof(a), a, "unknown remote action"),
    };

    public static F.SongRef ToFfi(M.SongRef s) => s switch
    {
        M.SongRef.Library l => new F.SongRef.Library(l.Id),
        _ => new F.SongRef.Current(),
    };

    public static F.ItemRef ToFfi(M.ItemRef i) => i switch
    {
        M.ItemRef.Track t => new F.ItemRef.Track(t.Id),
        M.ItemRef.Album a => new F.ItemRef.Album(a.Id),
        M.ItemRef.Artist a => new F.ItemRef.Artist(a.Id),
        M.ItemRef.File f => new F.ItemRef.File(f.Path),
        M.ItemRef.Source s => new F.ItemRef.Source(s.Plugin, ToFfi(s.Kind), s.Id),
        _ => throw new ArgumentOutOfRangeException(nameof(i), i, "unknown item reference"),
    };

    public static F.ItemKind ToFfi(M.ItemKind k) => k switch
    {
        M.ItemKind.Album => F.ItemKind.Album,
        M.ItemKind.Artist => F.ItemKind.Artist,
        M.ItemKind.Playlist => F.ItemKind.Playlist,
        M.ItemKind.Folder => F.ItemKind.Folder,
        _ => F.ItemKind.Track,
    };

    public static F.StoreEntitlement ToFfi(M.StoreEntitlement e) => new(
        e.Store switch
        {
            M.ProStore.AppStore => F.Store.AppStore,
            M.ProStore.GooglePlay => F.Store.GooglePlay,
            _ => F.Store.MicrosoftStore,
        },
        e.Product, e.Active, e.Transaction);

    public static F.TrackRef ToFfi(M.TrackRef t) => t switch
    {
        M.TrackRef.Library l => new F.TrackRef.Library(l.Id),
        M.TrackRef.File f => new F.TrackRef.File(f.Path),
        M.TrackRef.Source s => new F.TrackRef.Source(new F.SourceTrack(
            s.Track.Plugin, s.Track.Id, s.Track.Title, s.Track.Artist, s.Track.Album, s.Track.DurationMs,
            s.Track.ArtworkUrl, s.Track.Isrc)),
        _ => throw new ArgumentOutOfRangeException(nameof(t), t, "unknown track reference"),
    };

    public static F.Collection ToFfi(M.Collection c) => c switch
    {
        M.Collection.Album a => new F.Collection.Album(a.Id),
        M.Collection.Artist a => new F.Collection.Artist(a.Id),
        M.Collection.Playlist p => new F.Collection.Playlist(p.Id),
        M.Collection.Favorites => new F.Collection.Favorites(),
        M.Collection.Songs s => new F.Collection.Songs(ToFfi(s.Sort), s.Descending),
        M.Collection.Tracks t => new F.Collection.Tracks(List(t.Items, ToFfi)),
        M.Collection.Source s => new F.Collection.Source(s.Plugin, s.Id, ToFfi(s.Kind)),
        _ => throw new ArgumentOutOfRangeException(nameof(c), c, "unknown collection"),
    };

    public static F.SearchScope ToFfi(M.SearchScope s) => s switch
    {
        M.SearchScope.Library => new F.SearchScope.Library(),
        M.SearchScope.Plugin p => new F.SearchScope.Plugin(p.Id),
        _ => new F.SearchScope.All(),
    };

    public static F.RepeatMode ToFfi(M.RepeatMode r) => r switch
    {
        M.RepeatMode.All => F.RepeatMode.All,
        M.RepeatMode.One => F.RepeatMode.One,
        _ => F.RepeatMode.Off,
    };

    public static F.ReplayGainMode ToFfi(M.ReplayGainMode r) => r switch
    {
        M.ReplayGainMode.Track => F.ReplayGainMode.Track,
        M.ReplayGainMode.Album => F.ReplayGainMode.Album,
        _ => F.ReplayGainMode.Off,
    };

    public static F.AuthMethod ToFfi(M.AuthMethod a) => a switch
    {
        M.AuthMethod.Qr => F.AuthMethod.Qr,
        M.AuthMethod.Oauth => F.AuthMethod.Oauth,
        M.AuthMethod.CookieImport => F.AuthMethod.CookieImport,
        M.AuthMethod.PasswordExchange => F.AuthMethod.PasswordExchange,
        _ => F.AuthMethod.ApiKey,
    };

    public static F.EqSettings ToFfi(M.EqSettings e) => new(e.Enabled, e.PreampDb, e.Bands.Select(b => new F.EqBand(
        b.Kind switch
        {
            M.EqBandKind.LowShelf => F.BandKind.LowShelf,
            M.EqBandKind.HighShelf => F.BandKind.HighShelf,
            _ => F.BandKind.Peaking,
        },
        b.FreqHz, b.GainDb, b.Q, b.Enabled)).ToArray());

    public static F.CrossfadeSettings ToFfi(M.CrossfadeSettings c) => new(
        c.Enabled, Math.Clamp(c.DurationMs, M.CrossfadeSettings.MinDurationMs, M.CrossfadeSettings.MaxDurationMs), c.AlbumContinuity);

    public static F.Settings ToFfi(M.AppSettings s) => new(
        s.Appearance switch
        {
            M.AppearancePreference.Light => F.AppearancePref.Light,
            M.AppearancePreference.Dark => F.AppearancePref.Dark,
            _ => F.AppearancePref.System,
        },
        s.Style == M.BackdropStyle.Misty ? F.BackdropStyle.Misty : F.BackdropStyle.Artwork,
        s.AppIcon,
        s.HueSource is M.HueSource.Fixed f ? new F.HueSource.Fixed(f.Hue) : new F.HueSource.Artwork(),
        s.ReduceEffects,
        s.Transparency,
        s.LyricsSize switch
        {
            M.LyricsSize.Small => F.LyricsSize.Small,
            M.LyricsSize.Large => F.LyricsSize.Large,
            _ => F.LyricsSize.Medium,
        },
        s.LyricsAlign == M.LyricsAlign.Center ? F.LyricsAlign.Center : F.LyricsAlign.Start,
        s.ShowTranslation,
        s.ShowRomanization,
        ToFfi(s.Crossfade),
        ToFfi(s.ReplayGainMode),
        s.PreampDb,
        ToFfi(s.Eq),
        s.OutputDevice,
        s.ResumeOnLaunch,
        s.WatchFolders,
        s.AllowCommunityPlugins,
        s.CheckForUpdates,
        s.ShowLyricsBar,
        s.DesktopLyrics,
        ToFfi(s.StreamingQuality),
        // The core refuses a null here: a record built without them sends the core's defaults.
        ToFfi(s.Downloads ?? M.DownloadSettings.Default),
        ToFfi(s.MiniPlayer ?? M.MiniPlayerSettings.Default));

    public static F.TrackQuery ToFfi(M.TrackQuery q) => new(q.Offset, q.Limit, ToFfi(q.Sort), q.Descending, q.AlbumId, q.ArtistId);

    public static F.TrackSort ToFfi(M.TrackSort s) => s switch
    {
        M.TrackSort.Title => F.TrackSort.Title,
        M.TrackSort.Album => F.TrackSort.Album,
        M.TrackSort.Year => F.TrackSort.Year,
        M.TrackSort.Duration => F.TrackSort.Duration,
        M.TrackSort.Path => F.TrackSort.Path,
        M.TrackSort.Added => F.TrackSort.Added,
        _ => F.TrackSort.Artist,
    };

    public static M.TrackSort ToModel(F.TrackSort s) => s switch
    {
        F.TrackSort.Title => M.TrackSort.Title,
        F.TrackSort.Album => M.TrackSort.Album,
        F.TrackSort.Year => M.TrackSort.Year,
        F.TrackSort.Duration => M.TrackSort.Duration,
        F.TrackSort.Path => M.TrackSort.Path,
        F.TrackSort.Added => M.TrackSort.Added,
        _ => M.TrackSort.Artist,
    };

    public static F.AlbumSort ToFfi(M.AlbumSort s) => s switch
    {
        M.AlbumSort.Artist => F.AlbumSort.Artist,
        M.AlbumSort.Year => F.AlbumSort.Year,
        M.AlbumSort.Added => F.AlbumSort.Added,
        _ => F.AlbumSort.Title,
    };

    public static F.PlaybackAnchor ToFfi(M.PlaybackAnchor a) =>
        new(a.TrackGeneration, a.PositionUs, a.AnchorMonoNs, a.Rate, a.DurationUs);

    public static F.LyricTimeline ToFfi(M.LyricTimelineData t) =>
        new(t.LineStarts, t.LineEnds, t.SegmentStarts, t.SegmentLines, t.WordOffsets, t.WordStarts, t.WordEnds);

    private static TOut[] List<TIn, TOut>(IReadOnlyList<TIn>? items, Func<TIn, TOut> map)
    {
        if (items is null || items.Count == 0)
        {
            return [];
        }

        var result = new TOut[items.Count];
        for (var i = 0; i < result.Length; i++)
        {
            result[i] = map(items[i]);
        }

        return result;
    }
}
