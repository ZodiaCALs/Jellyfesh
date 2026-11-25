using Jellyfesh.Core;
using Jellyfesh.ViewModels;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Controls.Primitives;

namespace Jellyfesh.App.Shell;

/// <summary>
/// The quality badge's menu (ADR-0026 §3), the device button and More of the player bar and Now
/// Playing's dock (docs/ui.md, "The player bar and the mini player", "Links and remote playback"). The device button lists the output devices, or in a remote session
/// Devices: every device of the service with its kind's icon, the playing one checked; it asks
/// the service for its devices as it opens, and choosing one moves playback there. More holds
/// Open in {Service} and, in a remote session, Stop Controlling.
/// </summary>
internal static class PlayerMenus
{
    /// <summary>
    /// The quality menu above the badge: the format line, then the levels the source offers with
    /// Streaming Quality checked, or why there are none ("Files play as they are.").
    /// </summary>
    public static void ShowQuality(FrameworkElement target, PlayerViewModel player, ShellServices s)
    {
        if (player.Quality is not { } quality)
        {
            return;
        }

        var menu = new MenuFlyout { Placement = FlyoutPlacementMode.Top };
        if (quality.FormatLine is { } line)
        {
            menu.Items.Add(new MenuFlyoutItem { Text = line, IsEnabled = false });
        }

        if (quality.Levels.Count > 0)
        {
            if (menu.Items.Count > 0)
            {
                menu.Items.Add(new MenuFlyoutSeparator());
            }

            menu.Items.Add(new MenuFlyoutItem { Text = s.L.Get("Quality_Menu"), IsEnabled = false, FontWeight = Microsoft.UI.Text.FontWeights.SemiBold });
            foreach (var level in quality.Levels)
            {
                var item = new RadioMenuFlyoutItem
                {
                    Text = QualityMenu.LevelName(level, s.L),
                    GroupName = "StreamingQuality",
                    IsChecked = level == quality.Chosen,
                };
                var choice = level;
                item.Click += (_, _) => player.ChooseQualityCommand.Execute(choice);
                menu.Items.Add(item);
            }
        }
        else if (quality.Note is { } note)
        {
            menu.Items.Add(new MenuFlyoutItem { Text = note, IsEnabled = false });
        }

        menu.ShowAt(target);
    }

    /// <summary>Opens the output devices or, in a remote session, Devices above <paramref name="target"/>.</summary>
    public static void ShowDevices(FrameworkElement target, PlayerViewModel player, ShellServices s)
    {
        var menu = new MenuFlyout { Placement = FlyoutPlacementMode.TopEdgeAlignedRight };
        if (player.IsRemote)
        {
            FillRemote(menu, player, s.L);

            // The answer to Refresh arrives as a Remote slice: the open menu follows it.
            void OnChanged(object? sender, System.Collections.Specialized.NotifyCollectionChangedEventArgs e) => FillRemote(menu, player, s.L);
            player.RemoteDevices.CollectionChanged += OnChanged;
            menu.Closed += (_, _) => player.RemoteDevices.CollectionChanged -= OnChanged;
            player.RefreshRemoteDevices();
        }
        else
        {
            foreach (var device in player.Devices)
            {
                var item = new RadioMenuFlyoutItem
                {
                    Text = device.Name,
                    GroupName = "OutputDevices",
                    IsChecked = Equals(device, player.SelectedDevice),
                };
                var choice = device;
                item.Click += (_, _) => player.SelectedDevice = choice;
                menu.Items.Add(item);
            }

            // A good moment to look again: the core never polls for devices (ADR-0009).
            s.Send(new CoreCommand.RefreshDevices());
        }

        menu.ShowAt(target);
    }

    /// <summary>More: Open in {Service} when the song has a link, Stop Controlling in a remote session.</summary>
    public static void ShowMore(FrameworkElement target, PlayerViewModel player, ShellServices s)
    {
        var menu = new MenuFlyout { Placement = FlyoutPlacementMode.TopEdgeAlignedRight };
        if (player.HasLink)
        {
            var open = new MenuFlyoutItem { Text = player.OpenLinkLabel, Icon = new FontIcon { Glyph = "" } };
            open.Click += (_, _) => player.OpenLinkCommand.Execute(null);
            menu.Items.Add(open);
        }

        if (player.IsRemote)
        {
            var stop = new MenuFlyoutItem { Text = s.L.Get("Remote_StopControlling") };
            stop.Click += (_, _) => player.StopControllingCommand.Execute(null);
            menu.Items.Add(stop);
        }

        // Lyrics AI Lyrics made: Make Again, Delete Made Lyrics (asks first).
        var ai = AppHost.App.AiLyrics;
        if (ai.HasMadeLyrics)
        {
            if (menu.Items.Count > 0)
            {
                menu.Items.Add(new MenuFlyoutSeparator());
            }

            var again = new MenuFlyoutItem { Text = s.L.Get("AiLyrics_MakeAgainLabel") };
            again.Click += (_, _) => ai.MakeAgainCommand.Execute(null);
            menu.Items.Add(again);
            var delete = new MenuFlyoutItem { Text = s.L.Get("AiLyrics_DeleteMade") };
            delete.Click += (_, _) => ai.DeleteMadeCommand.Execute(null);
            menu.Items.Add(delete);
        }

        // Desktop Lyrics: open or close it, and unlock it while clicks pass through it.
        if (menu.Items.Count > 0)
        {
            menu.Items.Add(new MenuFlyoutSeparator());
        }

        var settings = s.Hub.State.Settings;
        var desktop = new ToggleMenuFlyoutItem { Text = s.L.Get("DesktopLyrics_Title"), IsChecked = settings.DesktopLyrics };
        desktop.Click += (_, _) => s.Send(new Core.CoreCommand.SetSettings(s.Hub.State.Settings with { DesktopLyrics = !settings.DesktopLyrics }));
        menu.Items.Add(desktop);
        if (AppHost.Window.DesktopLyrics is { IsLocked: true } window)
        {
            var unlock = new MenuFlyoutItem { Text = s.L.Get("DesktopLyrics_Unlock") };
            unlock.Click += (_, _) => window.Lock(false);
            menu.Items.Add(unlock);
        }

        menu.ShowAt(target);
    }

    /// <summary>Devices: its title, then each device with its kind's icon, the playing one checked.</summary>
    private static void FillRemote(MenuFlyout menu, PlayerViewModel player, Jellyfesh.Formatting.ILocalizer l)
    {
        menu.Items.Clear();
        menu.Items.Add(new MenuFlyoutItem { Text = l.Get("Remote_Devices"), IsEnabled = false, FontWeight = Microsoft.UI.Text.FontWeights.SemiBold });
        menu.Items.Add(new MenuFlyoutSeparator());
        foreach (var device in player.RemoteDevices)
        {
            var item = new RadioMenuFlyoutItem
            {
                Text = device.Name,
                GroupName = "RemoteDevices",
                IsChecked = device.IsPlaying,
                Icon = new FontIcon { Glyph = KindGlyph(device.Kind) },
            };
            var choice = device;
            item.Click += (_, _) => player.TransferToCommand.Execute(choice);
            menu.Items.Add(item);
        }
    }

    /// <summary>Segoe Fluent Icons (Segoe MDL2 Assets on Windows 10) for each kind of device.</summary>
    private static string KindGlyph(RemoteDeviceKind kind) => kind switch
    {
        RemoteDeviceKind.Computer => "",   // DeviceLaptopNoPic
        RemoteDeviceKind.Smartphone => "", // CellPhone
        RemoteDeviceKind.Speaker => "",    // Speakers
        RemoteDeviceKind.Tv => "",         // TVMonitor
        RemoteDeviceKind.Car => "",        // Car
        _ => "",                           // Devices
    };
}
