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

    public IReadOnlyList<string> Choices { get; init; } = [];

    public double? Minimum { get; init; }

    public double? Maximum { get; init; }

    public string? Placeholder { get; init; }
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
        var choices = p.TryGetProperty("enum", out var e) && e.ValueKind == JsonValueKind.Array
            ? e.EnumerateArray().Where(x => x.ValueKind == JsonValueKind.String).Select(x => x.GetString()!).ToArray()
            : [];
        var kind = (type, format) switch
        {
            ("boolean", _) => SettingFieldKind.Toggle,
            ("integer", _) => SettingFieldKind.Integer,
            ("number", _) => SettingFieldKind.Number,
            ("string", _) when choices.Length > 0 => SettingFieldKind.Choice,
            ("string", "password") => SettingFieldKind.Secret,
            ("string", "uri" or "url") => SettingFieldKind.Url,
            ("string", _) => SettingFieldKind.Text,
            _ => (SettingFieldKind?)null,
        };
        if (kind is null)
        {
            return null; // objects and arrays are not part of the subset
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
            Choices = choices,
            Minimum = Num(p, "minimum"),
            Maximum = Num(p, "maximum"),
            Placeholder = p.TryGetProperty("examples", out var ex) && ex.ValueKind == JsonValueKind.Array && ex.GetArrayLength() > 0
                ? ex[0].ToString()
                : null,
        };
    }

    private static void Fill(SettingField field, JsonElement value)
    {
        switch (field.Kind)
        {
            case SettingFieldKind.Toggle:
                field.IsOn = value.ValueKind == JsonValueKind.True;
                break;
            case SettingFieldKind.Choice:
                field.Choice = value.ValueKind == JsonValueKind.String && field.Choices.Contains(value.GetString())
                    ? value.GetString()
                    : field.Choices.FirstOrDefault();
                break;
            case SettingFieldKind.Integer or SettingFieldKind.Number:
                field.Text = value.ValueKind == JsonValueKind.Number ? value.GetRawText() : string.Empty;
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
            default:
                return null;
        }
    }

    private static string? Str(JsonElement e, string name) =>
        e.TryGetProperty(name, out var v) && v.ValueKind == JsonValueKind.String ? v.GetString() : null;

    private static double? Num(JsonElement e, string name) =>
        e.TryGetProperty(name, out var v) && v.ValueKind == JsonValueKind.Number ? v.GetDouble() : null;
}
