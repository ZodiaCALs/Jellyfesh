using System.Collections.ObjectModel;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using Jellyfesh.Core;
using Jellyfesh.Formatting;

namespace Jellyfesh.ViewModels;

/// <summary>One shelf of a Discover page: its title, its style (a row, a grid, a chart), its items and See All.</summary>
public sealed class ShelfViewModel
{
    public ShelfViewModel(Shelf shelf, ILocalizer l)
    {
        Shelf = shelf;
        Title = Words(shelf, l);
        var number = 0;
        foreach (var item in shelf.Items)
        {
            number++;
            if (item.Kind == ItemKind.Track)
            {
                var track = new TrackItem(Items.Count) { Localizer = l, SourceIndex = number - 1 };
                track.Load(item);
                if (shelf.Style == ShelfStyle.Chart)
                {
                    track.Number = number.ToString(l.Culture);
                }

                Items.Add(track);
            }
            else
            {
                Items.Add(new SourceEntryItem(item, l));
            }
        }
    }

    public Shelf Shelf { get; }

    public string Title { get; }

    /// <summary>SourceEntryItem and TrackItem rows, in the shelf's order.</summary>
    public ObservableCollection<object> Items { get; } = [];

    public bool IsRow => Shelf.Style == ShelfStyle.Row;

    public bool IsGrid => Shelf.Style == ShelfStyle.Grid;

    public bool IsChart => Shelf.Style == ShelfStyle.Chart;

    /// <summary>See All: the shelf has a node to browse.</summary>
    public bool HasMore => Shelf.More is not null;

    public bool IsEmpty => Items.Count == 0;

    /// <summary>
    /// The shelf's title: the words for a shelf the core makes (its stable id: For You, Recently
    /// Added, Most Played, Rediscover), else the plugin's own title.
    /// </summary>
    public static string Words(Shelf shelf, ILocalizer l)
    {
        var key = "Shelf_" + shelf.Id;
        var word = l.Get(key);
        return word != key && !string.IsNullOrEmpty(word) ? word : shelf.Title;
    }
}

/// <summary>
/// A source's Discover page (docs/ui.md, "Discover"; docs/core-api.md, "Discover"): its shelves from
/// the core's answer (Discover, by request id), placeholders while they load, the error with Try
/// Again, See All for a shelf with more, and Browse for the source's own folders. Choosing an album,
/// artist or playlist opens its page; a song plays with the songs of its shelf.
/// </summary>
public sealed partial class DiscoverViewModel : PageViewModel
{
    /// <summary>The library's Discover page (Home's sections).</summary>
    public const string Library = "library";

    private ulong _request;

    [ObservableProperty]
    private bool _isLoading;

    [ObservableProperty]
    private string? _error;

    public DiscoverViewModel(ShellServices services, string source, string title)
        : base(services)
    {
        Source = source;
        Title = title;
    }

    public string Source { get; }

    public string Title { get; }

    public ObservableCollection<ShelfViewModel> Shelves { get; } = [];

    /// <summary>The source's status under its name: "Signed in as …", or why it is off.</summary>
    public string Status => S.Hub.State.Sources.Find(Source) is { } plugin
        ? plugin.Account is { } account ? S.L.Format("Source_SignedInAs", account) : plugin.Enabled ? string.Empty : S.L.Get("Source_StatusOff")
        : string.Empty;

    public bool HasError => !string.IsNullOrEmpty(Error);

    /// <summary>Raised to open a page of the source (an album, an artist, a shelf's See All, Browse).</summary>
    public event Action<SourceTarget>? OpenRequested;

    partial void OnErrorChanged(string? value) => OnPropertyChanged(nameof(HasError));

    protected override Task LoadCoreAsync()
    {
        Shelves.Clear();
        Error = null;
        IsLoading = true;
        _request = S.SendHandled(new CoreCommand.Discover(Source));
        return Task.CompletedTask;
    }

    protected override void OnNotification(CoreEvent e)
    {
        switch (e)
        {
            case CoreEvent.Discover d when d.Page.Request == _request:
                IsLoading = false;
                Shelves.Clear();
                foreach (var shelf in d.Page.Shelves.Where(s => s.Items.Count > 0 || s.More is not null))
                {
                    Shelves.Add(new ShelfViewModel(shelf, S.L));
                }

                Error = d.Page.Error;
                IsEmpty = Shelves.Count == 0 && !HasError;
                break;
            case CoreEvent.CommandFailed f when f.Id == _request:
                IsLoading = false;
                Error = f.Message;
                break;
        }
    }

    [RelayCommand]
    private Task TryAgain() => LoadAsync();

    /// <summary>Browse: the source's own folders.</summary>
    [RelayCommand]
    private void Browse()
    {
        if (S.Hub.State.Sources.Find(Source) is { } plugin)
        {
            OpenRequested?.Invoke(SourceTarget.Root(plugin));
        }
    }

    /// <summary>See All: the shelf's node, browsed.</summary>
    [RelayCommand]
    private void SeeAll(ShelfViewModel? shelf)
    {
        if (shelf?.Shelf.More is { } more)
        {
            OpenRequested?.Invoke(new SourceTarget(Source, more, shelf.Title, null));
        }
    }

    /// <summary>An item of a shelf: an album, artist or playlist opens its page; a song plays with its shelf's songs.</summary>
    public void Open(ShelfViewModel shelf, object? row)
    {
        switch (row)
        {
            case SourceEntryItem entry when entry.OpensPage:
                OpenRequested?.Invoke(SourceTarget.Of(entry.Item));
                break;
            case SourceEntryItem entry when entry.Link is not null:
                ItemLinks.Open(S, entry.Link);
                break;
            case TrackItem { IsPlayable: true, Ref: not null } track:
                var tracks = shelf.Items.OfType<TrackItem>().Where(t => t.IsPlayable && t.Ref is not null).ToList();
                S.Send(new CoreCommand.PlayTracks(tracks.Select(t => t.Ref!).ToArray(), (uint)Math.Max(0, tracks.IndexOf(track))));
                break;
            case TrackItem { Link: { } link }:
                ItemLinks.Open(S, link);
                break;
        }
    }
}
