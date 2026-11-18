using System.Globalization;
using System.Text.Json;
using Jellyfesh.Core;
using Jellyfesh.Formatting;
using Jellyfesh.Interop;
using Jellyfesh.Tests.Support;
using Jellyfesh.ViewModels;
using Xunit;
using F = Jellyfesh.Ffi;

namespace Jellyfesh.Tests;

/// <summary>
/// Sign-in steps and fields as the plugin describes them (docs/parity.md N10, N11): Jellyfin's
/// Quick Connect code, the plugin's sentence under a QR code, optional fields, help, kinds, and a
/// failure that says the plugin's message alone.
/// </summary>
public class SignInTests
{
    private static readonly Dictionary<string, string> English = new(StringComparer.Ordinal)
    {
        ["SignIn_ScanQr"] = "Scan the code with the {0} app on your phone.",
        ["SignIn_CodeSpoken"] = "Code {0}",
        ["SignIn_Optional"] = "Optional",
        ["SignIn_FailedNoReason"] = "Signing in did not work.",
    };

    [Fact]
    public void Quick_connect_shows_the_code_with_the_plugins_words()
    {
        var (h, flow) = Begin();

        h.Emit(Step(new AuthStep.ShowCode("482 913", "In Jellyfin, open Quick Connect and enter this code.", null)));

        Assert.Equal(SignInStage.ShowCode, flow.Stage);
        Assert.Equal("482 913", flow.Code);
        Assert.Equal("In Jellyfin, open Quick Connect and enter this code.", flow.CodeHint);
        Assert.Equal("Code 4 8 2 9 1 3", flow.CodeSpoken);
        Assert.False(flow.IsFinished);
        Assert.False(flow.CanSubmit);

        // The core polls the plugin; approval on the server ends the flow.
        h.Emit(Step(new AuthStep.Done("ana")));
        Assert.Equal(SignInStage.Done, flow.Stage);
    }

    [Fact]
    public void A_qr_code_carries_the_plugins_sentence_or_the_shared_one()
    {
        var (h, flow) = Begin();

        h.Emit(Step(new AuthStep.ShowQr("https://login.example/q", null, "Open the Example app and scan this.")));
        Assert.Equal("Open the Example app and scan this.", flow.QrHint);

        h.Emit(Step(new AuthStep.ShowQr("https://login.example/q2", null)));
        Assert.Equal("Scan the code with the Sub app on your phone.", flow.QrHint);
        Assert.NotNull(flow.Qr);
    }

    [Fact]
    public void Sign_in_waits_for_the_required_fields_only()
    {
        var (h, flow) = Begin();
        h.Emit(Step(new AuthStep.AskFields(
        [
            new AuthField("user", "User Name", false, null),
            new AuthField("password", "Password", true, null),
            new AuthField("otp", "One-Time Code", false, null, AuthFieldKind.Number, Required: false),
        ])));

        Assert.False(flow.CanSubmit);
        Assert.False(flow.SubmitCommand.CanExecute(null));
        flow.Fields[0].Value = "ana";
        flow.Fields[1].Value = "   ";
        Assert.False(flow.CanSubmit); // spaces are not a password
        flow.Fields[1].Value = "secret";
        Assert.True(flow.CanSubmit); // the optional code may stay empty
        Assert.True(flow.SubmitCommand.CanExecute(null));

        flow.SubmitCommand.Execute(null);
        var submit = h.Core.LastSent<CoreCommand.SubmitLogin>();
        Assert.Contains(new KeyValuePair<string, string>("otp", string.Empty), submit.Fields);
        Assert.Equal(SignInStage.Waiting, flow.Stage);
        Assert.False(flow.CanSubmit);
    }

    [Theory]
    [InlineData("123456", "123456")]
    [InlineData("12 34-56", "123456")]
    [InlineData("１２３", "123")]
    [InlineData("abc", "")]
    public void A_number_field_keeps_its_digits(string typed, string kept)
    {
        var field = new SignInField(new AuthField("otp", "Code", false, null, AuthFieldKind.Number), Localizer());
        field.Value = typed;
        Assert.Equal(kept, field.Value);
        Assert.Equal("a b", SignInField.Accepted("a b", AuthFieldKind.Text));
    }

    [Fact]
    public void Fields_show_optional_and_the_plugins_help_under_their_label()
    {
        var l = Localizer();
        var email = new SignInField(new AuthField("user", "Email", false, "you@example.com", AuthFieldKind.Email, true, " The address you signed up with "), l);
        var code = new SignInField(new AuthField("otp", "Code", false, null, AuthFieldKind.Number, false, "From your authenticator app"), l);
        var server = new SignInField(new AuthField("server", "Server", false, null, AuthFieldKind.Url, false), l);
        var plain = new SignInField(new AuthField("user", "User Name", false, null), l);
        var cookies = new SignInField(new AuthField("cookies", "Cookies", false, null, AuthFieldKind.CookieHeader), l);

        Assert.Equal("The address you signed up with", email.Note);
        Assert.Equal("you@example.com", email.Placeholder);
        Assert.Equal("Optional · From your authenticator app", code.Note);
        Assert.Equal("Optional", server.Note);
        Assert.Null(plain.Note);
        Assert.True(cookies.IsMultiline);
        Assert.False(plain.IsMultiline);
    }

    [Fact]
    public void A_failed_sign_in_shows_the_message_alone()
    {
        var (h, flow) = Begin();

        h.Emit(Step(new AuthStep.Failed("The code has expired. Try again.")));
        Assert.Equal(SignInStage.Failed, flow.Stage);
        Assert.Equal("The code has expired. Try again.", flow.FailureText);

        var (h2, other) = Begin();
        h2.Emit(Step(new AuthStep.Failed(" ")));
        Assert.Equal("Signing in did not work.", other.FailureText);
    }

    [Fact]
    public void Sign_in_steps_map_with_the_code_the_hints_and_the_fields_details()
    {
        using var contract = Contract.Read("ffi-types.json");
        var steps = contract.RootElement.GetProperty("samples").EnumerateArray()
            .Where(s => s.GetProperty("name").GetString() == "Event")
            .Select(s => s.GetProperty("json"))
            .Where(j => j.GetProperty("type").GetString() == "Auth")
            .Select(j => Assert.IsType<CoreEvent.Auth>(FfiMapping.ToModel(j.Deserialize(F.JellyfeshJsonContext.Default.Event)!)).Update.Step)
            .ToList();

        var qr = Assert.Single(steps.OfType<AuthStep.ShowQr>());
        Assert.Equal(("otpauth://x", "Scan with the Example app", 1_700_000_000_000L), (qr.Data, qr.Hint, qr.ExpiresAtMs!.Value));
        var code = Assert.Single(steps.OfType<AuthStep.ShowCode>());
        Assert.Equal(("482 913", "In Example, open Quick Connect"), (code.Code, code.Hint));
        Assert.Null(code.ExpiresAtMs);
        var fields = Assert.Single(steps.OfType<AuthStep.AskFields>()).Fields;
        Assert.Equal(
            new AuthField("username", "User name", false, null, AuthFieldKind.Email, true, "The e-mail address you signed up with"),
            fields[0]);
        Assert.Equal(new AuthField("password", "Password", true, "••••", AuthFieldKind.Text, true, null), fields[1]);
        Assert.DoesNotContain(steps, s => s is AuthStep.Failed f && f.Message.Contains("unknown", StringComparison.Ordinal));
    }

    private static (AppHarness Harness, SignInViewModel Flow) Begin()
    {
        var h = new AppHarness(Localizer()).Started();
        var plugin = new PluginInfo(
            "org.sub", "Sub", "1.0.0", "Someone", PluginBadge.Official, false, ["search"],
            [AuthMethod.Qr, AuthMethod.PasswordExchange], true, null, false, "{}", "{}", null);
        var flow = new SignInViewModel(h.S, plugin, AuthMethod.Qr);
        flow.Begin();
        return (h, flow);
    }

    private static CoreEvent Step(AuthStep step) => new CoreEvent.Auth(new AuthUpdate("org.sub", 7, step));

    private static TableLocalizer Localizer() => new(English);

    private sealed class TableLocalizer(Dictionary<string, string> table) : ILocalizer
    {
        public CultureInfo Culture => CultureInfo.InvariantCulture;

        public string Get(string key) => table.TryGetValue(key, out var value) ? value : key;
    }
}
