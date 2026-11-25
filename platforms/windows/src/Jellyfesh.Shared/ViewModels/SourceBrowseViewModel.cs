using System.Collections.ObjectModel;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using Jellyfesh.Core;
using Jellyfesh.Formatting;

namespace Jellyfesh.ViewModels;

/// <summary>What a source page shows: the source's top level (no node), a folder, an artist, an album or a playlist.</summary>
/// <param name="SongsListed">False for a source playlist whose songs the service lists only in its own app.</param>
public sealed record SourceTarget(string Plugin, string? Node, string Title, ItemKind? Kind, SourceItem? Item = null, bool SongsListed = true)
{
    /// <summary>The source's top level, titled with its name.</summary>
    public static SourceTarget Root(PluginInfo plugin) => new(plugin.Id, null, plugin.Name, null);

    /// <summary>An album, artist, playlist or folder the source listed.</summary>
    public static SourceTarget Of(SourceItem item) => new(item.Source, item.Id, item.Title, item.Kind, item);
}

/// <summary>
/// A page of a source's catalogue (docs/ui.md, "Sources"; the web's source page is the model,
/// docs/parity.md N17, N18): the source's top level or a folder lists what it holds; an album or a
/// playlist has the large header (artwork, kind, title, the artist in accent text, the song count,
/// Play, Shuffle and More) above its songs, numbered on an album, and the "12 songs, 48 minutes"
/// footer. Pages of the catalogue stream in as BrowsePage events, in the plugin's order (folders,
/// albums and songs mixed as the source lists them), with Load More while there are more. Choosing
/// an album, artist, playlist or folder opens its own page; a song plays with the rest of its album
/// or playlist after it. A source that asks for credit says "Content from {Service}". Nothing here
/// sorts or filters: the order and what plays are the core's.
/// </summary>
public sealed partial class SourceBrowseViewModel : PageViewModel
{
    private ulong _request;
    private uint _requestedPage;
    private uint? _nextPage;
    private bool _started;

    /// <summary>A page was asked for while the page was shown and has not arrived (asked again on return).</summary>
    private bool _pending;

    /// <summary>Items so far, songs or not, in the plugin's order: a song's place for PlayCollection's start.</summary>
    private int _itemCount;

    [ObservableProperty]
    private bool _isBrowsing;

    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(HasError))]
    private string? _error;

    [ObservableProperty]
    private bool _hasNextPage;

    /// <summary>Every page has arrived.</summary>
    [ObservableProperty]
    private bool _isComplete;

    /// <summary>The song count under an album's or playlist's title ("12 songs"), once every page is here.</summary>
    [ObservableProperty]
    private string _facts = string.Empty;

    /// <summary>"12 songs, 48 minutes" under the last song, once every page is here.</summary>
    [ObservableProperty]
    private string _footer = string.Empty;

    /// <summary>Play and Shuffle show: something of this album, artist, playlist or folder plays here.</summary>
    [ObservableProperty]
    private bool _canPlay;

    /// <summary>Nothing here plays here, but the item has a link: Open in {Service} is the main action.</summary>
    [ObservableProperty]
    private bool _opensLink;

    /// <summary>Every page is here and the source listed nothing: "Nothing Here".</summary>
    [ObservableProperty]
    private bool _isNothingHere;

    public SourceBrowseViewModel(ShellServices services, SourceTarget target)
        : base(services)
    {
        Target = target;
        var l = services.L;
        var sources = services.Hub.State.Sources;
        Title = target.Title;
        KindLabel = target.Kind is { } kind ? l.Get("Kind_" + kind) : string.Empty;
        Subtitle = target.Kind is ItemKind.Album or ItemKind.Playlist ? target.Item?.Subtitle ?? string.Empty : string.Empty;
        Attribution = ItemLinks.Attribution(sources, target.Plugin, l);
        Link = ItemLinks.Usable(target.Item?.Link);
        var service = ItemLinks.ServiceName(sources, target.Plugin);
        OpenInLabel = ItemLinks.OpenInLabel(l, service);
        IsRemote = ItemLinks.IsRemoteSource(sources, target.Plugin);
        Header = target.Item is { } item ? new SourceEntryItem(item, l) : null;
        SongsListedElsewhereText = SongsListedElsewhere ? l.Format("SourceLibrary_SongsOnlyIn", service, service) : string.Empty;
    }

    /// <summary>
    /// A source playlist whose songs the service lists only in its own app (songs-listed false, as a
    /// Spotify playlist the person follows): the page lists none and says so; Play and Shuffle still
    /// start it there (ADR-0024 §3).
    /// </summary>
    public bool SongsListedElsewhere => Target is { Kind: ItemKind.Playlist, SongsListed: false, Node: not null };

    /// <summary>"{Service} lists this playlist's songs only in {Service}."</summary>
    public string SongsListedElsewhereText { get; }

    /// <summary>Save as Playlist… (ADR-0024 §2): a source playlist whose songs are listed can be copied into a Jellyfesh playlist.</summary>
    public bool CanSaveCopy => Target is { Kind: ItemKind.Playlist, SongsListed: true, Node: not null };

    private ulong _saveRequest;
    private string _savedName = string.Empty;

    public SourceTarget Target { get; }

    public string Title { get; }

    /// <summary>"Album", "Playlist", "Artist" or "Folder" above the title; empty for the source's top level.</summary>
    public string KindLabel { get; }

    /// <summary>An album's artist (or a playlist's line), in accent text under the title.</summary>
    public string Subtitle { get; }

    /// <summary>"Content from {Service}", or null when the source does not ask for credit.</summary>
    public string? Attribution { get; }

    public bool HasAttribution => Attribution is not null;

    /// <summary>
    /// The item's artwork (and its link) for the header; null for the source's top level. A page
    /// opened by id (Go to Album, Go to Artist) gets it from the item's details.
    /// </summary>
    public SourceEntryItem? Header { get; private set; }

    /// <summary>An album's notes or an artist's biography, from the details ("More" expands it).</summary>
    public string Description { get; private set; } = string.Empty;

    public bool HasDescription => Description.Length > 0;

    private ulong _detailsRequest;

    /// <summary>
    /// Get Info for this page's album or artist: its source's details (docs/core-api.md, "Details and
    /// comments"), when the source offers them.
    /// </summary>
    public ItemRef? DetailsItem =>
        Target is { Node: { } id, Kind: ItemKind.Album or ItemKind.Artist } && S.Hub.State.Sources.Find(Target.Plugin) is { Details: true }
            ? new ItemRef.Source(Target.Plugin, Target.Kind!.Value, id)
            : null;

    public bool HasDetails => DetailsItem is not null;

    /// <summary>Raised when the header came with the details (the view loads its picture).</summary>
    public event Action? HeaderChanged;

    /// <summary>An album or a playlist: the large header with the artwork and the footer.</summary>
    public bool IsCollection => Target.Kind is ItemKind.Album or ItemKind.Playlist;

    /// <summary>An album's songs are numbered.</summary>
    public bool IsNumbered => Target.Kind == ItemKind.Album;

    /// <summary>
    /// The kind the core fetches this page's node by when it plays: the listed item's, or a folder
    /// for a node that is only a listing (a Discover shelf's more), whose pages the core browses.
    /// </summary>
    private ItemKind TargetKind => Target.Kind ?? ItemKind.Folder;

    /// <summary>The item opens in its service (More › Open in {Service}).</summary>
    public string? Link { get; }

    public bool HasLink => Link is not null;

    /// <summary>"Open in {Service}".</summary>
    public string OpenInLabel { get; }

    /// <summary>The source plays on the person's own devices: no Play Next or Play Last (ADR-0018).</summary>
    public bool IsRemote { get; }

    /// <summary>Play Next and Play Last are offered.</summary>
    public bool CanQueue => CanPlay && !IsRemote;

    /// <summary>More has something to offer.</summary>
    public bool HasMore => CanQueue || HasLink || CanSaveCopy || CanDownload;

    /// <summary>Download (ADR-0025): this album or playlist, when its source allows downloads.</summary>
    public bool CanDownload => DownloadTarget is not null;

    private DownloadTarget? DownloadTarget =>
        DownloadActions.For(S.Hub.State.Sources, Target.Plugin, Target.Node, Target.Kind, Target.SongsListed);

    public bool HasError => Error is not null;

    /// <summary>What the source listed so far, in its order: <see cref="SourceEntryItem"/>s and <see cref="TrackItem"/>s.</summary>
    public ObservableCollection<object> Rows { get; } = [];

    /// <summary>The songs among <see cref="Rows"/>.</summary>
    public IEnumerable<TrackItem> Tracks => Rows.OfType<TrackItem>();

    /// <summary>Choosing an album, artist, playlist or folder opens its page (the view navigates).</summary>
    public event Action<SourceTarget>? OpenRequested;

    protected override Task LoadCoreAsync()
    {
        // Pages come as events; the library's revision does not change what a source lists.
        if (!_started)
        {
            _started = true;
            if (SongsListedElsewhere)
            {
                // Nothing to list: the service keeps these songs to itself.
                IsComplete = true;
                Summarize();
                return Task.CompletedTask;
            }

            Browse(0, clear: true);
            if (DetailsItem is { } details)
            {
                // The picture and the notes or biography (docs/ui.md, "Singer and album").
                _detailsRequest = S.SendHandled(new CoreCommand.Details(details));
            }
        }

        return Task.CompletedTask;
    }

    protected override void OnActivated()
    {
        if (_pending)
        {
            // Its answer came while the page was not shown: ask again.
            Browse(_requestedPage, clear: _requestedPage == 0);
        }
    }

    protected override void OnNotification(CoreEvent e)
    {
        switch (e)
        {
            case CoreEvent.Details { Page: var d } when d.Request == _detailsRequest && _detailsRequest != 0:
                Description = d.Description?.Trim() ?? string.Empty;
                if (Header is null && d.Image is { } image && Target.Node is { } id && Target.Kind is { } kind)
                {
                    Header = new SourceEntryItem(
                        new SourceItem(Target.Plugin, kind, id, d.Title, d.Subtitle, null, null, image, false, null, null, []), S.L);
                    OnPropertyChanged(nameof(Header));
                    HeaderChanged?.Invoke();
                }

                OnPropertyChanged(nameof(Description));
                OnPropertyChanged(nameof(HasDescription));
                break;
            case CoreEvent.BrowsePage { Update: var u } when u.Request == _request:
                _pending = false;
                IsBrowsing = false;
                Error = u.Error;
                _nextPage = u.NextPage;
                HasNextPage = u.NextPage is not null;
                Add(u.Items);
                IsComplete = u.Error is null && u.NextPage is null;
                Summarize();
                break;
            case CoreEvent.CommandFailed f when f.Id == _request:
                _pending = false;
                IsBrowsing = false;
                IsNotAvailable = SearchViewModel.IsNotAvailableMessage(f.Message);
                Error = IsNotAvailable ? null : f.Message;
                break;
            case CoreEvent.PlaylistCreated created when created.Request == _saveRequest && _saveRequest != 0:
                _saveRequest = 0;
                S.Notifications.Success(S.L.Get("SourceLibrary_Saved"), _savedName);
                break;
            case CoreEvent.CommandFailed f when f.Id == _saveRequest && _saveRequest != 0:
                _saveRequest = 0;
                if (SearchViewModel.IsNotAvailableMessage(f.Message))
                {
                    S.Notifications.Info(S.L.Get("SourceLibrary_SaveTitle"), S.L.Get("SourceLibrary_SaveNotAvailable"));
                }
                else
                {
                    S.Notifications.Error(S.L.Get("SourceLibrary_SaveTitle"), f.Message);
                }

                break;
        }
    }

    /// <summary>Download: the core downloads every song of this album or playlist.</summary>
    [RelayCommand]
    private void Download()
    {
        if (DownloadTarget is { } target)
        {
            DownloadActions.Download(S, target);
        }
    }

    /// <summary>Save as Playlist…: asks for the name (the playlist's own to begin with), then the core copies its songs.</summary>
    [RelayCommand]
    private async Task SaveAsPlaylist()
    {
        if (!CanSaveCopy || Target.Node is not { } id)
        {
            return;
        }

        var name = await S.Platform.PromptTextAsync(S.L.Get("SourceLibrary_SaveTitle"), Title, S.L.Get("SourceLibrary_Save"));
        if (string.IsNullOrWhiteSpace(name))
        {
            return;
        }

        _savedName = name.Trim();
        _saveRequest = S.SendHandled(new CoreCommand.SavePlaylistCopy(Target.Plugin, id, _savedName));
    }

    /// <summary>
    /// A row was chosen: an album, artist, playlist or folder opens its page (or its link, when it
    /// cannot play here and has one); a song plays, with the rest of its album or playlist after it,
    /// or opens its link when it cannot play here.
    /// </summary>
    [RelayCommand]
    private void Open(object? row)
    {
        switch (row)
        {
            case SourceEntryItem entry when entry.OpensPage:
                OpenRequested?.Invoke(SourceTarget.Of(entry.Item));
                break;
            case SourceEntryItem entry:
                ItemLinks.Open(S, entry.Link);
                break;
            case TrackItem { IsPlayable: true } track:
                PlayFrom(track);
                break;
            case TrackItem { OpensLink: true } track:
                ItemLinks.Open(S, track.Link);
                break;
        }
    }

    [RelayCommand]
    private void Play() => PlayAll(shuffle: false);

    [RelayCommand]
    private void Shuffle() => PlayAll(shuffle: true);

    [RelayCommand]
    private void PlayNext()
    {
        if (CanQueue && Target.Node is { } node)
        {
            CollectionActions.PlayNext(S, new Collection.Source(Target.Plugin, node, TargetKind));
        }
    }

    [RelayCommand]
    private void PlayLast()
    {
        if (CanQueue && Target.Node is { } node)
        {
            CollectionActions.PlayLast(S, new Collection.Source(Target.Plugin, node, TargetKind));
        }
    }

    /// <summary>Open in {Service} (More, or the main action when nothing here plays here).</summary>
    [RelayCommand]
    private void OpenLink() => ItemLinks.Open(S, Link);

    [RelayCommand]
    private void LoadMore()
    {
        if (_nextPage is { } page && !IsBrowsing)
        {
            Browse(page, clear: false);
        }
    }

    private void PlayAll(bool shuffle)
    {
        if (!CanPlay)
        {
            return;
        }

        if (Target.Node is { } node)
        {
            // The core fetches every page and queues what plays here, in the source's order (an
            // artist plays its albums).
            CollectionActions.Play(S, new Collection.Source(Target.Plugin, node, TargetKind), shuffle);
        }
    }

    private void PlayFrom(TrackItem track)
    {
        if (IsCollection && Target.Node is { } node && track.SourceIndex is { } index)
        {
            // The whole album or playlist, starting at this song (the core counts in the source's order).
            S.Send(new CoreCommand.PlayCollection(new Collection.Source(Target.Plugin, node, TargetKind), (uint)index, false));
            return;
        }

        var refs = Tracks.Where(t => t.IsPlayable).Select(t => t.Ref).OfType<TrackRef>().ToArray();
        var start = Array.IndexOf(refs, track.Ref);
        if (start >= 0)
        {
            S.Send(new CoreCommand.PlayTracks(refs, (uint)start));
        }
    }

    private void Browse(uint page, bool clear)
    {
        if (clear)
        {
            Rows.Clear();
            _itemCount = 0;
            IsComplete = false;
            Summarize();
        }

        Error = null;
        IsBrowsing = true;
        _pending = true;
        _requestedPage = page;
        _request = S.SendHandled(new CoreCommand.Browse(Target.Plugin, Target.Node, page));
    }

    private void Add(IReadOnlyList<SourceItem> items)
    {
        foreach (var item in items)
        {
            var index = _itemCount++;
            if (item.Kind == ItemKind.Track)
            {
                var track = new TrackItem(Rows.Count) { Localizer = S.L, SourceIndex = index };
                track.Load(item);
                if (IsNumbered)
                {
                    track.Number = (Tracks.Count() + 1).ToString(S.L.Culture);
                }

                Rows.Add(track);
            }
            else
            {
                Rows.Add(new SourceEntryItem(item, S.L));
            }
        }
    }

    private void Summarize()
    {
        var tracks = Tracks.ToList();

        // Play and Shuffle play the whole album, artist, playlist or folder (the source's top level is
        // no collection): offered once something of it plays here.
        CanPlay = Target.Node is not null
            && (SongsListedElsewhere || tracks.Any(t => t.IsPlayable) || (Target.Kind == ItemKind.Artist && Rows.OfType<SourceEntryItem>().Any(e => e.CanPlay)));
        OpensLink = !CanPlay && HasLink && IsComplete && tracks.Count > 0;
        IsNothingHere = IsComplete && Rows.Count == 0 && !SongsListedElsewhere;
        if (IsComplete && tracks.Count > 0)
        {
            var songs = S.L.Plural("Count_Songs", tracks.Count);
            var total = tracks.Aggregate(0UL, (sum, t) => sum + (t.DurationMs ?? 0));
            Facts = IsCollection ? songs : string.Empty;
            Footer = IsCollection ? S.L.Format("Album_Summary", songs, TimeFormat.LongTotal(total, S.L)) : string.Empty;
        }
        else
        {
            Facts = string.Empty;
            Footer = string.Empty;
        }

        OnPropertyChanged(nameof(CanQueue));
        OnPropertyChanged(nameof(HasMore));
    }
}
