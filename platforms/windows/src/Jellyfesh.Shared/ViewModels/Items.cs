using CommunityToolkit.Mvvm.ComponentModel;
using Jellyfesh.Collections;
using Jellyfesh.Core;
using Jellyfesh.Formatting;
using Jellyfesh.Services;

namespace Jellyfesh.ViewModels;

/// <summary>
/// A row with artwork. The view asks for the artwork when it realises the row and cancels when
/// the row is recycled, so thumbnails load off the UI thread and only for what is on screen.
/// </summary>
public abstract partial class ArtworkItem : ObservableObject
{
    private CancellationTokenSource? _artwork;
    private (ThumbnailService Service, double Pixels)? _wanted;
    private string? _artworkHash;

    [ObservableProperty]
    private string? _artworkUri;

    /// <summary>
    /// The library artwork hash (for thumbnails), when the row has one. A placeholder row that was
    /// realised before its page arrived loads its thumbnail as soon as the hash is known.
    /// </summary>
    public string? ArtworkHash
    {
        get => _artworkHash;
        protected set
        {
            _artworkHash = value;
            if (_wanted is { } wanted && value is not null && ArtworkUri is null)
            {
                RequestArtwork(wanted.Service, wanted.Pixels);
            }
        }
    }

    /// <summary>Loads the thumbnail at about <paramref name="pixels"/> device pixels (now, or once the hash is known).</summary>
    public async void RequestArtwork(ThumbnailService thumbnails, double pixels)
    {
        _wanted = (thumbnails, pixels);
        if (ArtworkUri is not null || ArtworkHash is null || _artwork is not null)
        {
            return;
        }

        var cts = new CancellationTokenSource();
        _artwork = cts;
        try
        {
            var path = await thumbnails.GetAsync(ArtworkHash, ThumbnailService.SizeFor(pixels), cts.Token);
            if (!cts.IsCancellationRequested && path is not null && FileUris.From(path) is { } uri)
            {
                ArtworkUri = uri;
            }
        }
        catch (OperationCanceledException)
        {
            // The row scrolled away.
        }
        catch (Exception e)
        {
            // async void: an exception would end the app. The row keeps its placeholder.
            System.Diagnostics.Debug.WriteLine($"thumbnail {ArtworkHash}: {e.Message}");
        }
        finally
        {
            if (ReferenceEquals(_artwork, cts))
            {
                _artwork = null;
            }

            cts.Dispose();
        }
    }

    /// <summary>The row was recycled: stop waiting for its thumbnail.</summary>
    public void CancelArtwork()
    {
        _wanted = null;
        _artwork?.Cancel();
        _artwork = null;
    }

    protected void SetArtwork(ArtworkRef? artwork, string? hash)
    {
        ArtworkUri = artwork?.ToUriString();
        ArtworkHash = hash;
    }
}

public sealed partial class AlbumItem : ArtworkItem, IPagedItem<Album>
{
    [ObservableProperty]
    private string _title = string.Empty;

    [ObservableProperty]
    private string _subtitle = string.Empty;

    [ObservableProperty]
    private bool _isLoaded;

    public AlbumItem(int index) => Index = index;

    public AlbumItem(Album album, ILocalizer l)
        : this(-1)
    {
        Load(album, l);
    }

    public int Index { get; }

    public long Id { get; private set; }

    public Album? Model { get; private set; }

    public ILocalizer? Localizer { get; set; }

    public void Load(Album model) => Load(model, Localizer ?? KeyLocalizer.Instance);

    public void Load(Album model, ILocalizer l)
    {
        Model = model;
        Id = model.Id;
        Title = model.Title;
        var artist = model.AlbumArtist ?? l.Get("Library_UnknownArtist");
        Subtitle = model.Year is { } year ? l.Format("Album_ArtistYear", artist, year) : artist;
        ArtworkHash = model.ArtworkHash;
        IsLoaded = true;
        OnPropertyChanged(nameof(AccessibleName));
    }

    /// <summary>"Title, Artist · Year" for Narrator.</summary>
    public string AccessibleName => Join(Title, Subtitle);

    public override string ToString() => AccessibleName;

    internal static string Join(params string?[] parts) =>
        string.Join(", ", parts.Where(p => !string.IsNullOrEmpty(p)));
}

public sealed partial class ArtistItem : ObservableObject, IPagedItem<Artist>
{
    [ObservableProperty]
    private string _name = string.Empty;

    [ObservableProperty]
    private string _subtitle = string.Empty;

    [ObservableProperty]
    private bool _isLoaded;

    public ArtistItem(int index) => Index = index;

    public int Index { get; }

    public long Id { get; private set; }

    public Artist? Model { get; private set; }

    public ILocalizer? Localizer { get; set; }

    /// <summary>The first letter, for the round monogram that stands in for artist images.</summary>
    public string Monogram => Name.Length > 0 ? char.ToUpperInvariant(Name[0]).ToString() : string.Empty;

    public void Load(Artist model)
    {
        var l = Localizer ?? KeyLocalizer.Instance;
        Model = model;
        Id = model.Id;
        Name = model.Name;
        Subtitle = l.Plural("Count_Songs", model.TrackCount);
        OnPropertyChanged(nameof(Monogram));
        IsLoaded = true;
        OnPropertyChanged(nameof(AccessibleName));
    }

    public string AccessibleName => AlbumItem.Join(Name, Subtitle);

    public override string ToString() => AccessibleName;
}

/// <summary>A track row from the library, a playlist, favourites, history or a source.</summary>
public sealed partial class TrackItem : ArtworkItem, IPagedItem<Track>, IPagedItem<TrackRow>, IPagedItem<PlaylistEntry>
{
    [ObservableProperty]
    private string _title = string.Empty;

    [ObservableProperty]
    private string _subtitle = string.Empty;

    [ObservableProperty]
    private string _duration = string.Empty;

    [ObservableProperty]
    private string _number = string.Empty;

    [ObservableProperty]
    private bool _isLoaded;

    [ObservableProperty]
    private bool _isFavorite;

    [ObservableProperty]
    private bool _isMissing;

    [ObservableProperty]
    private bool _isCurrent;

    [ObservableProperty]
    private bool _isPlayable = true;

    [ObservableProperty]
    private string? _origin;

    public TrackItem(int index) => Index = index;

    public int Index { get; }

    public TrackRef? Ref { get; private set; }

    /// <summary>The playlist entry id, for rows of a playlist.</summary>
    public long? EntryId { get; private set; }

    /// <summary>Why a source item cannot play here (region, paywall); never bypassed.</summary>
    public string? UnplayableReason { get; private set; }

    /// <summary>The reason shown on the row, beside its other facts: only while it cannot play.</summary>
    public string? UnplayableNote => IsPlayable ? null : UnplayableReason;

    /// <summary>The source the row comes from (its plugin id), for Open in {Service}; null for library rows.</summary>
    public string? SourceId { get; private set; }

    /// <summary>Opens the song in its service (Open in {Service}), when its source declares links.</summary>
    public string? Link { get; private set; }

    /// <summary>It cannot play here but has a link: choosing the row opens the link (docs/ui.md).</summary>
    public bool OpensLink => !IsPlayable && Link is not null;

    public ILocalizer? Localizer { get; set; }

    /// <summary>"Title, Artist, 3:52" for Narrator (and the source, for search results; why it cannot play).</summary>
    public string AccessibleName => string.Join(", ", new[] { Title, Subtitle, Duration, Origin, UnplayableNote }.Where(s => !string.IsNullOrEmpty(s)));

    public void Load(Track model)
    {
        var l = Localizer ?? KeyLocalizer.Instance;
        Ref = new TrackRef.Library(model.Id);
        Title = model.Title;
        Subtitle = model.Artist ?? l.Get("Library_UnknownArtist");
        Duration = TimeFormat.Duration(model.DurationMs);
        Number = model.TrackNo?.ToString(l.Culture) ?? string.Empty;
        SetArtwork(null, model.ArtworkHash);
        Done();
    }

    public void Load(TrackRow row)
    {
        var l = Localizer ?? KeyLocalizer.Instance;
        Ref = row.Track;
        Title = row.Title;
        Subtitle = row.Artist ?? l.Get("Library_UnknownArtist");
        Duration = TimeFormat.Duration(row.DurationMs);
        IsFavorite = row.Favorite;
        IsMissing = row.Missing;
        IsPlayable = !row.Missing;
        SetArtwork(row.Artwork, row.ArtworkHash);
        Done();
    }

    public void Load(PlaylistEntry entry)
    {
        EntryId = entry.EntryId;
        Load(entry.Row);
    }

    public void Load(SourceItem item)
    {
        var l = Localizer ?? KeyLocalizer.Instance;
        Ref = item.Track;
        Title = item.Title;
        Subtitle = item.Subtitle ?? string.Empty;
        Duration = TimeFormat.Duration(item.DurationMs);
        IsPlayable = item.Playable && item.Track is not null;
        UnplayableReason = item.UnplayableReason ?? (item.Playable ? null : l.Get("Track_Unplayable"));
        SourceId = item.Source;
        Link = ItemLinks.Usable(item.Link);
        SetArtwork(item.Artwork, null);
        Done();
    }

    partial void OnOriginChanged(string? value) => OnPropertyChanged(nameof(AccessibleName));

    public override string ToString() => AccessibleName;

    private void Done()
    {
        IsLoaded = true;
        OnPropertyChanged(nameof(UnplayableNote));
        OnPropertyChanged(nameof(AccessibleName));
    }
}

public sealed partial class PlaylistItem : ArtworkItem
{
    public PlaylistItem(PlaylistSummary model, ILocalizer l)
    {
        Model = model;
        Id = model.Id;
        Name = model.Name;
        Subtitle = l.Format("Playlist_Summary", l.Plural("Count_Songs", model.EntryCount), TimeFormat.Total(model.DurationMs, l));
        ArtworkHash = model.ArtworkHash;
        EntryCount = model.EntryCount;
    }

    public PlaylistSummary Model { get; }

    public long Id { get; }

    public string Name { get; }

    public string Subtitle { get; }

    public uint EntryCount { get; }

    public override string ToString() => AlbumItem.Join(Name, Subtitle);
}

/// <summary>A search or browse result that is not a track (album, artist, playlist, folder).</summary>
public sealed class SourceEntryItem : ArtworkItem
{
    public SourceEntryItem(SourceItem item, ILocalizer l)
    {
        Item = item;
        Title = item.Title;
        Subtitle = item.Subtitle ?? string.Empty;
        KindLabel = l.Get("Kind_" + item.Kind);
        Link = ItemLinks.Usable(item.Link);
        SetArtwork(item.Artwork, null);
    }

    /// <summary>Opens the album, artist, playlist or folder in its service (Open in {Service}).</summary>
    public string? Link { get; }

    public bool HasLink => Link is not null;

    public SourceItem Item { get; }

    public string Title { get; }

    public string Subtitle { get; }

    public string KindLabel { get; }

    /// <summary>The sources that have this entry, for search results ("Jellyfin · Subsonic").</summary>
    public string? Origin { get; init; }

    public bool CanPlay => Item.Playable && Item.Kind is ItemKind.Album or ItemKind.Playlist;

    public string AccessibleName => AlbumItem.Join(Title, Subtitle, KindLabel, Origin);

    public override string ToString() => AccessibleName;
}
