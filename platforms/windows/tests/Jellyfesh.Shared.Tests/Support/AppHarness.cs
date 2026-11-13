using Jellyfesh.Core;
using Jellyfesh.Formatting;
using Jellyfesh.Interop;
using Jellyfesh.Services;
using Jellyfesh.ViewModels;

namespace Jellyfesh.Tests.Support;

/// <summary>Answers platform questions from fields and records what was asked.</summary>
internal sealed class FakePlatform : IShellPlatform
{
    public string? Folder { get; set; } = "/music/new";

    public string? File { get; set; } = "/downloads/plugin.jfpkg";

    public string? SavePath { get; set; } = "/tmp/list.m3u8";

    public bool Confirm { get; set; } = true;

    public string? Prompt { get; set; } = "Road trip";

    public List<string> Opened { get; } = [];

    public List<string> Copied { get; } = [];

    public List<string> Confirmations { get; } = [];

    public Task<string?> PickFolderAsync() => Task.FromResult(Folder);

    public Task<string?> PickFileAsync(IReadOnlyList<string> extensions) => Task.FromResult(File);

    public Task<string?> PickSaveFileAsync(string suggestedName, string extension) => Task.FromResult(SavePath);

    public void OpenUrl(string url) => Opened.Add(url);

    public void CopyText(string text) => Copied.Add(text);

    public Task<bool> ConfirmAsync(string title, string body, string primary, string secondary)
    {
        Confirmations.Add(title);
        return Task.FromResult(Confirm);
    }

    public Task<string?> PromptTextAsync(string title, string initial, string primary) => Task.FromResult(Prompt);
}

/// <summary>An AppViewModel over a FakeCore, with a dispatcher, clock and timer the test drives.</summary>
internal sealed class AppHarness : IDisposable
{
    public AppHarness()
    {
        App = new AppViewModel(Core, Ui, KeyLocalizer.Instance, Platform, PortDrawingTime.Instance, Clock, Timer);
    }

    public FakeCore Core { get; } = new();

    public QueueDispatcher Ui { get; } = new();

    public FakePlatform Platform { get; } = new();

    public ManualClock Clock { get; } = new();

    public ManualTimer Timer { get; } = new();

    public AppViewModel App { get; }

    public ShellServices S => App.Services;

    public AppHarness Started()
    {
        App.Start();
        Ui.RunAll();
        return this;
    }

    /// <summary>Emits events and applies them as the UI thread would.</summary>
    public void Emit(params CoreEvent[] events)
    {
        foreach (var e in events)
        {
            Core.Emit(e);
        }

        Ui.RunAll();
    }

    public void Dispose()
    {
    }
}
