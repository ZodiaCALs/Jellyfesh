using System.Buffers.Binary;

namespace Jellyfesh.Appearance;

/// <summary>
/// Writes Windows .ico files whose images are PNGs (supported since Windows Vista for every
/// size). The app builds the .ico of an icon that ships only as PNGs the first time it is picked
/// (ADR-0017: "rasterised on first selection"), for WM_SETICON and shortcut icons.
/// </summary>
public static class IcoFile
{
    private static readonly byte[] PngSignature = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];

    /// <summary>An .ico holding <paramref name="images"/> (size in px, PNG bytes), smallest first.</summary>
    public static byte[] Write(IEnumerable<(int Size, byte[] Png)> images)
    {
        var list = images.OrderBy(i => i.Size).ToList();
        if (list.Count == 0 || list.Count > ushort.MaxValue)
        {
            throw new ArgumentException("an icon needs between one and 65535 images", nameof(images));
        }

        foreach (var (size, png) in list)
        {
            if (size is < 1 or > 256)
            {
                throw new ArgumentException($"icon images are 1 to 256 px, not {size}", nameof(images));
            }

            if (png.Length < PngSignature.Length || !png.AsSpan(0, PngSignature.Length).SequenceEqual(PngSignature))
            {
                throw new ArgumentException($"the {size} px image is not a PNG", nameof(images));
            }
        }

        const int headerSize = 6;
        const int entrySize = 16;
        var offset = headerSize + (entrySize * list.Count);
        var total = offset + list.Sum(i => i.Png.Length);
        var bytes = new byte[total];
        var span = bytes.AsSpan();
        BinaryPrimitives.WriteUInt16LittleEndian(span[2..], 1); // type: icon
        BinaryPrimitives.WriteUInt16LittleEndian(span[4..], (ushort)list.Count);
        for (var i = 0; i < list.Count; i++)
        {
            var (size, png) = list[i];
            var entry = span.Slice(headerSize + (i * entrySize), entrySize);
            entry[0] = (byte)(size >= 256 ? 0 : size); // 0 means 256
            entry[1] = (byte)(size >= 256 ? 0 : size);
            BinaryPrimitives.WriteUInt16LittleEndian(entry[4..], 1); // planes
            BinaryPrimitives.WriteUInt16LittleEndian(entry[6..], 32); // bits per pixel
            BinaryPrimitives.WriteUInt32LittleEndian(entry[8..], (uint)png.Length);
            BinaryPrimitives.WriteUInt32LittleEndian(entry[12..], (uint)offset);
            png.CopyTo(span[offset..]);
            offset += png.Length;
        }

        return bytes;
    }

    /// <summary>The image sizes in an .ico (for checks and tests).</summary>
    public static IReadOnlyList<int> Sizes(ReadOnlySpan<byte> ico)
    {
        if (ico.Length < 6 || BinaryPrimitives.ReadUInt16LittleEndian(ico[2..]) != 1)
        {
            throw new FormatException("not an icon file");
        }

        var count = BinaryPrimitives.ReadUInt16LittleEndian(ico[4..]);
        var sizes = new List<int>(count);
        for (var i = 0; i < count; i++)
        {
            var w = ico[6 + (i * 16)];
            sizes.Add(w == 0 ? 256 : w);
        }

        return sizes;
    }
}
