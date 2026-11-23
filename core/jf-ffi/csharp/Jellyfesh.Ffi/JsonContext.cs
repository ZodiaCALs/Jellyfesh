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
[JsonSerializable(typeof(StoreEntitlement))]
[JsonSerializable(typeof(LyricsBar))]
[JsonSerializable(typeof(LyricTimeline))]
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
[JsonSerializable(typeof(Command.MakeLyrics), TypeInfoPropertyName = "CommandMakeLyrics")]
[JsonSerializable(typeof(Command.CancelLyrics), TypeInfoPropertyName = "CommandCancelLyrics")]
[JsonSerializable(typeof(Command.DeleteMadeLyrics), TypeInfoPropertyName = "CommandDeleteMadeLyrics")]
[JsonSerializable(typeof(Command.AllowAudioUpload), TypeInfoPropertyName = "CommandAllowAudioUpload")]
[JsonSerializable(typeof(Command.RevokeAudioUpload), TypeInfoPropertyName = "CommandRevokeAudioUpload")]
[JsonSerializable(typeof(Command.DownloadSpeechModel), TypeInfoPropertyName = "CommandDownloadSpeechModel")]
[JsonSerializable(typeof(Command.CancelSpeechModelDownload), TypeInfoPropertyName = "CommandCancelSpeechModelDownload")]
[JsonSerializable(typeof(Command.DeleteSpeechModel), TypeInfoPropertyName = "CommandDeleteSpeechModel")]
[JsonSerializable(typeof(Command.ResetPluginSettings), TypeInfoPropertyName = "CommandResetPluginSettings")]
[JsonSerializable(typeof(Command.ActivateLicence), TypeInfoPropertyName = "CommandActivateLicence")]
[JsonSerializable(typeof(Command.RemoveLicence), TypeInfoPropertyName = "CommandRemoveLicence")]
[JsonSerializable(typeof(Command.CheckForUpdates), TypeInfoPropertyName = "CommandCheckForUpdates")]
[JsonSerializable(typeof(Command.Discover), TypeInfoPropertyName = "CommandDiscover")]
[JsonSerializable(typeof(Command.Details), TypeInfoPropertyName = "CommandDetails")]
[JsonSerializable(typeof(Command.Comments), TypeInfoPropertyName = "CommandComments")]
[JsonSerializable(typeof(Command.SetNetworkState), TypeInfoPropertyName = "CommandSetNetworkState")]
[JsonSerializable(typeof(Command.SearchMore), TypeInfoPropertyName = "CommandSearchMore")]
[JsonSerializable(typeof(Command.RefreshSourceLibrary), TypeInfoPropertyName = "CommandRefreshSourceLibrary")]
[JsonSerializable(typeof(Command.SourceLibraryMore), TypeInfoPropertyName = "CommandSourceLibraryMore")]
[JsonSerializable(typeof(Command.SavePlaylistCopy), TypeInfoPropertyName = "CommandSavePlaylistCopy")]
[JsonSerializable(typeof(Command.Download), TypeInfoPropertyName = "CommandDownload")]
[JsonSerializable(typeof(Command.CancelDownload), TypeInfoPropertyName = "CommandCancelDownload")]
[JsonSerializable(typeof(Command.RetryDownload), TypeInfoPropertyName = "CommandRetryDownload")]
[JsonSerializable(typeof(Command.RemoveDownload), TypeInfoPropertyName = "CommandRemoveDownload")]
[JsonSerializable(typeof(Command.RemoveAllDownloads), TypeInfoPropertyName = "CommandRemoveAllDownloads")]
[JsonSerializable(typeof(DownloadTarget.Track), TypeInfoPropertyName = "DownloadTargetTrack")]
[JsonSerializable(typeof(DownloadTarget.Collection), TypeInfoPropertyName = "DownloadTargetCollection")]
[JsonSerializable(typeof(QualityNote.FilesPlayAsTheyAre), TypeInfoPropertyName = "QualityNoteFilesPlayAsTheyAre")]
[JsonSerializable(typeof(QualityNote.SetInService), TypeInfoPropertyName = "QualityNoteSetInService")]
[JsonSerializable(typeof(DownloadError.NotAllowed), TypeInfoPropertyName = "DownloadErrorNotAllowed")]
[JsonSerializable(typeof(DownloadError.NotOffered), TypeInfoPropertyName = "DownloadErrorNotOffered")]
[JsonSerializable(typeof(DownloadError.Network), TypeInfoPropertyName = "DownloadErrorNetwork")]
[JsonSerializable(typeof(DownloadError.Storage), TypeInfoPropertyName = "DownloadErrorStorage")]
[JsonSerializable(typeof(DownloadError.Other), TypeInfoPropertyName = "DownloadErrorOther")]
[JsonSerializable(typeof(DownloadState.Queued), TypeInfoPropertyName = "DownloadStateQueued")]
[JsonSerializable(typeof(DownloadState.Downloading), TypeInfoPropertyName = "DownloadStateDownloading")]
[JsonSerializable(typeof(DownloadState.Done), TypeInfoPropertyName = "DownloadStateDone")]
[JsonSerializable(typeof(DownloadState.Failed), TypeInfoPropertyName = "DownloadStateFailed")]
[JsonSerializable(typeof(SongRef.Current), TypeInfoPropertyName = "SongRefCurrent")]
[JsonSerializable(typeof(SongRef.Library), TypeInfoPropertyName = "SongRefLibrary")]
[JsonSerializable(typeof(ItemRef.Track), TypeInfoPropertyName = "ItemRefTrack")]
[JsonSerializable(typeof(ItemRef.Album), TypeInfoPropertyName = "ItemRefAlbum")]
[JsonSerializable(typeof(ItemRef.Artist), TypeInfoPropertyName = "ItemRefArtist")]
[JsonSerializable(typeof(ItemRef.File), TypeInfoPropertyName = "ItemRefFile")]
[JsonSerializable(typeof(ItemRef.Source), TypeInfoPropertyName = "ItemRefSource")]
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
[JsonSerializable(typeof(Collection.Songs), TypeInfoPropertyName = "CollectionSongs")]
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
[JsonSerializable(typeof(LyricsSource.Made), TypeInfoPropertyName = "LyricsSourceMade")]
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
[JsonSerializable(typeof(Event.AiLyrics), TypeInfoPropertyName = "EventAiLyrics")]
[JsonSerializable(typeof(Event.Look), TypeInfoPropertyName = "EventLook")]
[JsonSerializable(typeof(Event.Pro), TypeInfoPropertyName = "EventPro")]
[JsonSerializable(typeof(Event.UpdateStatus), TypeInfoPropertyName = "EventUpdateStatus")]
[JsonSerializable(typeof(Event.Discover), TypeInfoPropertyName = "EventDiscover")]
[JsonSerializable(typeof(Event.Details), TypeInfoPropertyName = "EventDetails")]
[JsonSerializable(typeof(Event.Comments), TypeInfoPropertyName = "EventComments")]
[JsonSerializable(typeof(Event.Artwork), TypeInfoPropertyName = "EventArtwork")]
[JsonSerializable(typeof(Event.SearchPage), TypeInfoPropertyName = "EventSearchPage")]
[JsonSerializable(typeof(Event.SourceLibrary), TypeInfoPropertyName = "EventSourceLibrary")]
[JsonSerializable(typeof(Event.SourceLibraryPage), TypeInfoPropertyName = "EventSourceLibraryPage")]
[JsonSerializable(typeof(Event.Downloads), TypeInfoPropertyName = "EventDownloads")]
[JsonSerializable(typeof(AiLyricsNeed.Nothing), TypeInfoPropertyName = "AiLyricsNeedNothing")]
[JsonSerializable(typeof(AiLyricsNeed.Pro), TypeInfoPropertyName = "AiLyricsNeedPro")]
[JsonSerializable(typeof(AiLyricsNeed.SignIn), TypeInfoPropertyName = "AiLyricsNeedSignIn")]
[JsonSerializable(typeof(AiLyricsNeed.Consent), TypeInfoPropertyName = "AiLyricsNeedConsent")]
[JsonSerializable(typeof(AiLyricsNeed.Model), TypeInfoPropertyName = "AiLyricsNeedModel")]
[JsonSerializable(typeof(AiLyricsNeed.Unavailable), TypeInfoPropertyName = "AiLyricsNeedUnavailable")]
[JsonSerializable(typeof(LyricsJobStage.Waiting), TypeInfoPropertyName = "LyricsJobStageWaiting")]
[JsonSerializable(typeof(LyricsJobStage.ReadingAudio), TypeInfoPropertyName = "LyricsJobStageReadingAudio")]
[JsonSerializable(typeof(LyricsJobStage.Sending), TypeInfoPropertyName = "LyricsJobStageSending")]
[JsonSerializable(typeof(LyricsJobStage.Recognising), TypeInfoPropertyName = "LyricsJobStageRecognising")]
[JsonSerializable(typeof(LyricsJobStage.TimingWords), TypeInfoPropertyName = "LyricsJobStageTimingWords")]
[JsonSerializable(typeof(LyricsJobStage.Done), TypeInfoPropertyName = "LyricsJobStageDone")]
[JsonSerializable(typeof(LyricsJobStage.Failed), TypeInfoPropertyName = "LyricsJobStageFailed")]
[JsonSerializable(typeof(LyricsJobStage.Cancelled), TypeInfoPropertyName = "LyricsJobStageCancelled")]
[JsonSerializable(typeof(SpeechModelStatus.NotDownloaded), TypeInfoPropertyName = "SpeechModelStatusNotDownloaded")]
[JsonSerializable(typeof(SpeechModelStatus.Downloading), TypeInfoPropertyName = "SpeechModelStatusDownloading")]
[JsonSerializable(typeof(SpeechModelStatus.Ready), TypeInfoPropertyName = "SpeechModelStatusReady")]
[JsonSerializable(typeof(SpeechModelStatus.Failed), TypeInfoPropertyName = "SpeechModelStatusFailed")]
[JsonSerializable(typeof(LyricsHighlight.Artwork), TypeInfoPropertyName = "LyricsHighlightArtwork")]
[JsonSerializable(typeof(LyricsHighlight.Accent), TypeInfoPropertyName = "LyricsHighlightAccent")]
[JsonSerializable(typeof(LyricsHighlight.Custom), TypeInfoPropertyName = "LyricsHighlightCustom")]
[JsonSerializable(typeof(ProSource.None), TypeInfoPropertyName = "ProSourceNone")]
[JsonSerializable(typeof(ProSource.Preview), TypeInfoPropertyName = "ProSourcePreview")]
[JsonSerializable(typeof(ProSource.Store), TypeInfoPropertyName = "ProSourceStore")]
[JsonSerializable(typeof(ProSource.Licence), TypeInfoPropertyName = "ProSourceLicence")]
[JsonSerializable(typeof(FactKey.Version), TypeInfoPropertyName = "FactKeyVersion")]
[JsonSerializable(typeof(FactKey.Genre), TypeInfoPropertyName = "FactKeyGenre")]
[JsonSerializable(typeof(FactKey.Language), TypeInfoPropertyName = "FactKeyLanguage")]
[JsonSerializable(typeof(FactKey.Released), TypeInfoPropertyName = "FactKeyReleased")]
[JsonSerializable(typeof(FactKey.Label), TypeInfoPropertyName = "FactKeyLabel")]
[JsonSerializable(typeof(FactKey.Composer), TypeInfoPropertyName = "FactKeyComposer")]
[JsonSerializable(typeof(FactKey.Lyricist), TypeInfoPropertyName = "FactKeyLyricist")]
[JsonSerializable(typeof(FactKey.Arranger), TypeInfoPropertyName = "FactKeyArranger")]
[JsonSerializable(typeof(FactKey.Producer), TypeInfoPropertyName = "FactKeyProducer")]
[JsonSerializable(typeof(FactKey.Popularity), TypeInfoPropertyName = "FactKeyPopularity")]
[JsonSerializable(typeof(FactKey.Quality), TypeInfoPropertyName = "FactKeyQuality")]
[JsonSerializable(typeof(FactKey.Isrc), TypeInfoPropertyName = "FactKeyIsrc")]
[JsonSerializable(typeof(FactKey.Disc), TypeInfoPropertyName = "FactKeyDisc")]
[JsonSerializable(typeof(FactKey.Track), TypeInfoPropertyName = "FactKeyTrack")]
[JsonSerializable(typeof(FactKey.Other), TypeInfoPropertyName = "FactKeyOther")]
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
[JsonSerializable(typeof(EntitlementArgs))]
[JsonSerializable(typeof(SchemaArgs))]
[JsonSerializable(typeof(ArtworkArgs))]
[JsonSerializable(typeof(LyricsBarArgs))]
[JsonSerializable(typeof(AddressArgs))]
[JsonSerializable(typeof(TemplateArgs))]
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

internal sealed record EntitlementArgs(StoreEntitlement Entitlement);

internal sealed record SchemaArgs(string Plugin, string Language);

internal sealed record ArtworkArgs(string Url, uint SizePx);

internal sealed record LyricsBarArgs(LyricTimeline Timeline, [property: JsonPropertyName("t_ms")] uint TMs);

internal sealed record AddressArgs(string Address);

internal sealed record TemplateArgs(string Template);

/// <summary>A secret store request from the core: op is get, set or delete.</summary>
internal sealed record SecretRequest(string Op, string Key, string? Value);

/// <summary>{"ok": value or null}: a successful secret store answer.</summary>
internal sealed record SecretAnswer(
    [property: JsonIgnore(Condition = JsonIgnoreCondition.Never)] string? Ok);

/// <summary>{"error": message}: a failed secret store answer.</summary>
internal sealed record SecretFailure(string Error);

/// <summary>{"kind": "...", "message": "..."}.</summary>
internal sealed record ErrorJson(string Kind, string Message);
