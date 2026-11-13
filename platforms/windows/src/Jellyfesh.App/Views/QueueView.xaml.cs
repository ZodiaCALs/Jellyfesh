using Jellyfesh.App.Shell;
using Jellyfesh.ViewModels;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Windows.ApplicationModel.DataTransfer;

namespace Jellyfesh.App.Views;

public sealed partial class QueueView : UserControl
{
    private readonly MenuFlyout _menu = new();
    private QueueEntryItem? _menuItem;

    public QueueView()
    {
        Queue = AppHost.App.Queue;
        InitializeComponent();
        var l = AppHost.L;
        var playNext = new MenuFlyoutItem { Text = l.Get("Menu_PlayNext") };
        playNext.Click += (_, _) => Queue.PlayNextCommand.Execute(_menuItem);
        var remove = new MenuFlyoutItem { Text = l.Get("Menu_RemoveFromQueue") };
        remove.Click += (_, _) => Queue.RemoveCommand.Execute(_menuItem);
        _menu.Items.Add(playNext);
        _menu.Items.Add(remove);
        _menu.Opening += (_, _) => _menuItem = Lists.ItemOf<QueueEntryItem>(_menu);
        Lists.Attach(List, 40, realize: Queue.LoadMoreIfNeeded, menu: _menu);
    }

    public QueueViewModel Queue { get; }

    private void OnItemClick(object sender, ItemClickEventArgs e) => Queue.JumpToCommand.Execute(e.ClickedItem as QueueEntryItem);

    private void OnDragItemsCompleted(ListViewBase sender, DragItemsCompletedEventArgs args)
    {
        if (args.DropResult == DataPackageOperation.Move && args.Items.FirstOrDefault() is QueueEntryItem item)
        {
            Queue.CommitDrag(item);
        }
    }

    private void OnMoveUp(KeyboardAccelerator sender, KeyboardAcceleratorInvokedEventArgs args) => Move(args, -1);

    private void OnMoveDown(KeyboardAccelerator sender, KeyboardAcceleratorInvokedEventArgs args) => Move(args, 1);

    private void Move(KeyboardAcceleratorInvokedEventArgs args, int by)
    {
        if (List.SelectedItem is QueueEntryItem item)
        {
            var to = Queue.UpNext.IndexOf(item) + by;
            if (to >= 0 && to < Queue.UpNext.Count)
            {
                Queue.MoveTo(item, to);
            }

            args.Handled = true;
        }
    }

    private void OnRemoveKey(KeyboardAccelerator sender, KeyboardAcceleratorInvokedEventArgs args)
    {
        if (List.SelectedItem is QueueEntryItem item)
        {
            Queue.RemoveCommand.Execute(item);
            args.Handled = true;
        }
    }
}
