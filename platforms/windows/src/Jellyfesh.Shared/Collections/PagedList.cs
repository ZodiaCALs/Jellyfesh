using System.Collections.ObjectModel;
using Jellyfesh.Core;

namespace Jellyfesh.Collections;

/// <summary>A row that starts as a placeholder and fills in when its page arrives.</summary>
public interface IPagedItem<in TModel>
{
    bool IsLoaded { get; }

    void Load(TModel model);
}

/// <summary>
/// A virtualised list backed by a paged core query. With a known count it holds that many
/// placeholders at once (so the scroll bar is right from the start) and loads the pages the list
/// realises; with an unknown count it grows a page at a time as the end comes into view. Pages
/// load on the thread pool and are applied on the UI thread; a list that was replaced (a new
/// library revision) ignores late pages.
/// </summary>
public sealed class PagedList<TItem, TModel>
    where TItem : class, IPagedItem<TModel>
{
    private readonly Func<ulong, ulong, IReadOnlyList<TModel>> _load;
    private readonly Func<int, TItem> _placeholder;
    private readonly IUiDispatcher _ui;
    private readonly HashSet<int> _requested = [];
    private readonly bool _countKnown;
    private int _version;

    public PagedList(
        long? count,
        Func<int, TItem> placeholder,
        Func<ulong, ulong, IReadOnlyList<TModel>> load,
        IUiDispatcher ui,
        int pageSize = 100)
    {
        _load = load;
        _placeholder = placeholder;
        _ui = ui;
        PageSize = pageSize;
        _countKnown = count is not null;
        Items = count is { } n
            ? new ObservableCollection<TItem>(Enumerable.Range(0, (int)Math.Min(n, int.MaxValue)).Select(placeholder))
            : [];
        HasMore = count is null;
    }

    public ObservableCollection<TItem> Items { get; }

    public int PageSize { get; }

    /// <summary>For unknown counts: whether another page may follow.</summary>
    public bool HasMore { get; private set; }

    /// <summary>Raised on the UI thread when a page failed (for example "not available in this build yet").</summary>
    public event Action<CoreQueryException>? Failed;

    /// <summary>Raised on the UI thread after a page was applied.</summary>
    public event Action? PageLoaded;

    /// <summary>The list realised the row at <paramref name="index"/>: make sure its page loads.</summary>
    public void Realize(int index)
    {
        if (_countKnown)
        {
            if (index < 0 || index >= Items.Count || Items[index].IsLoaded)
            {
                return;
            }

            Request(index / PageSize);
        }
        else if (HasMore && index >= Items.Count - (PageSize / 2))
        {
            Request(Items.Count / PageSize);
        }
    }

    /// <summary>Loads the first page (unknown counts) or the page of the first rows.</summary>
    public void Start() => Request(0);

    /// <summary>Forgets loaded pages; late pages from before are ignored.</summary>
    public void Invalidate()
    {
        _version++;
        _requested.Clear();
    }

    private void Request(int page)
    {
        if (!_requested.Add(page))
        {
            return;
        }

        var version = _version;
        var offset = (ulong)page * (ulong)PageSize;
        _ = Task.Run(() =>
        {
            IReadOnlyList<TModel> rows;
            try
            {
                rows = _load(offset, (ulong)PageSize);
            }
            catch (CoreQueryException e)
            {
                _ui.Post(() =>
                {
                    if (version == _version)
                    {
                        HasMore = false;
                        Failed?.Invoke(e);
                    }
                });
                return;
            }

            _ui.Post(() => Apply(version, page, rows));
        });
    }

    private void Apply(int version, int page, IReadOnlyList<TModel> rows)
    {
        if (version != _version)
        {
            return;
        }

        var start = page * PageSize;
        if (!_countKnown)
        {
            while (Items.Count < start + rows.Count)
            {
                Items.Add(_placeholder(Items.Count));
            }

            HasMore = rows.Count == PageSize;
        }

        for (var i = 0; i < rows.Count && start + i < Items.Count; i++)
        {
            Items[start + i].Load(rows[i]);
        }

        PageLoaded?.Invoke();
    }
}
