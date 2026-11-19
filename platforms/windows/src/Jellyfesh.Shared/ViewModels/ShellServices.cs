using System.Collections.ObjectModel;
using CommunityToolkit.Mvvm.ComponentModel;
using Jellyfesh.Core;
using Jellyfesh.Drawing;
using Jellyfesh.Formatting;
using Jellyfesh.Services;

namespace Jellyfesh.ViewModels;

/// <summary>What every view model needs: the hub, the UI thread, strings, platform actions.</summary>
public sealed class ShellServices
{
    public ShellServices(
        CoreHub hub,
        IUiDispatcher ui,
        ILocalizer localizer,
        IShellPlatform platform,
        IDrawingTime time,
        PlaybackTicker ticker)
    {
        Hub = hub;
        Ui = ui;
        L = localizer;
        Platform = platform;
        Time = time;
        Ticker = ticker;
        Thumbnails = new ThumbnailService(hub.Core);
        Notifications = new NotificationsViewModel(localizer);
        hub.Notification += e =>
        {
            if (e is CoreEvent.Artwork artwork)
            {
                Thumbnails.OnArtwork(artwork.Ready);
            }
        };
    }

    public CoreHub Hub { get; }

    public ICoreClient Core => Hub.Core;

    public IUiDispatcher Ui { get; }

    public ILocalizer L { get; }

    public IShellPlatform Platform { get; }

    public IDrawingTime Time { get; }

    public PlaybackTicker Ticker { get; }

    public ThumbnailService Thumbnails { get; }

    public NotificationsViewModel Notifications { get; }

    private readonly HashSet<ulong> _handled = [];

    public ulong Send(CoreCommand command) => Hub.Send(command);

    /// <summary>
    /// Sends a command whose failure the caller shows itself (a search, a browse, a sign-in), so the
    /// generic error notification stays quiet for it.
    /// </summary>
    public ulong SendHandled(CoreCommand command)
    {
        var id = Hub.Send(command);
        _handled.Add(id);
        return id;
    }

    /// <summary>True for a command sent with <see cref="SendHandled"/>.</summary>
    public bool IsHandled(ulong id) => _handled.Contains(id);

    /// <summary>
    /// Runs a query on the thread pool and returns its result on the caller's context. A feature
    /// not in this build yields <see cref="QueryResult{T}.NotAvailable"/>; other failures are shown
    /// as a notification and yield <see cref="QueryResult{T}.Failed"/>.
    /// </summary>
    public async Task<QueryResult<T>> QueryAsync<T>(Func<ICoreClient, T> query)
    {
        try
        {
            var value = await Task.Run(() => query(Core));
            return QueryResult<T>.Ok(value);
        }
        catch (CoreQueryException e) when (e.IsNotYetAvailable)
        {
            return QueryResult<T>.NotAvailable;
        }
        catch (CoreQueryException e)
        {
            Notifications.Error(L.Get("Error_Query"), e.Message);
            return QueryResult<T>.Failed;
        }
    }
}

/// <summary>The outcome of a query: a value, "not in this build yet", or a failure already reported.</summary>
public readonly record struct QueryResult<T>(T? Value, bool IsOk, bool IsNotAvailable)
{
    public static QueryResult<T> NotAvailable => new(default, false, true);

    public static QueryResult<T> Failed => new(default, false, false);

    public static QueryResult<T> Ok(T value) => new(value, true, false);
}

public enum NotificationSeverity
{
    Informational,
    Success,
    Warning,
    Error,
}

public sealed record Notification(NotificationSeverity Severity, string Title, string Message);

/// <summary>Short messages shown in an InfoBar: command failures, scan results, errors.</summary>
public sealed partial class NotificationsViewModel(ILocalizer l) : ObservableObject
{
    private const int MaxShown = 3;

    public ObservableCollection<Notification> Items { get; } = [];

    public void Info(string title, string message) => Add(new Notification(NotificationSeverity.Informational, title, message));

    public void Success(string title, string message) => Add(new Notification(NotificationSeverity.Success, title, message));

    public void Error(string title, string message) => Add(new Notification(NotificationSeverity.Error, title, message));

    public void Warning(string title, string message) => Add(new Notification(NotificationSeverity.Warning, title, message));

    public void Dismiss(Notification n) => Items.Remove(n);

    /// <summary>Turns core notifications into messages; returns true when it showed one.</summary>
    public bool Show(CoreEvent e)
    {
        switch (e)
        {
            case CoreEvent.CommandFailed f:
                Add(new Notification(NotificationSeverity.Error, l.Get("Error_Command"), f.Message));
                return true;
            case CoreEvent.Error err:
                Add(new Notification(NotificationSeverity.Warning, l.Get("Error_Playback"), err.Message));
                return true;
            case CoreEvent.Scan { Status: ScanStatus.Finished done }:
                var r = done.Report;
                Add(new Notification(
                    r.Errors.Count > 0 ? NotificationSeverity.Warning : NotificationSeverity.Success,
                    l.Get("Scan_Finished"),
                    l.Format("Scan_Report", r.Added, r.Updated, r.Removed, r.Errors.Count)));
                return true;
            case CoreEvent.Scan { Status: ScanStatus.Failed failed }:
                Add(new Notification(NotificationSeverity.Error, l.Get("Scan_Failed"), failed.Message));
                return true;
            default:
                return false;
        }
    }

    private void Add(Notification n)
    {
        // The same message twice in a row (a failing command repeated) shows once.
        if (Items.Count > 0 && Items[^1] == n)
        {
            return;
        }

        Items.Add(n);
        while (Items.Count > MaxShown)
        {
            Items.RemoveAt(0);
        }
    }
}
