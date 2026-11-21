using System.Collections.ObjectModel;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using Jellyfesh.Core;
using Jellyfesh.Formatting;

namespace Jellyfesh.ViewModels;

/// <summary>A comment: the author and their picture (through the core), when it was written, its likes, its text.</summary>
public sealed class CommentItem : ArtworkItem
{
    public CommentItem(CommentEntry entry, ILocalizer l, long nowMs)
    {
        Entry = entry;
        Author = entry.Author;
        Text = entry.Text;
        When = entry.TimeMs is { } ms ? CommentsViewModel.Age(nowMs - ms, l) : string.Empty;
        Likes = entry.Likes is > 0 and { } likes ? l.Format("Comments_Likes", likes.ToString("N0", l.Culture)) : string.Empty;
        SetArtwork(entry.Avatar, null);
    }

    public CommentEntry Entry { get; }

    public string Author { get; }

    public string Text { get; }

    /// <summary>"3 days ago".</summary>
    public string When { get; }

    /// <summary>"7 likes", or empty.</summary>
    public string Likes { get; }

    public string AccessibleName => string.Join(", ", new[] { Author, When, Likes, Text }.Where(s => s.Length > 0));

    public override string ToString() => AccessibleName;
}

/// <summary>
/// Now Playing's Comments pane (docs/ui.md, "Comments"), for songs from sources that have comments
/// (PluginSummary.comments): "Comments from {Service}", "Popular" (the first page's hot comments)
/// then "Latest", more at the end of the list, "No Comments Yet" without any. Read-only; asked of
/// the core by request id (Comments), never stored.
/// </summary>
public sealed partial class CommentsViewModel : ObservableObject
{
    private readonly ShellServices _s;
    private readonly Func<long> _nowMs;
    private ItemRef? _item;
    private ulong _request;
    private string? _next;
    private bool _loaded;

    [ObservableProperty]
    private bool _isAvailable;

    [ObservableProperty]
    private bool _isLoading;

    [ObservableProperty]
    private string _error = string.Empty;

    [ObservableProperty]
    private string _attribution = string.Empty;

    public CommentsViewModel(ShellServices services, Func<long>? nowMs = null)
    {
        _s = services;
        _nowMs = nowMs ?? (() => DateTimeOffset.UtcNow.ToUnixTimeMilliseconds());
        _s.Hub.Notification += OnNotification;
    }

    public ObservableCollection<CommentItem> Popular { get; } = [];

    public ObservableCollection<CommentItem> Latest { get; } = [];

    public bool HasPopular => Popular.Count > 0;

    public bool HasMore => _next is not null;

    /// <summary>"No Comments Yet": the source answered with none.</summary>
    public bool IsEmpty => _loaded && !IsLoading && Error.Length == 0 && Popular.Count == 0 && Latest.Count == 0;

    /// <summary>The song playing changed: its comments, when its source has them, are asked for when the pane opens.</summary>
    public void Update(CoreSnapshot state)
    {
        var item = state.NowPlaying?.Track is TrackRef.Source s
            && !state.Remote.HasSession
            && state.Sources.Find(s.Track.Plugin) is { Comments: true }
                ? new ItemRef.Source(s.Track.Plugin, ItemKind.Track, s.Track.Id)
                : null;
        if (item == _item)
        {
            return;
        }

        _item = item;
        IsAvailable = item is not null;
        Reset();
    }

    /// <summary>The pane opened: the first page, once per song.</summary>
    public void Open()
    {
        if (_item is not null && !IsLoading && Popular.Count == 0 && Latest.Count == 0 && Error.Length == 0)
        {
            Ask(null);
        }
    }

    /// <summary>More at the end of the list.</summary>
    [RelayCommand]
    private void LoadMore()
    {
        if (_next is { } next && !IsLoading)
        {
            Ask(next);
        }
    }

    /// <summary>"3 days ago", "5 hours ago", "Just now".</summary>
    public static string Age(long ms, ILocalizer l)
    {
        var age = TimeSpan.FromMilliseconds(Math.Max(0, ms));
        return age switch
        {
            { TotalMinutes: < 1 } => l.Get("Comments_JustNow"),
            { TotalHours: < 1 } => l.Plural("Comments_Minutes", (long)age.TotalMinutes),
            { TotalDays: < 1 } => l.Plural("Comments_Hours", (long)age.TotalHours),
            { TotalDays: < 365 } => l.Plural("Comments_Days", (long)age.TotalDays),
            _ => l.Plural("Comments_Years", (long)(age.TotalDays / 365)),
        };
    }

    private void Ask(string? page)
    {
        IsLoading = true;
        Error = string.Empty;
        _request = _s.SendHandled(new CoreCommand.Comments(_item!, page));
        OnPropertyChanged(nameof(IsEmpty));
    }

    private void Reset()
    {
        _request = 0;
        _next = null;
        _loaded = false;
        Popular.Clear();
        Latest.Clear();
        Error = string.Empty;
        Attribution = string.Empty;
        IsLoading = false;
        OnPropertyChanged(nameof(HasMore));
        OnPropertyChanged(nameof(HasPopular));
        OnPropertyChanged(nameof(IsEmpty));
    }

    private void OnNotification(CoreEvent e)
    {
        switch (e)
        {
            case CoreEvent.Comments { Page: var page } when page.Request == _request && _request != 0 && page.Item == _item:
                var now = _nowMs();
                foreach (var hot in page.Hot)
                {
                    Popular.Add(new CommentItem(hot, _s.L, now));
                }

                foreach (var comment in page.Comments)
                {
                    Latest.Add(new CommentItem(comment, _s.L, now));
                }

                _next = page.Next;
                _loaded = true;
                Attribution = page.Attribution is { Length: > 0 } a ? _s.L.Format("Comments_From", a) : string.Empty;
                Error = page.Error ?? string.Empty;
                _request = 0;
                IsLoading = false;
                OnPropertyChanged(nameof(HasMore));
                OnPropertyChanged(nameof(HasPopular));
                OnPropertyChanged(nameof(IsEmpty));
                break;
            case CoreEvent.CommandFailed f when f.Id == _request && _request != 0:
                Error = f.Message;
                _request = 0;
                IsLoading = false;
                OnPropertyChanged(nameof(IsEmpty));
                break;
        }
    }
}
