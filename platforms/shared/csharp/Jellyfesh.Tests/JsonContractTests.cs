using System.Collections.Generic;
using System.Text.Json;
using System.Text.Json.Serialization.Metadata;
using Jellyfesh.Ffi;
using Xunit;

namespace Jellyfesh.Tests;

/// <summary>Jellyfesh.Ffi's records read and write tests/contract/ffi-types.json unchanged.</summary>
public class JsonContractTests
{
    [Fact]
    public void EverySampleRoundTrips()
    {
        using JsonDocument contract = Contract.Read("ffi-types.json");
        Assert.Equal(1, contract.RootElement.GetProperty("version").GetInt32());
        var seen = new HashSet<string>();
        int count = 0;
        foreach (JsonElement sample in contract.RootElement.GetProperty("samples").EnumerateArray())
        {
            string name = sample.GetProperty("name").GetString()!;
            JsonElement json = sample.GetProperty("json");
            string written = name switch
            {
                "CoreConfig" => RoundTrip(json, JellyfeshJsonContext.Default.CoreConfig),
                "Command" => RoundTrip(json, JellyfeshJsonContext.Default.Command),
                "Event" => RoundTrip(json, JellyfeshJsonContext.Default.Event),
                "Snapshot" => RoundTrip(json, JellyfeshJsonContext.Default.Snapshot),
                "TrackRef" => RoundTrip(json, JellyfeshJsonContext.Default.TrackRef),
                "TrackQuery" => RoundTrip(json, JellyfeshJsonContext.Default.TrackQuery),
                "Track" => RoundTrip(json, JellyfeshJsonContext.Default.Track),
                "Album" => RoundTrip(json, JellyfeshJsonContext.Default.Album),
                "Artist" => RoundTrip(json, JellyfeshJsonContext.Default.Artist),
                "SearchResults" => RoundTrip(json, JellyfeshJsonContext.Default.SearchResults),
                "LibraryStats" => RoundTrip(json, JellyfeshJsonContext.Default.LibraryStats),
                "TrackRow" => RoundTrip(json, JellyfeshJsonContext.Default.TrackRow),
                "PlaylistSummary" => RoundTrip(json, JellyfeshJsonContext.Default.PlaylistSummary),
                "PlaylistEntry" => RoundTrip(json, JellyfeshJsonContext.Default.PlaylistEntry),
                "QueueItem" => RoundTrip(json, JellyfeshJsonContext.Default.QueueItem),
                "HistoryEntry" => RoundTrip(json, JellyfeshJsonContext.Default.HistoryEntry),
                "DeviceInfo" => RoundTrip(json, JellyfeshJsonContext.Default.DeviceInfo),
                "EqPreset" => RoundTrip(json, JellyfeshJsonContext.Default.EqPreset),
                _ => throw new Xunit.Sdk.XunitException($"no C# type for sample {name}"),
            };
            using JsonDocument back = JsonDocument.Parse(written);
            Assert.True(Contract.Same(json, back.RootElement), $"{name} changed:\n{json.GetRawText()}\n{written}");
            seen.Add(name);
            count++;
        }
        Assert.True(count > 100, $"only {count} samples");
        Assert.Equal(18, seen.Count);
    }

    [Fact]
    public void VariantNamesAndRenamedFields()
    {
        string json = JsonSerializer.Serialize<Command>(new Command.SetRepeat(RepeatMode.One), JellyfeshJsonContext.Default.Command);
        Assert.Equal("""{"type":"SetRepeat","mode":"One"}""", json);
        Event ev = JsonSerializer.Deserialize("""{"type":"CommandFailed","id":3,"error":"nothing to play"}""", JellyfeshJsonContext.Default.Event)!;
        Assert.Equal(new Event.CommandFailed(3, "nothing to play"), ev);
        Collection tracks = JsonSerializer.Deserialize("""{"type":"Tracks","tracks":[{"type":"Library","id":1}]}""", JellyfeshJsonContext.Default.Collection)!;
        Assert.Equal(new TrackRef.Library(1), Assert.IsType<Collection.Tracks>(tracks).Value[0]);
    }

    private static string RoundTrip<T>(JsonElement json, JsonTypeInfo<T> info)
    {
        T value = json.Deserialize(info)!;
        Assert.NotNull(value);
        return JsonSerializer.Serialize(value, info);
    }
}
