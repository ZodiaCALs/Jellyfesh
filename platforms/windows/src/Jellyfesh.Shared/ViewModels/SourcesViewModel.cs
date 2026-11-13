using System.Collections.ObjectModel;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using Jellyfesh.Core;

namespace Jellyfesh.ViewModels;

/// <summary>An installed source as the browser lists it.</summary>
public sealed class SourceChoice(PluginInfo plugin, string badgeLabel)
{
    public PluginInfo Plugin { get; } = plugin;

    public string Id => Plugin.Id;

    public string Name => Plugin.Name;

    public bool IsOfficial => Plugin.Badge == PluginBadge.Official;

    public string BadgeLabel { get; } = badgeLabel;

    public string? Account => Plugin.Account;

    public bool CanBrowse => Plugin.Enabled && Plugin.Capabilities.Contains("browse");

    public override string ToString() => Name;
}

/// <summary>One level of a source's catalogue (a breadcrumb shows its title).</summary>
public sealed record BrowseLevel(string? Node, string Title)
{
    public override string ToString() => Title;
}

/// <summary>
/// Browses the catalogues of enabled sources (Command.Browse). Pages stream in as BrowsePage
/// events; the person drills into folders and plays albums or playlists as collections.
/// </summary>
public sealed partial class SourcesViewModel(ShellServices services) : PageViewModel(services)
{
    private ulong _request;
    private uint? _nextPage;

    [ObservableProperty]
    private SourceChoice? _selected;

    [ObservableProperty]
    private bool _isBrowsing;

    [ObservableProperty]
    private string? _error;

    public ObservableCollection<SourceChoice> Choices { get; } = [];

    public ObservableCollection<BrowseLevel> Path { get; } = [];

    public ObservableCollection<TrackItem> Tracks { get; } = [];

    public ObservableCollection<SourceEntryItem> Entries { get; } = [];

    public bool HasNextPage => _nextPage is not null;

    protected override Task LoadCoreAsync()
    {
        Refresh(S.Hub.State.Sources);
        return Task.CompletedTask;
    }

    protected override void OnSlices(SliceChange changes, CoreSnapshot state)
    {
        if (changes.HasFlag(SliceChange.Sources))
        {
            Refresh(state.Sources);
        }
    }

    protected override void OnNotification(CoreEvent e)
    {
        switch (e)
        {
            case CoreEvent.BrowsePage { Update: var u } when u.Request == _request:
                IsBrowsing = false;
                Error = u.Error;
                _nextPage = u.NextPage;
                OnPropertyChanged(nameof(HasNextPage));
                foreach (var item in u.Items)
                {
                    if (item.Kind == ItemKind.Track)
                    {
                        var t = new TrackItem(Tracks.Count) { Localizer = S.L };
                        t.Load(item);
                        Tracks.Add(t);
                    }
                    else
                    {
                        Entries.Add(new SourceEntryItem(item, S.L));
                    }
                }

                break;
            case CoreEvent.CommandFailed f when f.Id == _request:
                IsBrowsing = false;
                IsNotAvailable = f.Message.Contains("not available", StringComparison.OrdinalIgnoreCase);
                Error = IsNotAvailable ? null : f.Message;
                break;
        }
    }

    partial void OnSelectedChanged(SourceChoice? value)
    {
        Path.Clear();
        if (value is not null)
        {
            Path.Add(new BrowseLevel(null, value.Name));
            Browse(null, 0, clear: true);
        }
    }

    [RelayCommand]
    private void Open(SourceEntryItem? entry)
    {
        if (entry is null || Selected is null)
        {
            return;
        }

        if (entry.Item.Kind is ItemKind.Folder or ItemKind.Artist)
        {
            Path.Add(new BrowseLevel(entry.Item.Id, entry.Title));
            Browse(entry.Item.Id, 0, clear: true);
        }
        else if (entry.CanPlay)
        {
            S.Send(new CoreCommand.PlayCollection(new Collection.Source(entry.Item.Source, entry.Item.Id), 0, false));
        }
    }

    /// <summary>Back to a level of the breadcrumb.</summary>
    [RelayCommand]
    private void GoTo(BrowseLevel? level)
    {
        if (level is null)
        {
            return;
        }

        var index = Path.IndexOf(level);
        while (Path.Count > index + 1)
        {
            Path.RemoveAt(Path.Count - 1);
        }

        Browse(level.Node, 0, clear: true);
    }

    [RelayCommand]
    private void LoadMore()
    {
        if (_nextPage is { } page && Path.Count > 0)
        {
            Browse(Path[^1].Node, page, clear: false);
        }
    }

    [RelayCommand]
    private void PlayTrack(TrackItem? item)
    {
        if (item?.Ref is { } r && item.IsPlayable)
        {
            var refs = Tracks.Where(t => t.IsPlayable).Select(t => t.Ref).OfType<TrackRef>().ToArray();
            S.Send(new CoreCommand.PlayTracks(refs, (uint)Math.Max(0, Array.IndexOf(refs, r))));
        }
    }

    private void Browse(string? node, uint page, bool clear)
    {
        if (Selected is null)
        {
            return;
        }

        if (clear)
        {
            Tracks.Clear();
            Entries.Clear();
        }

        Error = null;
        IsBrowsing = true;
        _request = S.SendHandled(new CoreCommand.Browse(Selected.Id, node, page));
    }

    private void Refresh(SourcesState sources)
    {
        var selected = Selected?.Id;
        Choices.Clear();
        foreach (var p in sources.Plugins.Where(p => p.Enabled))
        {
            Choices.Add(new SourceChoice(p, S.L.Get(p.Badge == PluginBadge.Official ? "Source_Official" : "Source_Community")));
        }

        IsEmpty = Choices.Count == 0;
        if (selected is not null && Choices.All(c => c.Id != selected))
        {
            Selected = null;
        }
    }
}
