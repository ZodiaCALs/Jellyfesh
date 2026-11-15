using System.Collections.ObjectModel;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using Jellyfesh.Core;
using Jellyfesh.Formatting;

namespace Jellyfesh.ViewModels;

/// <summary>A queue entry with what to show for it.</summary>
public sealed partial class QueueEntryItem : ArtworkItem
{
    [ObservableProperty]
    private string _title = string.Empty;

    [ObservableProperty]
    private string _subtitle = string.Empty;

    [ObservableProperty]
    private string _duration = string.Empty;

    public QueueEntryItem(QueueItem item)
    {
        Uid = item.Uid;
        Ref = item.Track;
    }

    public ulong Uid { get; }

    public TrackRef Ref { get; }

    internal bool Resolved { get; private set; }

    internal void Show(string title, string? subtitle, ulong? durationMs, string? artworkHash, string? artworkUrl)
    {
        Title = title;
        Subtitle = subtitle ?? string.Empty;
        Duration = TimeFormat.Duration(durationMs);
        ArtworkHash = artworkHash;
        if (artworkUrl is not null)
        {
            ArtworkUri = artworkUrl;
        }

        Resolved = true;
        OnPropertyChanged(nameof(AccessibleName));
    }

    public string AccessibleName => AlbumItem.Join(Title, Subtitle, Duration);

    public override string ToString() => AccessibleName;
}

/// <summary>
/// The play queue: the current entry and what follows, in playback order. The slice carries the
/// next 50 entries; longer queues page in through QueueWindow as the list scrolls. Reordering by
/// drag sends MoveInQueue; the next Queue slice is the truth.
/// </summary>
public sealed partial class QueueViewModel : ObservableObject
{
    private const uint PageSize = 100;

    private readonly ShellServices _s;
    private readonly Dictionary<long, Track?> _tracks = [];
    private QueueState _queue = QueueState.Initial;
    private bool _loadingMore;

    [ObservableProperty]
    private QueueEntryItem? _current;

    [ObservableProperty]
    private bool _isEmpty = true;

    [ObservableProperty]
    private string _summary = string.Empty;

    public QueueViewModel(ShellServices services) => _s = services;

    /// <summary>The entries after the current one.</summary>
    public ObservableCollection<QueueEntryItem> UpNext { get; } = [];

    /// <summary>More entries exist than are loaded.</summary>
    public bool HasMore => UpNext.Count < RemainingCount;

    private uint RemainingCount => _queue.Position is { } p ? _queue.Length - p - 1 : _queue.Length;

    public void OnQueue(QueueState queue)
    {
        _queue = queue;
        var known = UpNext.ToDictionary(i => i.Uid);
        if (Current is not null)
        {
            known[Current.Uid] = Current;
        }

        Current = queue.Current is { } c ? Reuse(known, c) : null;
        var next = queue.UpNext.Select(i => Reuse(known, i)).ToList();
        Sync(next);
        IsEmpty = queue.Length == 0;
        Summary = IsEmpty ? string.Empty : _s.L.Plural("Count_Songs", queue.Length);
        OnPropertyChanged(nameof(HasMore));
        _ = ResolveAsync([.. (Current is null ? [] : new[] { Current }), .. UpNext]);
    }

    /// <summary>The list showed a row near the end: page in the next entries.</summary>
    public async void LoadMoreIfNeeded(int realizedIndex)
    {
        if (_loadingMore || !HasMore || realizedIndex < UpNext.Count - 20)
        {
            return;
        }

        _loadingMore = true;
        try
        {
            var offset = (_queue.Position is { } p ? p + 1 : 0) + (uint)UpNext.Count;
            var version = _queue.Version;
            var page = await _s.QueryAsync(core => core.QueueWindow(offset, PageSize));
            if (!page.IsOk || version != _queue.Version)
            {
                return;
            }

            var added = new List<QueueEntryItem>();
            foreach (var item in page.Value!)
            {
                if (UpNext.All(u => u.Uid != item.Uid))
                {
                    var entry = new QueueEntryItem(item);
                    UpNext.Add(entry);
                    added.Add(entry);
                }
            }

            OnPropertyChanged(nameof(HasMore));
            await ResolveAsync(added);
        }
        finally
        {
            _loadingMore = false;
        }
    }

    [RelayCommand]
    private void JumpTo(QueueEntryItem? item)
    {
        if (item is not null)
        {
            _s.Send(new CoreCommand.JumpTo(item.Uid));
        }
    }

    [RelayCommand]
    private void Remove(QueueEntryItem? item)
    {
        if (item is not null)
        {
            _s.Send(new CoreCommand.RemoveFromQueue(item.Uid));
        }
    }

    [RelayCommand]
    private void PlayNext(QueueEntryItem? item)
    {
        if (item is not null && UpNext.IndexOf(item) > 0)
        {
            MoveTo(item, 0);
        }
    }

    /// <summary>Clear Queue asks first: playback stops and every song leaves the queue.</summary>
    [RelayCommand]
    private async Task Clear()
    {
        if (!IsEmpty && await _s.Platform.ConfirmAsync(_s.L.Get("Queue_ClearTitle"), _s.L.Get("Queue_ClearBody"), _s.L.Get("Action_Clear"), _s.L.Get("Action_Cancel")))
        {
            _s.Send(new CoreCommand.ClearQueue());
        }
    }

    /// <summary>
    /// The person dropped <paramref name="item"/> at <paramref name="upNextIndex"/> in the up-next
    /// list (the list has already moved it). Converts that to the whole queue's playback order.
    /// </summary>
    public void MoveTo(QueueEntryItem item, int upNextIndex)
    {
        var start = _queue.Position is { } p ? p + 1 : 0;
        var to = (uint)Math.Clamp(start + upNextIndex, 0, Math.Max(0, (int)_queue.Length - 1));
        _s.Send(new CoreCommand.MoveInQueue(item.Uid, to));
    }

    /// <summary>After a drag in the list: finds where the item landed and sends the move.</summary>
    public void CommitDrag(QueueEntryItem item)
    {
        var index = UpNext.IndexOf(item);
        if (index >= 0)
        {
            MoveTo(item, index);
        }
    }

    private static QueueEntryItem Reuse(Dictionary<ulong, QueueEntryItem> known, QueueItem item) =>
        known.TryGetValue(item.Uid, out var existing) ? existing : new QueueEntryItem(item);

    /// <summary>
    /// Updates the list in place so a reorder or a skip moves rows instead of rebuilding them.
    /// Entries paged in beyond the slice's window are dropped: they may be stale after a change,
    /// and scrolling pages them in again.
    /// </summary>
    private void Sync(List<QueueEntryItem> next)
    {
        for (var i = 0; i < next.Count; i++)
        {
            var at = UpNext.IndexOf(next[i]);
            if (at == i)
            {
                continue;
            }

            if (at > i)
            {
                UpNext.Move(at, i);
            }
            else
            {
                UpNext.Insert(i, next[i]);
            }
        }

        while (UpNext.Count > next.Count)
        {
            UpNext.RemoveAt(UpNext.Count - 1);
        }
    }

    private async Task ResolveAsync(IReadOnlyList<QueueEntryItem> items)
    {
        var missing = items
            .Where(i => !i.Resolved)
            .Select(i => i.Ref)
            .OfType<TrackRef.Library>()
            .Select(r => r.Id)
            .Where(id => !_tracks.ContainsKey(id))
            .Distinct()
            .ToArray();
        if (missing.Length > 0)
        {
            var found = await _s.QueryAsync(core => missing.Select(id => (id, track: core.Track(id))).ToArray());
            if (found.IsOk)
            {
                foreach (var (id, track) in found.Value!)
                {
                    _tracks[id] = track;
                }
            }
        }

        foreach (var item in items.Where(i => !i.Resolved))
        {
            switch (item.Ref)
            {
                case TrackRef.Library l when _tracks.GetValueOrDefault(l.Id) is { } t:
                    item.Show(t.Title, t.Artist ?? _s.L.Get("Library_UnknownArtist"), t.DurationMs, t.ArtworkHash, null);
                    break;
                case TrackRef.Library:
                    item.Show(_s.L.Get("Library_MissingTrack"), null, null, null, null);
                    break;
                case TrackRef.File f:
                    item.Show(Path.GetFileNameWithoutExtension(f.Path), Path.GetDirectoryName(f.Path), null, null, null);
                    break;
                case TrackRef.Source s:
                    item.Show(s.Track.Title, s.Track.Artist, s.Track.DurationMs, null, s.Track.ArtworkUrl);
                    break;
            }
        }
    }
}
