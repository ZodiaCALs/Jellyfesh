using Jellyfesh.Services;
using Xunit;

namespace Jellyfesh.Tests;

/// <summary>The start-up safety net's logic: the smoke-test switch, exit codes, where the log goes and what a failure says.</summary>
public class StartupTests
{
    [Theory]
    [InlineData(new string[0], false)]
    [InlineData(new[] { "--smoke-test" }, true)]
    [InlineData(new[] { "--SMOKE-TEST" }, true)]
    [InlineData(new[] { "  --smoke-test " }, true)]
    [InlineData(new[] { "C:\\Music\\song.flac", "--smoke-test" }, true)]
    [InlineData(new[] { "--smoke" }, false)]
    [InlineData(new[] { "smoke-test" }, false)]
    public void The_smoke_test_starts_only_with_its_switch(string[] args, bool smoke)
    {
        Assert.Equal(smoke, StartupOptions.Parse(args).SmokeTest);
    }

    [Fact]
    public void Exit_codes_are_distinct_and_only_success_is_zero()
    {
        int[] codes = [ExitCodes.Ok, ExitCodes.StartupFailed, ExitCodes.TimedOut, ExitCodes.AlreadyRunning, ExitCodes.ShutdownHung];
        Assert.Equal(codes.Length, codes.Distinct().Count());
        Assert.Equal(0, ExitCodes.Ok);
        Assert.All(codes.Skip(1), c => Assert.True(c > 0));
    }

    [Fact]
    public void The_log_prefers_the_configured_folder_then_local_app_data_then_temp()
    {
        var folders = LogLocation.Candidates("D:\\Logs", "C:\\Users\\张三\\AppData\\Local", "C:\\Temp");
        Assert.Equal(
            ["D:\\Logs", Path.Combine("C:\\Users\\张三\\AppData\\Local", "Jellyfesh", "logs"), Path.Combine("C:\\Temp", "Jellyfesh", "logs")],
            folders);
    }

    [Theory]
    [InlineData(null)]
    [InlineData("")]
    [InlineData("   ")]
    public void Without_a_configured_folder_the_log_goes_to_local_app_data(string? configured)
    {
        var folders = LogLocation.Candidates(configured, "C:\\Users\\Ann\\AppData\\Local", "C:\\Temp");
        Assert.Equal(Path.Combine("C:\\Users\\Ann\\AppData\\Local", "Jellyfesh", "logs"), folders[0]);
        Assert.Equal(2, folders.Count);
    }

    [Fact]
    public void Missing_folders_are_left_out_and_the_temporary_folder_remains()
    {
        Assert.Equal([Path.Combine("/tmp", "Jellyfesh", "logs")], LogLocation.Candidates(null, "", "/tmp"));
        Assert.Empty(LogLocation.Candidates(null, null, null));
    }

    [Fact]
    public void A_failure_names_the_stage_and_every_cause_with_its_hresult()
    {
        var error = new InvalidOperationException(
            "the window could not be created",
            new System.Runtime.InteropServices.COMException("Cannot find a resource with the name JfInk90.", unchecked((int)0x802B000A)));

        var text = FailureText.Describe("opening the main window", error);

        var lines = text.Split(Environment.NewLine);
        Assert.Equal("While opening the main window:", lines[0]);
        Assert.StartsWith("InvalidOperationException: the window could not be created (0x", lines[1], StringComparison.Ordinal);
        Assert.Equal("Caused by COMException: Cannot find a resource with the name JfInk90. (0x802B000A)", lines[2]);
    }

    [Fact]
    public void A_long_chain_of_causes_is_cut_short()
    {
        Exception error = new InvalidOperationException("innermost");
        for (var i = 0; i < 10; i++)
        {
            error = new InvalidOperationException($"level {i}", error);
        }

        var lines = FailureText.Describe("starting the core", error).Split(Environment.NewLine);

        Assert.Equal(1 + FailureText.MaxDepth, lines.Length);
    }
}
