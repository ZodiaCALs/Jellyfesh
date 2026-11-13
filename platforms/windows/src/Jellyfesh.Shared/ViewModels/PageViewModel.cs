using CommunityToolkit.Mvvm.ComponentModel;
using Jellyfesh.Core;

namespace Jellyfesh.ViewModels;

/// <summary>
/// A page's state. Pages subscribe to the hub while shown (<see cref="Activate"/>) and reload when
/// the library revision moves, so a page that is not on screen costs nothing.
/// </summary>
public abstract partial class PageViewModel : ObservableObject
{
    private ulong _revision;
    private bool _active;

    [ObservableProperty]
    private bool _isLoading;

    [ObservableProperty]
    private bool _isNotAvailable;

    [ObservableProperty]
    private bool _isEmpty;

    protected PageViewModel(ShellServices services) => S = services;

    protected ShellServices S { get; }

    /// <summary>The page became visible: load (again if the library changed while hidden).</summary>
    public void Activate()
    {
        if (_active)
        {
            return;
        }

        _active = true;
        S.Hub.Changed += OnChanged;
        S.Hub.Notification += OnNotification;
        var revision = S.Hub.State.LibraryRevision;
        if (!Loaded || revision != _revision)
        {
            _revision = revision;
            _ = LoadAsync();
        }
    }

    public void Deactivate()
    {
        if (!_active)
        {
            return;
        }

        _active = false;
        S.Hub.Changed -= OnChanged;
        S.Hub.Notification -= OnNotification;
    }

    protected bool Loaded { get; private set; }

    /// <summary>Loads the page's data; called on activation and after library changes.</summary>
    public async Task LoadAsync()
    {
        IsLoading = true;
        try
        {
            await LoadCoreAsync();
            Loaded = true;
        }
        finally
        {
            IsLoading = false;
        }
    }

    protected abstract Task LoadCoreAsync();

    /// <summary>Records the outcome of a query in the page's flags; true when there is a value.</summary>
    protected bool Accept<T>(QueryResult<T> result)
    {
        IsNotAvailable = result.IsNotAvailable;
        return result.IsOk;
    }

    protected virtual void OnSlices(SliceChange changes, CoreSnapshot state)
    {
    }

    /// <summary>A core notification (search results, sign-in steps, failures) while the page is shown.</summary>
    protected virtual void OnNotification(CoreEvent e)
    {
    }

    private void OnChanged(SliceChange changes)
    {
        var state = S.Hub.State;
        OnSlices(changes, state);
        if (changes.HasFlag(SliceChange.Library) && state.LibraryRevision != _revision)
        {
            _revision = state.LibraryRevision;
            _ = LoadAsync();
        }
    }
}
