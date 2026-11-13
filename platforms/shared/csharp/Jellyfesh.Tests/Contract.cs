using System;
using System.IO;
using System.Linq;
using System.Reflection;
using System.Text.Json;

namespace Jellyfesh.Tests;

/// <summary>The shared contract files in tests/contract/.</summary>
internal static class Contract
{
    public static string RepoRoot { get; } = FindRoot();

    public static JsonDocument Read(string name) =>
        JsonDocument.Parse(File.ReadAllText(Path.Combine(RepoRoot, "tests", "contract", name)));

    private static string FindRoot()
    {
        string? root = typeof(Contract).Assembly.GetCustomAttributes<AssemblyMetadataAttribute>()
            .FirstOrDefault(a => a.Key == "JfRepoRoot")?.Value;
        if (root is not null && Directory.Exists(Path.Combine(root, "tests", "contract")))
        {
            return root;
        }
        for (DirectoryInfo? dir = new(AppContext.BaseDirectory); dir is not null; dir = dir.Parent)
        {
            if (Directory.Exists(Path.Combine(dir.FullName, "tests", "contract")))
            {
                return dir.FullName;
            }
        }
        throw new InvalidOperationException("tests/contract not found above " + AppContext.BaseDirectory);
    }

    /// <summary>JSON equality that ignores property order and number formatting (1.0 and 1).</summary>
    public static bool Same(JsonElement a, JsonElement b)
    {
        if (a.ValueKind != b.ValueKind)
        {
            return false;
        }
        switch (a.ValueKind)
        {
            case JsonValueKind.Object:
                int count = 0;
                foreach (JsonProperty property in a.EnumerateObject())
                {
                    count++;
                    if (!b.TryGetProperty(property.Name, out JsonElement other) || !Same(property.Value, other))
                    {
                        return false;
                    }
                }
                return count == b.EnumerateObject().Count();
            case JsonValueKind.Array:
                return a.GetArrayLength() == b.GetArrayLength()
                    && a.EnumerateArray().Zip(b.EnumerateArray()).All(pair => Same(pair.First, pair.Second));
            case JsonValueKind.String:
                return a.GetString() == b.GetString();
            case JsonValueKind.Number:
                if (a.TryGetInt64(out long x) && b.TryGetInt64(out long y))
                {
                    return x == y;
                }
                if (a.TryGetUInt64(out ulong ux) && b.TryGetUInt64(out ulong uy))
                {
                    return ux == uy;
                }
                return a.GetDouble() == b.GetDouble();
            default:
                return true;
        }
    }
}
