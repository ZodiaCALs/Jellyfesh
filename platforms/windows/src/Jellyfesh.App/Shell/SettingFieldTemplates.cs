using Jellyfesh.Settings;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace Jellyfesh.App.Shell;

/// <summary>Picks the input for a source's settings field (from its JSON Schema) by kind.</summary>
public sealed partial class SettingFieldTemplates : DataTemplateSelector
{
    public DataTemplate? Text { get; set; }

    public DataTemplate? Secret { get; set; }

    public DataTemplate? Toggle { get; set; }

    public DataTemplate? Choice { get; set; }

    /// <summary>Profile 1.2: a number between two bounds.</summary>
    public DataTemplate? Slider { get; set; }

    /// <summary>Profile 1.2: "#RRGGBB".</summary>
    public DataTemplate? Color { get; set; }

    /// <summary>Profile 1.2: an ordered checklist.</summary>
    public DataTemplate? Checklist { get; set; }

    protected override DataTemplate? SelectTemplateCore(object item) => Pick(item);

    protected override DataTemplate? SelectTemplateCore(object item, DependencyObject container) => Pick(item);

    private DataTemplate? Pick(object item) => item is SettingField field
        ? field.Kind switch
        {
            SettingFieldKind.Secret => Secret,
            SettingFieldKind.Toggle => Toggle,
            SettingFieldKind.Choice => Choice,
            SettingFieldKind.Slider => Slider,
            SettingFieldKind.Color => Color,
            SettingFieldKind.Checklist => Checklist,
            _ => Text, // text, URL and numbers: a text box the form validates
        }
        : Text;
}
