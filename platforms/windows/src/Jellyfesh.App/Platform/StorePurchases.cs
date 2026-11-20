#if JF_DISTRIBUTION_MICROSOFT_STORE
using Jellyfesh.App.Shell;
using Jellyfesh.Core;
using Jellyfesh.ViewModels;
using Windows.Services.Store;

namespace Jellyfesh.App.Platform;

/// <summary>
/// Jellyfesh Pro through the Microsoft Store (Microsoft Store builds only; ADR-0021 §4). The Pro
/// add-on is a durable add-on whose product ID (its InAppOfferToken) is org.jellyfesh.pro. The
/// Store's licence for it is reported to the core with SetStoreEntitlement at start and whenever
/// the Store says the licences changed (OfflineLicensesChanged); the core decides what Pro is.
/// StoreContext is a desktop app's: its dialogs need the window (InitializeWithWindow), as the
/// Windows.Services.Store documentation for Windows App SDK apps says.
/// </summary>
internal sealed class StorePurchases : IProStore
{
    /// <summary>The Pro add-on's product ID in Partner Center (docs/core-api.md, "Jellyfesh Pro").</summary>
    public const string ProductId = "org.jellyfesh.pro";

    private static readonly string[] DurableKinds = ["Durable"];

    private readonly StoreContext _context;
    private readonly ICoreClient _core;

    private StorePurchases(StoreContext context, ICoreClient core)
    {
        _context = context;
        _core = core;
        _context.OfflineLicensesChanged += (_, _) => _ = ReportAsync();
    }

    /// <summary>The glue for the window <paramref name="hwnd"/>, reporting the licence once at start.</summary>
    public static StorePurchases Create(nint hwnd, ICoreClient core)
    {
        var context = StoreContext.GetDefault();
        WinRT.Interop.InitializeWithWindow.Initialize(context, hwnd);
        var store = new StorePurchases(context, core);
        _ = store.ReportAsync();
        return store;
    }

    public async Task<string?> PriceAsync() => (await FindAsync())?.Price?.FormattedPrice;

    public async Task<ProPurchase> BuyAsync()
    {
        try
        {
            var product = await FindAsync();
            if (product is null)
            {
                return ProPurchase.Failed;
            }

            var result = await _context.RequestPurchaseAsync(product.StoreId);
            await ReportAsync();
            return result.Status switch
            {
                StorePurchaseStatus.Succeeded => ProPurchase.Bought,
                StorePurchaseStatus.AlreadyPurchased => ProPurchase.AlreadyOwned,
                StorePurchaseStatus.NotPurchased => ProPurchase.Cancelled,
                _ => Failed(result.ExtendedError, "buying Jellyfesh Pro"),
            };
        }
        catch (Exception e)
        {
            return Failed(e, "buying Jellyfesh Pro");
        }
    }

    public Task RestoreAsync() => ReportAsync();

    /// <summary>The Store's licence for Pro, to the core: active, or not (after a refund).</summary>
    private async Task ReportAsync()
    {
        try
        {
            var licence = await _context.GetAppLicenseAsync();
            var addOn = licence.AddOnLicenses.Values.FirstOrDefault(l => l.InAppOfferToken == ProductId);
            _core.SetStoreEntitlement(new StoreEntitlement(ProStore.MicrosoftStore, ProductId, addOn?.IsActive ?? false, addOn?.SkuStoreId));
        }
        catch (Exception e)
        {
            AppLog.Write("reading the Microsoft Store's licence for Jellyfesh Pro failed", e);
        }
    }

    private async Task<StoreProduct?> FindAsync()
    {
        try
        {
            var result = await _context.GetAssociatedStoreProductsAsync(DurableKinds);
            if (result.ExtendedError is { } error)
            {
                AppLog.Write("the Microsoft Store did not list Jellyfesh Pro", error);
            }

            return result.Products.Values.FirstOrDefault(p => p.InAppOfferToken == ProductId);
        }
        catch (Exception e)
        {
            AppLog.Write("the Microsoft Store could not be reached", e);
            return null;
        }
    }

    private static ProPurchase Failed(Exception? error, string what)
    {
        AppLog.Write($"{what} failed", error);
        return ProPurchase.Failed;
    }
}
#endif
