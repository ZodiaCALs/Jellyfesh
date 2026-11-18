using System.Globalization;
using System.Runtime.InteropServices;
using Jellyfesh.App.Platform;
using Jellyfesh.App.Shell;
using Jellyfesh.Services;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml;
using Microsoft.Windows.AppLifecycle;

namespace Jellyfesh.App;

/// <summary>
/// The entry point (the XAML compiler's Main is turned off with DISABLE_XAML_GENERATED_MAIN). It
/// starts the log and the safety net first (Startup), loads the Windows App SDK runtime, keeps one
/// Jellyfesh per user (a second start hands its activation to the running one, which comes to the
/// front, and exits: the Windows App SDK's single-instancing pattern) and starts XAML. Anything that
/// fails on the way is logged and shown (docs/windows.md, "When Jellyfesh does not start").
/// </summary>
public static class Program
{
    private const string InstanceKey = "Jellyfesh.Main";

    /// <summary>A smoke test registers under its own key, so a running Jellyfesh never takes its start.</summary>
    private const string SmokeTestInstanceKey = "Jellyfesh.SmokeTest";

    /// <summary>How long a second start waits for the running Jellyfesh to take its activation.</summary>
    private static readonly TimeSpan RedirectTimeout = TimeSpan.FromSeconds(20);

    private static AppInstance? _instance;

    private enum Redirect
    {
        /// <summary>This is the only Jellyfesh: start.</summary>
        None,

        /// <summary>The running Jellyfesh took the start.</summary>
        Done,

        /// <summary>A Jellyfesh is running but did not take the start in time.</summary>
        NoAnswer,
    }

    [STAThread]
    private static int Main(string[] args)
    {
        Startup.Begin(args);
        try
        {
            Startup.EnterStage("loading the Windows App SDK runtime");
            LoadWindowsAppRuntime();
            WinRT.ComWrappersSupport.InitializeComWrappers();

            Startup.EnterStage("looking for a Jellyfesh that is already running");
            switch (RedirectToRunningInstance(Startup.SmokeTest ? SmokeTestInstanceKey : InstanceKey))
            {
                case Redirect.Done:
                    AppLog.Write("the running Jellyfesh took this start");
                    return Startup.SmokeTest ? ExitCodes.AlreadyRunning : ExitCodes.Ok;
                case Redirect.NoAnswer:
                    AppLog.Write("the running Jellyfesh did not take this start in time");
                    if (!Startup.SmokeTest)
                    {
                        StartupErrorWindow.ShowNotAnswering();
                    }

                    return ExitCodes.AlreadyRunning;
            }

            Startup.EnterStage("starting XAML");
            Application.Start(start =>
            {
                _ = start;
                try
                {
                    var context = new DispatcherQueueSynchronizationContext(DispatcherQueue.GetForCurrentThread());
                    SynchronizationContext.SetSynchronizationContext(context);
                    _ = new App();
                }
                catch (Exception e)
                {
                    // The App could not even be made: a message box (no XAML), then the end.
                    Startup.Fail(e, xamlWorks: false);
                }
            });
            AppLog.Write($"ended (code {Startup.ExitCode})");
        }
        catch (Exception e)
        {
            Startup.Fail(e, xamlWorks: false);
        }

        return Startup.ExitCode;
    }

    /// <summary>
    /// The window is closing: a new start now opens a new window instead of handing itself to this
    /// process, which is shutting down (the Windows App SDK asks apps to unregister before they exit).
    /// </summary>
    internal static void ReleaseInstanceKey()
    {
        try
        {
            _instance?.UnregisterKey();
            _instance = null;
        }
        catch (Exception e)
        {
            AppLog.Write("the single-instance key could not be released", e);
        }
    }

    /// <summary>
    /// What the Windows App SDK's UndockedRegFreeWinRT auto-initializer does for a self-contained app
    /// in a module initializer, before Main. The project turns that initializer off
    /// (WindowsAppSdkUndockedRegFreeWinRTInitialize), so that a runtime that cannot load, such as a
    /// missing Microsoft.WindowsAppRuntime.dll, is logged and shown here instead of ending the
    /// process before any of our code runs. Loading the DLL sets up registration-free WinRT for the
    /// Windows App SDK's classes; the two variables tell it, and MRT Core, where the app is.
    /// </summary>
    private static void LoadWindowsAppRuntime()
    {
        Environment.SetEnvironmentVariable("MICROSOFT_WINDOWSAPPRUNTIME_BASE_DIRECTORY", AppContext.BaseDirectory);
        Environment.SetEnvironmentVariable(
            "MICROSOFT_WINDOWSAPPRUNTIME_BASE_DIRECTORY_PID",
            Environment.ProcessId.ToString(CultureInfo.InvariantCulture));
        var result = Win32.WindowsAppRuntimeEnsureIsLoaded();
        if (result < 0)
        {
            Marshal.ThrowExceptionForHR(result);
        }
    }

    private static Redirect RedirectToRunningInstance(string key)
    {
        // A second look when handing over failed: the other Jellyfesh may just have ended.
        for (var attempt = 0; ; attempt++)
        {
            AppInstance main;
            try
            {
                main = AppInstance.FindOrRegisterForKey(key);
            }
            catch (Exception e)
            {
                AppLog.Write("the single-instance check failed; starting anyway", e);
                return Redirect.None; // better two instances than none
            }

            if (main.IsCurrent)
            {
                _instance = main;

                // Raised on a background thread when another start redirects here.
                main.Activated += (_, _) =>
                {
                    AppLog.Write("another start was handed to this Jellyfesh");
                    App.BringToFront();
                };
                return Redirect.None;
            }

            AppLog.Write($"Jellyfesh is already running (process {main.ProcessId}); handing this start to it");
            switch (HandOver(main))
            {
                case true:
                    return Redirect.Done;
                case null:
                    return Redirect.NoAnswer;
                case false when attempt > 0:
                    return Redirect.NoAnswer;
            }
        }
    }

    /// <summary>True when the running Jellyfesh took the start, false when handing over failed, null when it did not answer in time.</summary>
    private static bool? HandOver(AppInstance main)
    {
        var activation = AppInstance.GetCurrent().GetActivatedEventArgs();
        Win32.AllowSetForegroundWindow(main.ProcessId);

        // Redirect on another thread and wait without blocking COM on this STA thread.
        var done = Win32.CreateEvent(0, true, false, null);
        var redirected = 0;
        _ = Task.Run(() =>
        {
            try
            {
                main.RedirectActivationToAsync(activation).AsTask().Wait();
                Volatile.Write(ref redirected, 1);
            }
            catch (Exception e)
            {
                AppLog.Write("handing this start to the running Jellyfesh failed", e);
            }
            finally
            {
                Win32.SetEvent(done);
            }
        });
        if (!Win32.WaitPumpingCom(done, RedirectTimeout))
        {
            return null; // the handle stays open: the task may still signal it
        }

        Win32.CloseHandle(done);
        return Volatile.Read(ref redirected) == 1;
    }
}
