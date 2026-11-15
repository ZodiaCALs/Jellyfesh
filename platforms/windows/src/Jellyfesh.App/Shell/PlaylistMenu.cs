using Jellyfesh.Core;
using Jellyfesh.ViewModels;
using Microsoft.UI.Xaml.Controls;

namespace Jellyfesh.App.Shell;

/// <summary>
/// Add to Playlist…: a submenu with New Playlist… (it asks for the name first) and the person's
/// playlists, filled each time its menu opens. The songs come from the caller when one is chosen,
/// so an album, an artist or a playlist is only read then.
/// </summary>
internal static class PlaylistMenu
{
    public static async Task FillAsync(MenuFlyoutSubItem submenu, ShellServices s, Func<Task<IReadOnlyList<TrackRef>>> tracks)
    {
        submenu.Items.Clear();
        var create = new MenuFlyoutItem { Text = s.L.Get("Menu_NewPlaylist") };
        create.Click += async (_, _) => await PlaylistActions.CreateAsync(s, await tracks());
        submenu.Items.Add(create);
        var playlists = await s.QueryAsync(c => c.Playlists());
        if (!playlists.IsOk || playlists.Value!.Count == 0)
        {
            return;
        }

        submenu.Items.Add(new MenuFlyoutSeparator());
        foreach (var playlist in playlists.Value!)
        {
            var id = playlist.Id;
            var item = new MenuFlyoutItem { Text = playlist.Name };
            item.Click += async (_, _) => PlaylistActions.AddTo(s, id, await tracks());
            submenu.Items.Add(item);
        }
    }
}
