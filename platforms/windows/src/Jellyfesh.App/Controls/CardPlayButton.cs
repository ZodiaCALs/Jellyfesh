using Jellyfesh.App.Shell;
using Jellyfesh.App.Theming;
using Jellyfesh.Formatting;
using Jellyfesh.ViewModels;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;

namespace Jellyfesh.App.Controls;

/// <summary>
/// The play button on a card's cover (docs/ui.md, "Rows and cards"; the web's ItemCard): a glass
/// button in the cover's bottom start corner, shown while the card is pointed at or has keyboard
/// focus, as the album grid's (AlbumTile). It plays what its card plays (CardActions: a source's
/// album or playlist that plays here); a card that does not play here, such as an artist's, has
/// none. Choosing the card still opens it.
/// </summary>
public sealed partial class CardPlayButton : Button
{
    private readonly RowHover _card;
    private bool _shown;

    public CardPlayButton()
    {
        Style = (Style)Application.Current.Resources["JfGlassPlayButtonStyle"];
        HorizontalAlignment = HorizontalAlignment.Left;
        VerticalAlignment = VerticalAlignment.Bottom;
        Margin = new Thickness(8);
        Opacity = 0;
        IsTabStop = false;
        Content = new StrokeIcon { Data = Bind.Glyph("IconPlay"), Filled = true, Size = 16 };
        ToolTipService.SetToolTip(this, AppHost.L.Get("Menu_Play"));
        _card = new RowHover(this, Update);
        DataContextChanged += (_, _) => Fit();
        Click += OnClick;
    }

    /// <summary>A recycled card holds another item: shown only for one that plays, named after it ("Play {title}").</summary>
    private void Fit()
    {
        Visibility = CardActions.CollectionOf(DataContext) is not null ? Visibility.Visible : Visibility.Collapsed;
        AutomationProperties.SetName(this, DataContext is SourceEntryItem entry ? AppHost.L.Format("Card_Play", entry.Title) : AppHost.L.Get("Menu_Play"));
    }

    private void OnClick(object sender, RoutedEventArgs e)
    {
        if (CardActions.CollectionOf(DataContext) is { } collection)
        {
            CollectionActions.Play(AppHost.Services, collection);
        }
    }

    /// <summary>Fades in or out over dur-1 Fluid (at once with reduced motion) when the card's state changes.</summary>
    private void Update()
    {
        var shown = _card is not null && _card.IsActive;
        if (shown == _shown)
        {
            return;
        }

        _shown = shown;
        Motion.Fade(this, shown ? 1 : 0, AppHost.App.ReduceMotion ? TimeSpan.Zero : TokenValues.Duration(1), TokenValues.Fluid);
    }
}
