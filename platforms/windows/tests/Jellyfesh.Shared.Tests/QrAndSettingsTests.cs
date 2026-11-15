using System.Text.Json;
using Jellyfesh.Appearance;
using Jellyfesh.Core;
using Jellyfesh.Qr;
using Jellyfesh.Settings;
using Xunit;

namespace Jellyfesh.Tests;

public class QrAndSettingsTests
{
    /// <summary>
    /// Fixtures/qr-reference.json holds matrices from the python-qrcode package (byte mode, level M,
    /// each of the eight masks) for three texts at versions 3, 4 and 9 (with version information and
    /// blocks of two lengths). Every module must match.
    /// </summary>
    [Fact]
    public void Matches_an_independent_encoder_for_every_mask()
    {
        using var doc = JsonDocument.Parse(File.ReadAllText(Path.Combine(AppContext.BaseDirectory, "Fixtures", "qr-reference.json")));
        var checkedCodes = 0;
        foreach (var c in doc.RootElement.GetProperty("codes").EnumerateArray())
        {
            var text = c.GetProperty("text").GetString()!;
            var mask = c.GetProperty("mask").GetInt32();
            var expected = string.Join('\n', c.GetProperty("rows").EnumerateArray().Select(r => r.GetString())) + "\n";
            var qr = QrCode.Encode(text, QrErrorCorrection.Medium, mask);
            Assert.Equal(c.GetProperty("version").GetInt32(), qr.Version);
            Assert.Equal(expected, qr.ToString());
            checkedCodes++;
        }

        Assert.Equal(24, checkedCodes);
    }

    [Fact]
    public void Picks_the_smallest_version_and_a_mask()
    {
        var qr = QrCode.Encode("https://jellyfesh.example/login?code=ABC123");
        Assert.Equal(4, qr.Version);
        Assert.Equal(33, qr.Size);
        Assert.InRange(qr.Mask, 0, 7);
        Assert.True(qr[0, 0]); // finder corner
        Assert.False(qr[-1, 0]);
        Assert.Throws<ArgumentException>(() => QrCode.Encode(new string('x', 3_000)));
    }

    [Fact]
    public void Settings_form_follows_the_schema_and_writes_json()
    {
        const string schema = """
            {
              "type": "object",
              "required": ["server_url", "username"],
              "properties": {
                "server_url": { "type": "string", "format": "uri", "title": "Server" },
                "username": { "type": "string", "title": "User Name" },
                "api_key": { "type": "string", "format": "password" },
                "bitrate": { "type": "integer", "minimum": 64, "maximum": 320, "default": 192 },
                "transcode": { "type": "boolean", "default": true },
                "quality": { "type": "string", "enum": ["low", "high"] },
                "nested": { "type": "object" }
              }
            }
            """;
        var form = PluginSettingsForm.Create(schema, """{"server_url":"https://music.example","quality":"high"}""");
        Assert.Equal(6, form.Fields.Count); // the object is not part of the subset
        Assert.Equal("Server", form.Fields[0].Label);
        Assert.Equal(SettingFieldKind.Url, form.Fields[0].Kind);
        Assert.Equal(SettingFieldKind.Secret, form.Fields[2].Kind);
        Assert.Equal("192", form.Fields[3].Text);
        Assert.True(form.Fields[4].IsOn);
        Assert.Equal("high", form.Fields[5].Choice);

        Assert.False(form.Validate()); // username is required
        Assert.Equal("Settings_Required", form.Fields[1].Error);
        form.Fields[1].Text = "ana";
        form.Fields[3].Text = "999";
        Assert.False(form.Validate());
        Assert.Equal("Settings_OutOfRange", form.Fields[3].Error);
        form.Fields[3].Text = "256";
        Assert.True(form.Validate());

        using var written = JsonDocument.Parse(form.ToJson());
        var root = written.RootElement;
        Assert.Equal("https://music.example", root.GetProperty("server_url").GetString());
        Assert.Equal(256, root.GetProperty("bitrate").GetInt32());
        Assert.True(root.GetProperty("transcode").GetBoolean());
        Assert.False(root.TryGetProperty("api_key", out _)); // empty optional fields stay out
    }

    [Fact]
    public void Unreadable_schemas_give_an_empty_form()
    {
        Assert.True(PluginSettingsForm.Create("{not json", "{}").IsEmpty);
        Assert.True(PluginSettingsForm.Create("", "").IsEmpty);
    }

    [Fact]
    public void Automatic_icon_follows_the_appearance()
    {
        Assert.Equal("ice", AppIcons.Resolve("automatic", Core.Appearance.Light));
        Assert.Equal("steel", AppIcons.Resolve("automatic", Core.Appearance.Dark));
        Assert.Equal("lagoon", AppIcons.Resolve("lagoon", Core.Appearance.Dark));
        Assert.Equal("steel", AppIcons.Resolve("no-such-icon", Core.Appearance.Dark));
        Assert.Equal(10, AppIcons.All.Count);
        Assert.Equal(2, AppIcons.All.Count(o => o.ShipsAsIco));
        Assert.Equal(Core.Appearance.Dark, AppearanceResolver.Effective(AppearancePreference.System, systemIsDark: true));
        Assert.Equal(Core.Appearance.Light, AppearanceResolver.Effective(AppearancePreference.Light, systemIsDark: true));
    }

    [Fact]
    public void Ico_files_hold_png_images_by_size()
    {
        byte[] Png(int marker) => [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, (byte)marker];
        var ico = IcoFile.Write([(256, Png(3)), (16, Png(1)), (32, Png(2))]);
        Assert.Equal(new[] { 16, 32, 256 }, IcoFile.Sizes(ico));
        Assert.Equal(6 + (3 * 16) + (3 * 9), ico.Length);
        Assert.Equal(0, ico[6 + 32]); // 256 is written as 0
        var firstOffset = BitConverter.ToInt32(ico, 6 + 12);
        Assert.Equal(1, ico[firstOffset + 8]); // the 16 px image comes first
        Assert.Throws<ArgumentException>(() => IcoFile.Write([(16, new byte[] { 1, 2, 3 })]));
    }

    [Fact]
    public void Fallback_theme_is_misty_at_hue_250()
    {
        var light = AppearanceResolver.Palette(null, Core.Appearance.Light);
        Assert.Equal(new Rgba(0xEE, 0xF3, 0xF9, 0xFF), light["field.sky"]);
        Assert.Equal(new Rgba(0x00, 0x59, 0xA7, 0xFF), light["accent.accent"]);
        var dark = AppearanceResolver.Palette(null, Core.Appearance.Dark);
        Assert.Equal(new Rgba(0x7A, 0xBD, 0xFF, 0xFF), dark["accent.accent"]);
        Assert.Equal(42, dark.Colors.Count);
    }
}
