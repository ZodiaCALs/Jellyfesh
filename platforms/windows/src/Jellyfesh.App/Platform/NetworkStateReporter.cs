using Jellyfesh.Core;
using Microsoft.UI.Dispatching;
using Windows.Networking.Connectivity;

namespace Jellyfesh.App.Platform;

/// <summary>
/// Tells the core whether the PC is online and whether its connection is metered
/// (Command.SetNetworkState; ADR-0025 §2, ADR-0026 §1): at start and after every change Windows
/// reports (NetworkInformation.NetworkStatusChanged), coalesced, and only when it differs from what
/// was sent last. The core decides what follows: Auto streaming quality asks High while metered,
/// and downloads wait offline or, with Wi-Fi Only, while metered.
/// </summary>
internal sealed class NetworkStateReporter : IDisposable
{
    private readonly Action<CoreCommand> _send;
    private readonly DispatcherQueueTimer _debounce;
    private readonly NetworkStatusChangedEventHandler _changed;
    private (bool Online, bool Metered)? _sent;

    public NetworkStateReporter(DispatcherQueue queue, Action<CoreCommand> send)
    {
        _send = send;
        _debounce = queue.CreateTimer();
        _debounce.IsRepeating = false;
        _debounce.Interval = TimeSpan.FromMilliseconds(500);
        _debounce.Tick += (_, _) =>
        {
            _debounce.Stop();
            Report();
        };

        // Raised on a thread pool thread, often several times for one change.
        _changed = _ => queue.TryEnqueue(Kick);
        NetworkInformation.NetworkStatusChanged += _changed;
        Report();
    }

    public void Dispose()
    {
        NetworkInformation.NetworkStatusChanged -= _changed;
        _debounce.Stop();
    }

    private void Kick()
    {
        _debounce.Stop();
        _debounce.Start();
    }

    private void Report()
    {
        (bool Online, bool Metered) state;
        try
        {
            state = Read();
        }
        catch (Exception e)
        {
            Shell.AppLog.Write("reading the network state", e);
            return;
        }

        if (state == _sent)
        {
            return;
        }

        _sent = state;
        _send(new CoreCommand.SetNetworkState(state.Online, state.Metered));
    }

    /// <summary>
    /// Online: the internet connection profile reaches the internet. Metered: Windows says the
    /// connection costs by the byte or up to a limit, or it roams, or it is near or over its data
    /// limit (the tests Windows documents for metered connections).
    /// </summary>
    private static (bool Online, bool Metered) Read()
    {
        var profile = NetworkInformation.GetInternetConnectionProfile();
        if (profile is null)
        {
            return (false, false);
        }

        var online = profile.GetNetworkConnectivityLevel() == NetworkConnectivityLevel.InternetAccess;
        var cost = profile.GetConnectionCost();
        var metered = cost.NetworkCostType is NetworkCostType.Fixed or NetworkCostType.Variable
            || cost.Roaming || cost.OverDataLimit || cost.ApproachingDataLimit;
        return (online, metered);
    }
}
