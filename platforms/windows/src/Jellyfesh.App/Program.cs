using Jellyfesh.App.Platform;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml;
using Microsoft.Windows.AppLifecycle;

namespace Jellyfesh.App;

/// <summary>
/// The entry point (the XAML compiler's Main is turned off with DISABLE_XAML_GENERATED_MAIN): one
/// Jellyfesh per user. A second start hands its activation to the running instance, which comes to
/// the front, and exits (the Windows App SDK's single-instancing pattern).
/// </summary>
public static class Program
{
    private const string InstanceKey = "Jellyfesh.Main";

    [STAThread]
    private static int Main(string[] args)
    {
        _ = args;
        WinRT.ComWrappersSupport.InitializeComWrappers();
        if (RedirectedToRunningInstance())
        {
            return 0;
        }

        Application.Start(start =>
        {
            _ = start;
            var context = new DispatcherQueueSynchronizationContext(DispatcherQueue.GetForCurrentThread());
            SynchronizationContext.SetSynchronizationContext(context);
            _ = new App();
        });
        return 0;
    }

    private static bool RedirectedToRunningInstance()
    {
        AppInstance main;
        try
        {
            main = AppInstance.FindOrRegisterForKey(InstanceKey);
        }
        catch (Exception)
        {
            return false; // better two instances than none
        }

        if (main.IsCurrent)
        {
            // Raised on a background thread when another start redirects here.
            main.Activated += (_, _) => App.BringToFront();
            return false;
        }

        var activation = AppInstance.GetCurrent().GetActivatedEventArgs();
        Win32.AllowSetForegroundWindow(main.ProcessId);

        // Redirect on another thread and wait without blocking COM on this STA thread.
        var done = Win32.CreateEvent(0, true, false, null);
        _ = Task.Run(() =>
        {
            try
            {
                main.RedirectActivationToAsync(activation).AsTask().Wait();
            }
            catch (Exception)
            {
                // The running instance is shutting down; nothing more to do here.
            }
            finally
            {
                Win32.SetEvent(done);
            }
        });
        Win32.WaitPumpingCom(done);
        Win32.CloseHandle(done);
        return true;
    }
}
