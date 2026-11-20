using System.Globalization;
using System.Text;
using System.Text.Json;
using CommunityToolkit.Mvvm.ComponentModel;

namespace Jellyfesh.Settings;

public enum SettingFieldKind
{
    Text,
    Secret,
    Url,
    Integer,
    Number,
    Toggle,
    Choice,

    /// <summary>A number ("type": "number") with a minimum and a maximum (profile 1.2): a slider, multipleOf its step.</summary>
    Slider,

    /// <summary>"format": "color" (profile 1.2): "#RRGGBB", a colour well.</summary>
    Color,

    /// <summary>An array of unique choices (profile 1.2): an ordered checklist; the value lists the shown items in order.</summary>
    Checklist,
}

/// <summary>A choice of a settings field: the value saved and the title shown (oneOf, or enum values as they are).</summary>
public sealed record SettingChoice(string Value, string Title)
{
    public override string ToString() => Title;
}

/// <summary>One item of an ordered checklist: shown or not, in the list's order, with Move Up and Move Down.</summary>
public sealed partial class ChecklistItem(SettingChoice choice, Action<ChecklistItem, int>? move = null) : ObservableObject
{
    [ObservableProperty]
    private bool _isChecked;

    public SettingChoice Choice { get; } = choice;

    public string Title => Choice.Title;

    [CommunityToolkit.Mvvm.Input.RelayCommand]
    private void MoveUp() => move?.Invoke(this, -1);

    [CommunityToolkit.Mvvm.Input.RelayCommand]
    private void MoveDown() => move?.Invoke(this, +1);

    public override string ToString() => Title;
}

/// <summary>One field of a source's settings form, from its JSON Schema property.</summary>
public sealed partial class SettingField : ObservableObject
{
    [ObservableProperty]
    private string _text = string.Empty;

    [ObservableProperty]
    private bool _isOn;

    [ObservableProperty]
    private string? _choice;

    [ObservableProperty]
    private string? _error;

    public required string Name { get; init; }

    public required string Label { get; init; }

    public string? Description { get; init; }

    public SettingFieldKind Kind { get; init; }

    public bool Required { get; init; }

    /// <summary>The choices' values, in order (Choice and Checklist).</summary>
    public IReadOnlyList<string> Choices => Options.Select(o => o.Value).ToArray();

    /// <summary>The choices with their titles, in order (Choice and Checklist).</summary>
    public IReadOnlyList<SettingChoice> Options { get; init; } = [];

    public double? Minimum { get; init; }

    public double? Maximum { get; init; }

    /// <summary>A slider's step (multipleOf), or null for any value.</summary>
    public double? Step { get; init; }

    /// <summary>An integer property: a slider saves whole numbers.</summary>
    public bool Integral { get; init; }

    public string? Placeholder { get; init; }

    /// <summary>The heading (x-jf-group) the field is shown under.</summary>
    public string? Group { get; init; }

    /// <summary>The first field of a run with its group: the heading shows above it.</summary>
    public bool StartsGroup { get; internal set; }

    /// <summary>A slider's value.</summary>
    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(ValueText))]
    private double _value;

    /// <summary>A slider's range and step for the view (a slider always has both bounds).</summary>
    public double SliderMinimum => Minimum ?? 0;

    public double SliderMaximum => Maximum ?? 1;

    public double SliderStep => Step ?? ((SliderMaximum - SliderMinimum) / 100);

    /// <summary>A slider's value as text, at its step's precision ("0.75").</summary>
    public string ValueText => Value.ToString(Step is { } step && step < 1 ? "0." + new string('0', Decimals(step)) : "0", CultureInfo.CurrentCulture);

    /// <summary>The chosen option (Choice), for a combo box.</summary>
    public SettingChoice? SelectedOption
    {
        get => Options.FirstOrDefault(o => o.Value == Choice);
        set
        {
            if (value is not null && value.Value != Choice)
            {
                Choice = value.Value;
                OnPropertyChanged();
            }
        }
    }

    /// <summary>An ordered checklist's items: the shown ones first in their order, then the others.</summary>
    public System.Collections.ObjectModel.ObservableCollection<ChecklistItem> Items { get; } = [];

    /// <summary>Moves a checklist item one place up.</summary>
    [CommunityToolkit.Mvvm.Input.RelayCommand]
    private void MoveUp(ChecklistItem? item) => Move(item, -1);

    /// <summary>Moves a checklist item one place down.</summary>
    [CommunityToolkit.Mvvm.Input.RelayCommand]
    private void MoveDown(ChecklistItem? item) => Move(item, +1);

    internal void Move(ChecklistItem? item, int by)
    {
        var from = item is null ? -1 : Items.IndexOf(item);
        var to = from + by;
        if (from >= 0 && to >= 0 && to < Items.Count)
        {
            Items.Move(from, to);
        }
    }

    private static int Decimals(double step)
    {
        var digits = 0;
        while (digits < 6 && Math.Abs((step * Math.Pow(10, digits)) - Math.Round(step * Math.Pow(10, digits))) > 1e-9)
        {
            digits++;
        }

        return Math.Max(1, digits);
    }
}

/// <summary>
/// A settings form built from a plugin's settings schema (ADR-0004: a JSON Schema subset) and
/// filled from its current settings JSON. It only validates types and bounds; what the values
/// mean is the plugin's business.
/// </summary>
public sealed class PluginSettingsForm
{
    private PluginSettingsForm(IReadOnlyList<SettingField> fields) => Fields = fields;

    public IReadOnlyList<SettingField> Fields { get; }

    public bool IsEmpty => Fields.Count == 0;

    /// <summary>Builds the form; an unreadable schema gives an empty form.</summary>
    public static PluginSettingsForm Create(string schemaJson, string settingsJson)
    {
        var fields = new List<SettingField>();
        try
        {
            using var schema = JsonDocument.Parse(string.IsNullOrWhiteSpace(schemaJson) ? "{}" : schemaJson);
            using var values = JsonDocument.Parse(string.IsNullOrWhiteSpace(settingsJson) ? "{}" : settingsJson);
            var root = schema.RootElement;
            if (root.ValueKind != JsonValueKind.Object || !root.TryGetProperty("properties", out var props)
                || props.ValueKind != JsonValueKind.Object)
            {
                return new PluginSettingsForm(fields);
            }

            var required = root.TryGetProperty("required", out var req) && req.ValueKind == JsonValueKind.Array
                ? req.EnumerateArray().Where(e => e.ValueKind == JsonValueKind.String).Select(e => e.GetString()!).ToHashSet()
                : [];
            foreach (var prop in props.EnumerateObject())
            {
                if (prop.Value.ValueKind != JsonValueKind.Object)
                {
                    continue;
                }

                var field = Field(prop.Name, prop.Value, required.Contains(prop.Name));
                if (field is null)
                {
                    continue;
                }

                var current = values.RootElement.ValueKind == JsonValueKind.Object && values.RootElement.TryGetProperty(prop.Name, out var v)
                    ? v
                    : prop.Value.TryGetProperty("default", out var d) ? d : default;
                Fill(field, current);
                field.StartsGroup = field.Group is not null && (fields.Count == 0 || fields[^1].Group != field.Group);
                fields.Add(field);
            }
        }
        catch (JsonException)
        {
            fields.Clear();
        }

        return new PluginSettingsForm(fields);
    }

    /// <summary>Checks every field; true when the form can be saved.</summary>
    public bool Validate()
    {
        var ok = true;
        foreach (var f in Fields)
        {
            f.Error = Check(f);
            ok &= f.Error is null;
        }

        return ok;
    }

    /// <summary>The settings as JSON for SetPluginSettings (empty optional fields are left out).</summary>
    public string ToJson()
    {
        using var stream = new MemoryStream();
        using (var w = new Utf8JsonWriter(stream))
        {
            w.WriteStartObject();
            foreach (var f in Fields)
            {
                switch (f.Kind)
                {
                    case SettingFieldKind.Toggle:
                        w.WriteBoolean(f.Name, f.IsOn);
                        break;
                    case SettingFieldKind.Integer when long.TryParse(f.Text, NumberStyles.Integer, CultureInfo.InvariantCulture, out var i):
                        w.WriteNumber(f.Name, i);
                        break;
                    case SettingFieldKind.Number when double.TryParse(f.Text, NumberStyles.Float, CultureInfo.InvariantCulture, out var n):
                        w.WriteNumber(f.Name, n);
                        break;
                    case SettingFieldKind.Choice when f.Choice is not null:
                        w.WriteString(f.Name, f.Choice);
                        break;
                    case SettingFieldKind.Slider when f.Integral:
                        w.WriteNumber(f.Name, (long)Math.Round(SnapToStep(f)));
                        break;
                    case SettingFieldKind.Slider:
                        w.WriteNumber(f.Name, Math.Round(SnapToStep(f), 6));
                        break;
                    case SettingFieldKind.Color when f.Text.Length > 0 || f.Required:
                        w.WriteString(f.Name, f.Text.ToUpperInvariant());
                        break;
                    case SettingFieldKind.Checklist:
                        w.WriteStartArray(f.Name);
                        foreach (var item in f.Items.Where(i => i.IsChecked))
                        {
                            w.WriteStringValue(item.Choice.Value);
                        }

                        w.WriteEndArray();
                        break;
                    case SettingFieldKind.Text or SettingFieldKind.Secret or SettingFieldKind.Url when f.Text.Length > 0 || f.Required:
                        w.WriteString(f.Name, f.Text);
                        break;
                }
            }

            w.WriteEndObject();
        }

        return Encoding.UTF8.GetString(stream.ToArray());
    }

    private static SettingField? Field(string name, JsonElement p, bool required)
    {
        var type = p.TryGetProperty("type", out var t) && t.ValueKind == JsonValueKind.String ? t.GetString() : "string";
        var format = p.TryGetProperty("format", out var f) && f.ValueKind == JsonValueKind.String ? f.GetString() : null;
        var choices = Options(p);
        var minimum = Num(p, "minimum");
        var maximum = Num(p, "maximum");
        var ranged = minimum is { } lo && maximum is { } hi && hi > lo;
        var items = type == "array" && p.TryGetProperty("items", out var it) && it.ValueKind == JsonValueKind.Object ? Options(it) : [];
        var kind = (type, format) switch
        {
            ("boolean", _) => SettingFieldKind.Toggle,
            ("number", _) when ranged => SettingFieldKind.Slider, // integers stay fields: ports, counts
            ("integer", _) => SettingFieldKind.Integer,
            ("number", _) => SettingFieldKind.Number,
            ("string", _) when choices.Count > 0 => SettingFieldKind.Choice,
            ("string", "color") => SettingFieldKind.Color,
            ("string", "password") => SettingFieldKind.Secret,
            ("string", "uri" or "url") => SettingFieldKind.Url,
            ("string", _) => SettingFieldKind.Text,
            ("array", _) when items.Count > 0 => SettingFieldKind.Checklist,
            _ => (SettingFieldKind?)null,
        };
        if (kind is null)
        {
            return null; // objects, and arrays other than checklists, are not part of the subset
        }

        // A secret flag (x-secret) marks tokens and keys too.
        if (kind == SettingFieldKind.Text && p.TryGetProperty("x-secret", out var secret) && secret.ValueKind == JsonValueKind.True)
        {
            kind = SettingFieldKind.Secret;
        }

        return new SettingField
        {
            Name = name,
            Label = Str(p, "title") ?? name,
            Description = Str(p, "description"),
            Kind = kind.Value,
            Required = required,
            Options = kind == SettingFieldKind.Checklist ? items : choices,
            Minimum = minimum,
            Maximum = maximum,
            Step = Num(p, "multipleOf") is { } step && step > 0 ? step : type == "integer" ? 1 : null,
            Integral = type == "integer",
            Group = Str(p, "x-jf-group"),
            Placeholder = p.TryGetProperty("examples", out var ex) && ex.ValueKind == JsonValueKind.Array && ex.GetArrayLength() > 0
                ? ex[0].ToString()
                : null,
        };
    }

    /// <summary>A property's choices: <c>oneOf</c> with <c>const</c> and <c>title</c>, else <c>enum</c> values as they are.</summary>
    private static IReadOnlyList<SettingChoice> Options(JsonElement p)
    {
        if (p.TryGetProperty("oneOf", out var one) && one.ValueKind == JsonValueKind.Array)
        {
            return one.EnumerateArray()
                .Where(o => o.ValueKind == JsonValueKind.Object && o.TryGetProperty("const", out var c) && c.ValueKind == JsonValueKind.String)
                .Select(o =>
                {
                    var value = o.GetProperty("const").GetString()!;
                    return new SettingChoice(value, Str(o, "title") ?? value);
                })
                .ToArray();
        }

        return p.TryGetProperty("enum", out var e) && e.ValueKind == JsonValueKind.Array
            ? e.EnumerateArray().Where(x => x.ValueKind == JsonValueKind.String).Select(x => new SettingChoice(x.GetString()!, x.GetString()!)).ToArray()
            : [];
    }

    /// <summary>A slider's value at its step, within its range.</summary>
    private static double SnapToStep(SettingField f)
    {
        var min = f.Minimum ?? 0;
        var value = f.Value;
        if (f.Step is { } step && step > 0)
        {
            value = min + (Math.Round((value - min) / step) * step);
        }

        return Math.Clamp(value, min, f.Maximum ?? value);
    }

    private static void Fill(SettingField field, JsonElement value)
    {
        switch (field.Kind)
        {
            case SettingFieldKind.Toggle:
                field.IsOn = value.ValueKind == JsonValueKind.True;
                break;
            case SettingFieldKind.Choice:
                field.Choice = value.ValueKind == JsonValueKind.String && field.Options.Any(o => o.Value == value.GetString())
                    ? value.GetString()
                    : field.Options.FirstOrDefault()?.Value;
                break;
            case SettingFieldKind.Integer or SettingFieldKind.Number:
                field.Text = value.ValueKind == JsonValueKind.Number ? value.GetRawText() : string.Empty;
                break;
            case SettingFieldKind.Slider:
                field.Value = Math.Clamp(
                    value.ValueKind == JsonValueKind.Number ? value.GetDouble() : field.Minimum ?? 0,
                    field.Minimum ?? double.MinValue,
                    field.Maximum ?? double.MaxValue);
                break;
            case SettingFieldKind.Checklist:
                var shown = value.ValueKind == JsonValueKind.Array
                    ? value.EnumerateArray().Where(v => v.ValueKind == JsonValueKind.String).Select(v => v.GetString()!).Distinct().ToList()
                    : [];
                foreach (var option in shown.Select(v => field.Options.FirstOrDefault(o => o.Value == v)).OfType<SettingChoice>())
                {
                    field.Items.Add(new ChecklistItem(option, field.Move) { IsChecked = true });
                }

                foreach (var option in field.Options.Where(o => !shown.Contains(o.Value)))
                {
                    field.Items.Add(new ChecklistItem(option, field.Move));
                }

                break;
            default:
                field.Text = value.ValueKind == JsonValueKind.String ? value.GetString()! : string.Empty;
                break;
        }
    }

    private static string? Check(SettingField f)
    {
        switch (f.Kind)
        {
            case SettingFieldKind.Integer or SettingFieldKind.Number:
                if (f.Text.Length == 0)
                {
                    return f.Required ? "Settings_Required" : null;
                }

                if (!double.TryParse(f.Text, NumberStyles.Float, CultureInfo.InvariantCulture, out var n)
                    || (f.Kind == SettingFieldKind.Integer && !long.TryParse(f.Text, NumberStyles.Integer, CultureInfo.InvariantCulture, out _)))
                {
                    return "Settings_NotANumber";
                }

                return (f.Minimum is { } min && n < min) || (f.Maximum is { } max && n > max) ? "Settings_OutOfRange" : null;
            case SettingFieldKind.Url:
                if (f.Text.Length == 0)
                {
                    return f.Required ? "Settings_Required" : null;
                }

                return Uri.TryCreate(f.Text, UriKind.Absolute, out var uri) && (uri.Scheme == Uri.UriSchemeHttps || uri.Scheme == Uri.UriSchemeHttp)
                    ? null
                    : "Settings_NotAUrl";
            case SettingFieldKind.Text or SettingFieldKind.Secret:
                return f.Required && f.Text.Length == 0 ? "Settings_Required" : null;
            case SettingFieldKind.Color:
                if (f.Text.Length == 0)
                {
                    return f.Required ? "Settings_Required" : null;
                }

                return IsColor(f.Text) ? null : "Settings_NotAColour";
            default:
                return null;
        }
    }

    /// <summary>"#RRGGBB".</summary>
    public static bool IsColor(string text) =>
        text.Length == 7 && text[0] == '#' && text.Skip(1).All(Uri.IsHexDigit);

    private static string? Str(JsonElement e, string name) =>
        e.TryGetProperty(name, out var v) && v.ValueKind == JsonValueKind.String ? v.GetString() : null;

    private static double? Num(JsonElement e, string name) =>
        e.TryGetProperty(name, out var v) && v.ValueKind == JsonValueKind.Number ? v.GetDouble() : null;
}
