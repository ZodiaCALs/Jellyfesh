using System.Text;

namespace Jellyfesh.Qr;

public enum QrErrorCorrection
{
    Low,
    Medium,
    Quartile,
    High,
}

/// <summary>
/// A QR code (ISO/IEC 18004) for sign-in flows that show a code for a phone to scan (AuthStep
/// ShowQr). Byte mode, versions 1–40, the standard mask choice. Written from the standard and
/// checked against an independent encoder in the tests; small enough not to need a dependency.
/// </summary>
public sealed class QrCode
{
    private static readonly int[,] EccPerBlock =
    {
        { -1, 7, 10, 15, 20, 26, 18, 20, 24, 30, 18, 20, 24, 26, 30, 22, 24, 28, 30, 28, 28, 28, 28, 30, 30, 26, 28, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30 },
        { -1, 10, 16, 26, 18, 24, 16, 18, 22, 22, 26, 30, 22, 22, 24, 24, 28, 28, 26, 26, 26, 26, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28 },
        { -1, 13, 22, 18, 26, 18, 24, 18, 22, 20, 24, 28, 26, 24, 20, 30, 24, 28, 28, 26, 30, 28, 30, 30, 30, 30, 28, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30 },
        { -1, 17, 28, 22, 16, 22, 28, 26, 26, 24, 28, 24, 28, 22, 24, 24, 30, 28, 28, 26, 28, 30, 24, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30 },
    };

    private static readonly int[,] BlockCount =
    {
        { -1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 4, 4, 4, 4, 4, 6, 6, 6, 6, 7, 8, 8, 9, 9, 10, 12, 12, 12, 13, 14, 15, 16, 17, 18, 19, 19, 20, 21, 22, 24, 25 },
        { -1, 1, 1, 1, 2, 2, 4, 4, 4, 5, 5, 5, 8, 9, 9, 10, 10, 11, 13, 14, 16, 17, 17, 18, 20, 21, 23, 25, 26, 28, 29, 31, 33, 35, 37, 38, 40, 43, 45, 47, 49 },
        { -1, 1, 1, 2, 2, 4, 4, 6, 6, 8, 8, 8, 10, 12, 16, 12, 17, 16, 18, 21, 20, 23, 23, 25, 27, 29, 34, 34, 35, 38, 40, 43, 45, 48, 51, 53, 56, 59, 62, 65, 68 },
        { -1, 1, 1, 2, 4, 4, 4, 5, 6, 8, 8, 11, 11, 16, 16, 18, 16, 19, 21, 25, 25, 25, 34, 30, 32, 35, 37, 40, 42, 45, 48, 51, 54, 57, 60, 63, 66, 70, 74, 77, 81 },
    };

    private readonly bool[,] _modules;
    private readonly bool[,] _isFunction;

    private QrCode(int version, QrErrorCorrection ecl, byte[] dataCodewords, int? forcedMask)
    {
        Version = version;
        ErrorCorrection = ecl;
        Size = (version * 4) + 17;
        _modules = new bool[Size, Size];
        _isFunction = new bool[Size, Size];
        DrawFunctionPatterns();
        DrawCodewords(AddEccAndInterleave(dataCodewords));
        var mask = forcedMask ?? ChooseMask();
        ApplyMask(mask);
        DrawFormatBits(mask);
        Mask = mask;
    }

    public int Version { get; }

    public int Size { get; }

    public int Mask { get; }

    public QrErrorCorrection ErrorCorrection { get; }

    /// <summary>True for a dark module at (<paramref name="x"/> column, <paramref name="y"/> row).</summary>
    public bool this[int x, int y] => x >= 0 && y >= 0 && x < Size && y < Size && _modules[y, x];

    /// <summary>Encodes UTF-8 text in byte mode at the smallest version that fits.</summary>
    public static QrCode Encode(string text, QrErrorCorrection ecl = QrErrorCorrection.Medium, int? mask = null, int minVersion = 1)
    {
        var data = Encoding.UTF8.GetBytes(text);
        for (var version = Math.Clamp(minVersion, 1, 40); version <= 40; version++)
        {
            var countBits = version <= 9 ? 8 : 16;
            var capacityBits = DataCodewords(version, ecl) * 8;
            var usedBits = 4 + countBits + (data.Length * 8);
            if (data.Length < (1 << countBits) && usedBits <= capacityBits)
            {
                return new QrCode(version, ecl, Pack(data, countBits, capacityBits), mask);
            }
        }

        throw new ArgumentException("the text is too long for a QR code", nameof(text));
    }

    /// <summary>Rows of '#' and '.', for tests and debugging.</summary>
    public override string ToString()
    {
        var sb = new StringBuilder();
        for (var y = 0; y < Size; y++)
        {
            for (var x = 0; x < Size; x++)
            {
                sb.Append(_modules[y, x] ? '#' : '.');
            }

            sb.Append('\n');
        }

        return sb.ToString();
    }

    private static byte[] Pack(byte[] data, int countBits, int capacityBits)
    {
        var bits = new List<bool>(capacityBits);
        void Append(int value, int length)
        {
            for (var i = length - 1; i >= 0; i--)
            {
                bits.Add(((value >> i) & 1) != 0);
            }
        }

        Append(0b0100, 4); // byte mode
        Append(data.Length, countBits);
        foreach (var b in data)
        {
            Append(b, 8);
        }

        Append(0, Math.Min(4, capacityBits - bits.Count));
        Append(0, (8 - (bits.Count % 8)) % 8);
        for (var pad = 0xEC; bits.Count < capacityBits; pad ^= 0xEC ^ 0x11)
        {
            Append(pad, 8);
        }

        var bytes = new byte[bits.Count / 8];
        for (var i = 0; i < bits.Count; i++)
        {
            if (bits[i])
            {
                bytes[i >> 3] |= (byte)(1 << (7 - (i & 7)));
            }
        }

        return bytes;
    }

    private static int RawDataModules(int version)
    {
        var result = ((16 * version) + 128) * version + 64;
        if (version >= 2)
        {
            var align = (version / 7) + 2;
            result -= ((25 * align) - 10) * align - 55;
            if (version >= 7)
            {
                result -= 36;
            }
        }

        return result;
    }

    private static int DataCodewords(int version, QrErrorCorrection ecl) =>
        (RawDataModules(version) / 8) - (EccPerBlock[(int)ecl, version] * BlockCount[(int)ecl, version]);

    private byte[] AddEccAndInterleave(byte[] data)
    {
        var blocks = BlockCount[(int)ErrorCorrection, Version];
        var eccLen = EccPerBlock[(int)ErrorCorrection, Version];
        var raw = RawDataModules(Version) / 8;
        var shortBlocks = blocks - (raw % blocks);
        var shortLen = raw / blocks;
        var divisor = ReedSolomon.Divisor(eccLen);
        var all = new List<byte[]>(blocks);
        for (int i = 0, k = 0; i < blocks; i++)
        {
            var len = shortLen - eccLen + (i < shortBlocks ? 0 : 1);
            var dat = data.AsSpan(k, len).ToArray();
            k += len;
            var ecc = ReedSolomon.Remainder(dat, divisor);
            var block = new byte[shortLen + 1];
            dat.CopyTo(block, 0);
            // Short blocks get a placeholder byte so every block has the same layout.
            ecc.CopyTo(block, len + (i < shortBlocks ? 1 : 0));
            all.Add(block);
        }

        var result = new byte[raw];
        var n = 0;
        for (var i = 0; i <= shortLen; i++)
        {
            for (var j = 0; j < blocks; j++)
            {
                if (i != shortLen - eccLen || j >= shortBlocks)
                {
                    result[n++] = all[j][i];
                }
            }
        }

        return result;
    }

    private void DrawFunctionPatterns()
    {
        for (var i = 0; i < Size; i++)
        {
            SetFunction(6, i, i % 2 == 0);
            SetFunction(i, 6, i % 2 == 0);
        }

        DrawFinder(3, 3);
        DrawFinder(Size - 4, 3);
        DrawFinder(3, Size - 4);
        var positions = AlignmentPositions();
        var count = positions.Length;
        for (var i = 0; i < count; i++)
        {
            for (var j = 0; j < count; j++)
            {
                if (!((i == 0 && j == 0) || (i == 0 && j == count - 1) || (i == count - 1 && j == 0)))
                {
                    DrawAlignment(positions[i], positions[j]);
                }
            }
        }

        DrawFormatBits(0); // reserves the area; drawn again after masking
        DrawVersion();
    }

    private int[] AlignmentPositions()
    {
        if (Version == 1)
        {
            return [];
        }

        var count = (Version / 7) + 2;
        var step = Version == 32 ? 26 : (((Version * 4) + (count * 2) + 1) / ((count * 2) - 2)) * 2;
        var result = new int[count];
        result[0] = 6;
        for (int i = count - 1, pos = Size - 7; i >= 1; i--, pos -= step)
        {
            result[i] = pos;
        }

        return result;
    }

    private void DrawFinder(int cx, int cy)
    {
        for (var dy = -4; dy <= 4; dy++)
        {
            for (var dx = -4; dx <= 4; dx++)
            {
                var dist = Math.Max(Math.Abs(dx), Math.Abs(dy));
                int x = cx + dx, y = cy + dy;
                if (x >= 0 && x < Size && y >= 0 && y < Size)
                {
                    SetFunction(x, y, dist != 2 && dist != 4);
                }
            }
        }
    }

    private void DrawAlignment(int cx, int cy)
    {
        for (var dy = -2; dy <= 2; dy++)
        {
            for (var dx = -2; dx <= 2; dx++)
            {
                SetFunction(cx + dx, cy + dy, Math.Max(Math.Abs(dx), Math.Abs(dy)) != 1);
            }
        }
    }

    private void DrawFormatBits(int mask)
    {
        var eclBits = ErrorCorrection switch
        {
            QrErrorCorrection.Low => 1,
            QrErrorCorrection.Medium => 0,
            QrErrorCorrection.Quartile => 3,
            _ => 2,
        };
        var data = (eclBits << 3) | mask;
        var rem = data;
        for (var i = 0; i < 10; i++)
        {
            rem = (rem << 1) ^ ((rem >> 9) * 0x537);
        }

        var bits = ((data << 10) | rem) ^ 0x5412;
        for (var i = 0; i <= 5; i++)
        {
            SetFunction(8, i, Bit(bits, i));
        }

        SetFunction(8, 7, Bit(bits, 6));
        SetFunction(8, 8, Bit(bits, 7));
        SetFunction(7, 8, Bit(bits, 8));
        for (var i = 9; i < 15; i++)
        {
            SetFunction(14 - i, 8, Bit(bits, i));
        }

        for (var i = 0; i < 8; i++)
        {
            SetFunction(Size - 1 - i, 8, Bit(bits, i));
        }

        for (var i = 8; i < 15; i++)
        {
            SetFunction(8, Size - 15 + i, Bit(bits, i));
        }

        SetFunction(8, Size - 8, true); // the dark module
    }

    private void DrawVersion()
    {
        if (Version < 7)
        {
            return;
        }

        var rem = Version;
        for (var i = 0; i < 12; i++)
        {
            rem = (rem << 1) ^ ((rem >> 11) * 0x1F25);
        }

        var bits = (Version << 12) | rem;
        for (var i = 0; i < 18; i++)
        {
            var bit = Bit(bits, i);
            int a = Size - 11 + (i % 3), b = i / 3;
            SetFunction(a, b, bit);
            SetFunction(b, a, bit);
        }
    }

    private void DrawCodewords(byte[] data)
    {
        var i = 0;
        for (var right = Size - 1; right >= 1; right -= 2)
        {
            if (right == 6)
            {
                right = 5;
            }

            for (var vert = 0; vert < Size; vert++)
            {
                for (var j = 0; j < 2; j++)
                {
                    var x = right - j;
                    var upward = ((right + 1) & 2) == 0;
                    var y = upward ? Size - 1 - vert : vert;
                    if (!_isFunction[y, x] && i < data.Length * 8)
                    {
                        _modules[y, x] = ((data[i >> 3] >> (7 - (i & 7))) & 1) != 0;
                        i++;
                    }
                }
            }
        }
    }

    private void ApplyMask(int mask)
    {
        for (var y = 0; y < Size; y++)
        {
            for (var x = 0; x < Size; x++)
            {
                var invert = mask switch
                {
                    0 => (x + y) % 2 == 0,
                    1 => y % 2 == 0,
                    2 => x % 3 == 0,
                    3 => (x + y) % 3 == 0,
                    4 => ((x / 3) + (y / 2)) % 2 == 0,
                    5 => ((x * y) % 2) + ((x * y) % 3) == 0,
                    6 => (((x * y) % 2) + ((x * y) % 3)) % 2 == 0,
                    _ => (((x + y) % 2) + ((x * y) % 3)) % 2 == 0,
                };
                _modules[y, x] ^= invert && !_isFunction[y, x];
            }
        }
    }

    private int ChooseMask()
    {
        var best = 0;
        var bestPenalty = int.MaxValue;
        for (var m = 0; m < 8; m++)
        {
            ApplyMask(m);
            DrawFormatBits(m);
            var penalty = Penalty();
            if (penalty < bestPenalty)
            {
                best = m;
                bestPenalty = penalty;
            }

            ApplyMask(m); // undo (XOR)
        }

        return best;
    }

    private int Penalty()
    {
        var result = 0;
        var history = new int[7];
        for (var pass = 0; pass < 2; pass++)
        {
            for (var a = 0; a < Size; a++)
            {
                var runColor = false;
                var run = 0;
                Array.Clear(history);
                for (var b = 0; b < Size; b++)
                {
                    var color = pass == 0 ? _modules[a, b] : _modules[b, a];
                    if (color == runColor)
                    {
                        run++;
                        if (run == 5)
                        {
                            result += 3;
                        }
                        else if (run > 5)
                        {
                            result++;
                        }
                    }
                    else
                    {
                        AddHistory(run, history);
                        if (!runColor)
                        {
                            result += CountFinderLike(history) * 40;
                        }

                        runColor = color;
                        run = 1;
                    }
                }

                if (runColor)
                {
                    AddHistory(run, history);
                    run = 0;
                }

                AddHistory(run + Size, history);
                result += CountFinderLike(history) * 40;
            }
        }

        var dark = 0;
        for (var y = 0; y < Size; y++)
        {
            for (var x = 0; x < Size; x++)
            {
                if (_modules[y, x])
                {
                    dark++;
                }

                if (y < Size - 1 && x < Size - 1)
                {
                    var c = _modules[y, x];
                    if (c == _modules[y, x + 1] && c == _modules[y + 1, x] && c == _modules[y + 1, x + 1])
                    {
                        result += 3;
                    }
                }
            }
        }

        var total = Size * Size;
        var k = ((Math.Abs((dark * 20) - (total * 10)) + total - 1) / total) - 1;
        return result + (k * 10);
    }

    private void AddHistory(int run, int[] history)
    {
        if (history[0] == 0)
        {
            run += Size; // the light border before the first run
        }

        Array.Copy(history, 0, history, 1, history.Length - 1);
        history[0] = run;
    }

    private static int CountFinderLike(int[] h)
    {
        var n = h[1];
        var core = n > 0 && h[2] == n && h[3] == n * 3 && h[4] == n && h[5] == n;
        return (core && h[0] >= n * 4 && h[6] >= n ? 1 : 0) + (core && h[6] >= n * 4 && h[0] >= n ? 1 : 0);
    }

    private void SetFunction(int x, int y, bool dark)
    {
        _modules[y, x] = dark;
        _isFunction[y, x] = true;
    }

    private static bool Bit(int value, int i) => ((value >> i) & 1) != 0;

    private static class ReedSolomon
    {
        public static byte[] Divisor(int degree)
        {
            var result = new byte[degree];
            result[degree - 1] = 1;
            byte root = 1;
            for (var i = 0; i < degree; i++)
            {
                for (var j = 0; j < result.Length; j++)
                {
                    result[j] = Multiply(result[j], root);
                    if (j + 1 < result.Length)
                    {
                        result[j] ^= result[j + 1];
                    }
                }

                root = Multiply(root, 0x02);
            }

            return result;
        }

        public static byte[] Remainder(byte[] data, byte[] divisor)
        {
            var result = new byte[divisor.Length];
            foreach (var b in data)
            {
                var factor = (byte)(b ^ result[0]);
                Array.Copy(result, 1, result, 0, result.Length - 1);
                result[^1] = 0;
                for (var i = 0; i < result.Length; i++)
                {
                    result[i] ^= Multiply(divisor[i], factor);
                }
            }

            return result;
        }

        private static byte Multiply(byte x, byte y)
        {
            var z = 0;
            for (var i = 7; i >= 0; i--)
            {
                z = (z << 1) ^ ((z >> 7) * 0x11D);
                z ^= ((y >> i) & 1) * x;
            }

            return (byte)z;
        }
    }
}
