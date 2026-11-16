// System.Text.Json source generation: no reflection, so the library is NativeAOT-safe.

using System.Collections.Generic;
using System.Text.Json.Serialization;

namespace Jellyfesh.Ffi;

/// <summary>JSON for the mirror types, in the shapes jf-ffi's C ABI reads and writes.</summary>
[JsonSourceGenerationOptions(PropertyNamingPolicy = JsonKnownNamingPolicy.SnakeCaseLower)]
[JsonSerializable(typeof(CoreConfig))]
[JsonSerializable(typeof(Command))]
[JsonSerializable(typeof(Event))]
[JsonSerializable(typeof(Snapshot))]
[JsonSerializable(typeof(TrackRef))]
[JsonSerializable(typeof(TrackQuery))]
[JsonSerializable(typeof(Track))]
[JsonSerializable(typeof(Album))]
[JsonSerializable(typeof(Artist))]
[JsonSerializable(typeof(SearchResults))]
[JsonSerializable(typeof(LibraryStats))]
[JsonSerializable(typeof(TrackRow))]
[JsonSerializable(typeof(PlaylistSummary))]
[JsonSerializable(typeof(PlaylistEntry))]
[JsonSerializable(typeof(QueueItem))]
[JsonSerializable(typeof(HistoryEntry))]
[JsonSerializable(typeof(DeviceInfo))]
[JsonSerializable(typeof(EqPreset))]
[JsonSerializable(typeof(IReadOnlyList<Track>))]
[JsonSerializable(typeof(IReadOnlyList<Album>))]
[JsonSerializable(typeof(IReadOnlyList<Artist>))]
[JsonSerializable(typeof(IReadOnlyList<TrackRef>))]
[JsonSerializable(typeof(IReadOnlyList<TrackRow>))]
[JsonSerializable(typeof(IReadOnlyList<PlaylistSummary>))]
[JsonSerializable(typeof(IReadOnlyList<PlaylistEntry>))]
[JsonSerializable(typeof(IReadOnlyList<QueueItem>))]
[JsonSerializable(typeof(IReadOnlyList<HistoryEntry>))]
[JsonSerializable(typeof(IReadOnlyList<DeviceInfo>))]
[JsonSerializable(typeof(IReadOnlyList<EqPreset>))]
[JsonSerializable(typeof(IReadOnlyList<string>))]
[JsonSerializable(typeof(bool))]
[JsonSerializable(typeof(string))]
// Every variant record by its full name: several share a simple name (Event.Theme and Theme).
[JsonSerializable(typeof(OutputKind.Device), TypeInfoPropertyName = "OutputKindDevice")]
[JsonSerializable(typeof(OutputKind.WavFile), TypeInfoPropertyName = "OutputKindWavFile")]
[JsonSerializable(typeof(OutputKind.Null), TypeInfoPropertyName = "OutputKindNull")]
[JsonSerializable(typeof(Command.PlayTracks), TypeInfoPropertyName = "CommandPlayTracks")]
[JsonSerializable(typeof(Command.Enqueue), TypeInfoPropertyName = "CommandEnqueue")]
[JsonSerializable(typeof(Command.RemoveFromQueue), TypeInfoPropertyName = "CommandRemoveFromQueue")]
[JsonSerializable(typeof(Command.MoveInQueue), TypeInfoPropertyName = "CommandMoveInQueue")]
[JsonSerializable(typeof(Command.JumpTo), TypeInfoPropertyName = "CommandJumpTo")]
[JsonSerializable(typeof(Command.ClearQueue), TypeInfoPropertyName = "CommandClearQueue")]
[JsonSerializable(typeof(Command.SetShuffle), TypeInfoPropertyName = "CommandSetShuffle")]
[JsonSerializable(typeof(Command.SetRepeat), TypeInfoPropertyName = "CommandSetRepeat")]
[JsonSerializable(typeof(Command.Play), TypeInfoPropertyName = "CommandPlay")]
[JsonSerializable(typeof(Command.Pause), TypeInfoPropertyName = "CommandPause")]
[JsonSerializable(typeof(Command.TogglePlayPause), TypeInfoPropertyName = "CommandTogglePlayPause")]
[JsonSerializable(typeof(Command.Stop), TypeInfoPropertyName = "CommandStop")]
[JsonSerializable(typeof(Command.Next), TypeInfoPropertyName = "CommandNext")]
[JsonSerializable(typeof(Command.Previous), TypeInfoPropertyName = "CommandPrevious")]
[JsonSerializable(typeof(Command.Seek), TypeInfoPropertyName = "CommandSeek")]
[JsonSerializable(typeof(Command.SetVolume), TypeInfoPropertyName = "CommandSetVolume")]
[JsonSerializable(typeof(Command.SetEq), TypeInfoPropertyName = "CommandSetEq")]
[JsonSerializable(typeof(Command.SetReplayGain), TypeInfoPropertyName = "CommandSetReplayGain")]
[JsonSerializable(typeof(Command.AddRoot), TypeInfoPropertyName = "CommandAddRoot")]
[JsonSerializable(typeof(Command.RemoveRoot), TypeInfoPropertyName = "CommandRemoveRoot")]
[JsonSerializable(typeof(Command.Rescan), TypeInfoPropertyName = "CommandRescan")]
[JsonSerializable(typeof(Command.PlayCollection), TypeInfoPropertyName = "CommandPlayCollection")]
[JsonSerializable(typeof(Command.EnqueueCollection), TypeInfoPropertyName = "CommandEnqueueCollection")]
[JsonSerializable(typeof(Command.SetSettings), TypeInfoPropertyName = "CommandSetSettings")]
[JsonSerializable(typeof(Command.SetMuted), TypeInfoPropertyName = "CommandSetMuted")]
[JsonSerializable(typeof(Command.SetCrossfade), TypeInfoPropertyName = "CommandSetCrossfade")]
[JsonSerializable(typeof(Command.SetOutputDevice), TypeInfoPropertyName = "CommandSetOutputDevice")]
[JsonSerializable(typeof(Command.RefreshDevices), TypeInfoPropertyName = "CommandRefreshDevices")]
[JsonSerializable(typeof(Command.CreatePlaylist), TypeInfoPropertyName = "CommandCreatePlaylist")]
[JsonSerializable(typeof(Command.RenamePlaylist), TypeInfoPropertyName = "CommandRenamePlaylist")]
[JsonSerializable(typeof(Command.DeletePlaylist), TypeInfoPropertyName = "CommandDeletePlaylist")]
[JsonSerializable(typeof(Command.AddToPlaylist), TypeInfoPropertyName = "CommandAddToPlaylist")]
[JsonSerializable(typeof(Command.RemoveFromPlaylist), TypeInfoPropertyName = "CommandRemoveFromPlaylist")]
[JsonSerializable(typeof(Command.MovePlaylistEntry), TypeInfoPropertyName = "CommandMovePlaylistEntry")]
[JsonSerializable(typeof(Command.ImportPlaylist), TypeInfoPropertyName = "CommandImportPlaylist")]
[JsonSerializable(typeof(Command.SetFavorite), TypeInfoPropertyName = "CommandSetFavorite")]
[JsonSerializable(typeof(Command.InstallPlugin), TypeInfoPropertyName = "CommandInstallPlugin")]
[JsonSerializable(typeof(Command.UninstallPlugin), TypeInfoPropertyName = "CommandUninstallPlugin")]
[JsonSerializable(typeof(Command.SetPluginEnabled), TypeInfoPropertyName = "CommandSetPluginEnabled")]
[JsonSerializable(typeof(Command.SetPluginSettings), TypeInfoPropertyName = "CommandSetPluginSettings")]
[JsonSerializable(typeof(Command.AcknowledgeUnofficial), TypeInfoPropertyName = "CommandAcknowledgeUnofficial")]
[JsonSerializable(typeof(Command.BeginLogin), TypeInfoPropertyName = "CommandBeginLogin")]
[JsonSerializable(typeof(Command.SubmitLogin), TypeInfoPropertyName = "CommandSubmitLogin")]
[JsonSerializable(typeof(Command.CancelLogin), TypeInfoPropertyName = "CommandCancelLogin")]
[JsonSerializable(typeof(Command.Logout), TypeInfoPropertyName = "CommandLogout")]
[JsonSerializable(typeof(Command.Search), TypeInfoPropertyName = "CommandSearch")]
[JsonSerializable(typeof(Command.Browse), TypeInfoPropertyName = "CommandBrowse")]
[JsonSerializable(typeof(Command.Remote), TypeInfoPropertyName = "CommandRemote")]
[JsonSerializable(typeof(RemoteAction.Refresh), TypeInfoPropertyName = "RemoteActionRefresh")]
[JsonSerializable(typeof(RemoteAction.Transfer), TypeInfoPropertyName = "RemoteActionTransfer")]
[JsonSerializable(typeof(RemoteAction.SetVolume), TypeInfoPropertyName = "RemoteActionSetVolume")]
[JsonSerializable(typeof(RemoteAction.End), TypeInfoPropertyName = "RemoteActionEnd")]
[JsonSerializable(typeof(TrackRef.Library), TypeInfoPropertyName = "TrackRefLibrary")]
[JsonSerializable(typeof(TrackRef.File), TypeInfoPropertyName = "TrackRefFile")]
[JsonSerializable(typeof(TrackRef.Source), TypeInfoPropertyName = "TrackRefSource")]
[JsonSerializable(typeof(Collection.Album), TypeInfoPropertyName = "CollectionAlbum")]
[JsonSerializable(typeof(Collection.Artist), TypeInfoPropertyName = "CollectionArtist")]
[JsonSerializable(typeof(Collection.Playlist), TypeInfoPropertyName = "CollectionPlaylist")]
[JsonSerializable(typeof(Collection.Favorites), TypeInfoPropertyName = "CollectionFavorites")]
[JsonSerializable(typeof(Collection.Tracks), TypeInfoPropertyName = "CollectionTracks")]
[JsonSerializable(typeof(Collection.Source), TypeInfoPropertyName = "CollectionSource")]
[JsonSerializable(typeof(SearchScope.All), TypeInfoPropertyName = "SearchScopeAll")]
[JsonSerializable(typeof(SearchScope.Library), TypeInfoPropertyName = "SearchScopeLibrary")]
[JsonSerializable(typeof(SearchScope.Plugin), TypeInfoPropertyName = "SearchScopePlugin")]
[JsonSerializable(typeof(HueSource.Artwork), TypeInfoPropertyName = "HueSourceArtwork")]
[JsonSerializable(typeof(HueSource.Fixed), TypeInfoPropertyName = "HueSourceFixed")]
[JsonSerializable(typeof(AuthStep.ShowQr), TypeInfoPropertyName = "AuthStepShowQr")]
[JsonSerializable(typeof(AuthStep.ShowCode), TypeInfoPropertyName = "AuthStepShowCode")]
[JsonSerializable(typeof(AuthStep.OpenUrl), TypeInfoPropertyName = "AuthStepOpenUrl")]
[JsonSerializable(typeof(AuthStep.AskFields), TypeInfoPropertyName = "AuthStepAskFields")]
[JsonSerializable(typeof(AuthStep.Waiting), TypeInfoPropertyName = "AuthStepWaiting")]
[JsonSerializable(typeof(AuthStep.Done), TypeInfoPropertyName = "AuthStepDone")]
[JsonSerializable(typeof(AuthStep.Failed), TypeInfoPropertyName = "AuthStepFailed")]
[JsonSerializable(typeof(ArtworkRef.File), TypeInfoPropertyName = "ArtworkRefFile")]
[JsonSerializable(typeof(ArtworkRef.Url), TypeInfoPropertyName = "ArtworkRefUrl")]
[JsonSerializable(typeof(ScanStatus.Progress), TypeInfoPropertyName = "ScanStatusProgress")]
[JsonSerializable(typeof(ScanStatus.Finished), TypeInfoPropertyName = "ScanStatusFinished")]
[JsonSerializable(typeof(ScanStatus.Failed), TypeInfoPropertyName = "ScanStatusFailed")]
[JsonSerializable(typeof(LyricsSource.Sidecar), TypeInfoPropertyName = "LyricsSourceSidecar")]
[JsonSerializable(typeof(LyricsSource.Embedded), TypeInfoPropertyName = "LyricsSourceEmbedded")]
[JsonSerializable(typeof(LyricsSource.Plugin), TypeInfoPropertyName = "LyricsSourcePlugin")]
[JsonSerializable(typeof(Event.Playback), TypeInfoPropertyName = "EventPlayback")]
[JsonSerializable(typeof(Event.Queue), TypeInfoPropertyName = "EventQueue")]
[JsonSerializable(typeof(Event.NowPlaying), TypeInfoPropertyName = "EventNowPlaying")]
[JsonSerializable(typeof(Event.Lyrics), TypeInfoPropertyName = "EventLyrics")]
[JsonSerializable(typeof(Event.Theme), TypeInfoPropertyName = "EventTheme")]
[JsonSerializable(typeof(Event.Scan), TypeInfoPropertyName = "EventScan")]
[JsonSerializable(typeof(Event.LibraryChanged), TypeInfoPropertyName = "EventLibraryChanged")]
[JsonSerializable(typeof(Event.PlaylistCreated), TypeInfoPropertyName = "EventPlaylistCreated")]
[JsonSerializable(typeof(Event.Settings), TypeInfoPropertyName = "EventSettings")]
[JsonSerializable(typeof(Event.Devices), TypeInfoPropertyName = "EventDevices")]
[JsonSerializable(typeof(Event.Sources), TypeInfoPropertyName = "EventSources")]
[JsonSerializable(typeof(Event.Remote), TypeInfoPropertyName = "EventRemote")]
[JsonSerializable(typeof(Event.Auth), TypeInfoPropertyName = "EventAuth")]
[JsonSerializable(typeof(Event.SearchResults), TypeInfoPropertyName = "EventSearchResults")]
[JsonSerializable(typeof(Event.BrowsePage), TypeInfoPropertyName = "EventBrowsePage")]
[JsonSerializable(typeof(Event.Buffering), TypeInfoPropertyName = "EventBuffering")]
[JsonSerializable(typeof(Event.CommandFailed), TypeInfoPropertyName = "EventCommandFailed")]
[JsonSerializable(typeof(Event.Error), TypeInfoPropertyName = "EventError")]
public partial class JellyfeshJsonContext : JsonSerializerContext
{
}

/// <summary>Query arguments and errors: JSON only the C ABI sees.</summary>
[JsonSourceGenerationOptions(PropertyNamingPolicy = JsonKnownNamingPolicy.SnakeCaseLower)]
[JsonSerializable(typeof(NoArgs))]
[JsonSerializable(typeof(PageArgs))]
[JsonSerializable(typeof(SortedPageArgs))]
[JsonSerializable(typeof(LimitArgs))]
[JsonSerializable(typeof(IdArgs))]
[JsonSerializable(typeof(AlbumIdArgs))]
[JsonSerializable(typeof(ArtistIdArgs))]
[JsonSerializable(typeof(QueryArgs))]
[JsonSerializable(typeof(TextArgs))]
[JsonSerializable(typeof(PlaylistPageArgs))]
[JsonSerializable(typeof(ExportArgs))]
[JsonSerializable(typeof(TrackArgs))]
[JsonSerializable(typeof(ThumbnailArgs))]
[JsonSerializable(typeof(ErrorJson))]
[JsonSerializable(typeof(SecretRequest))]
[JsonSerializable(typeof(SecretAnswer))]
[JsonSerializable(typeof(SecretFailure))]
[JsonSerializable(typeof(TrackRef.Library), TypeInfoPropertyName = "TrackRefLibrary")]
[JsonSerializable(typeof(TrackRef.File), TypeInfoPropertyName = "TrackRefFile")]
[JsonSerializable(typeof(TrackRef.Source), TypeInfoPropertyName = "TrackRefSource")]
internal partial class InteropJsonContext : JsonSerializerContext
{
}

internal sealed record NoArgs;

internal sealed record PageArgs(uint Offset, uint Limit);

internal sealed record SortedPageArgs(AlbumSort Sort, bool Descending, uint Offset, uint Limit);

internal sealed record LimitArgs(uint Limit);

internal sealed record IdArgs(long Id);

internal sealed record AlbumIdArgs(long AlbumId);

internal sealed record ArtistIdArgs(long ArtistId);

internal sealed record QueryArgs(TrackQuery Query);

internal sealed record TextArgs(string Text, uint Limit);

internal sealed record PlaylistPageArgs(long Id, uint Offset, uint Limit);

internal sealed record ExportArgs(long Id, string Path);

internal sealed record TrackArgs(TrackRef Track);

internal sealed record ThumbnailArgs(string Hash, uint SizePx);

/// <summary>A secret store request from the core: op is get, set or delete.</summary>
internal sealed record SecretRequest(string Op, string Key, string? Value);

/// <summary>{"ok": value or null}: a successful secret store answer.</summary>
internal sealed record SecretAnswer(
    [property: JsonIgnore(Condition = JsonIgnoreCondition.Never)] string? Ok);

/// <summary>{"error": message}: a failed secret store answer.</summary>
internal sealed record SecretFailure(string Error);

/// <summary>{"kind": "...", "message": "..."}.</summary>
internal sealed record ErrorJson(string Kind, string Message);
