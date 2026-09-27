using System.Globalization;
using Microsoft.Win32;

namespace OmarchyThemes.Platform.Windows.Registry;

/// <summary>A typed registry value that round-trips through a snapshot string.</summary>
public sealed record RegValue(RegistryValueKind Kind, object Data)
{
    public static RegValue DWord(uint value) => new(RegistryValueKind.DWord, unchecked((int)value));

    public static RegValue Binary(byte[] value) => new(RegistryValueKind.Binary, value);

    public uint? AsDWord => Data is int i ? unchecked((uint)i) : null;

    /// <summary>"absent" when <paramref name="value"/> is null; otherwise "kind:data".</summary>
    public static string Serialize(RegValue? value) => value switch
    {
        null => "absent",
        { Kind: RegistryValueKind.DWord, Data: int i } => $"dword:{unchecked((uint)i)}",
        { Kind: RegistryValueKind.QWord, Data: long l } => $"qword:{l}",
        { Kind: RegistryValueKind.Binary, Data: byte[] b } => $"binary:{Convert.ToBase64String(b)}",
        { Kind: RegistryValueKind.ExpandString, Data: string s } => $"expand:{s}",
        { Kind: RegistryValueKind.String, Data: string s } => $"string:{s}",
        _ => throw new NotSupportedException($"Registry value kind {value.Kind} isn't supported."),
    };

    public static RegValue? Deserialize(string text)
    {
        if (text == "absent")
            return null;
        var colon = text.IndexOf(':');
        if (colon < 0)
            throw new FormatException($"Invalid registry snapshot value '{text}'.");
        var data = text[(colon + 1)..];
        return text[..colon] switch
        {
            "dword" => DWord(uint.Parse(data, CultureInfo.InvariantCulture)),
            "qword" => new RegValue(RegistryValueKind.QWord, long.Parse(data, CultureInfo.InvariantCulture)),
            "binary" => Binary(Convert.FromBase64String(data)),
            "expand" => new RegValue(RegistryValueKind.ExpandString, data),
            "string" => new RegValue(RegistryValueKind.String, data),
            var kind => throw new FormatException($"Unknown registry value kind '{kind}'."),
        };
    }

    public bool Equals(RegValue? other) =>
        other is not null && Kind == other.Kind && Serialize(this) == Serialize(other);

    public override int GetHashCode() => Serialize(this).GetHashCode();
}

/// <summary>HKEY_CURRENT_USER access. Everything this app changes is per-user; it never touches HKLM.</summary>
public interface IRegistryAccess
{
    RegValue? Read(string key, string name);
    void Write(string key, string name, RegValue value);
    void Delete(string key, string name);
}

public sealed class CurrentUserRegistry : IRegistryAccess
{
    public RegValue? Read(string key, string name)
    {
        using var k = Microsoft.Win32.Registry.CurrentUser.OpenSubKey(key);
        var data = k?.GetValue(name, null, RegistryValueOptions.DoNotExpandEnvironmentNames);
        return data is null ? null : new RegValue(k!.GetValueKind(name), data);
    }

    public void Write(string key, string name, RegValue value)
    {
        using var k = Microsoft.Win32.Registry.CurrentUser.CreateSubKey(key, writable: true);
        k.SetValue(name, value.Data, value.Kind);
    }

    public void Delete(string key, string name)
    {
        using var k = Microsoft.Win32.Registry.CurrentUser.OpenSubKey(key, writable: true);
        k?.DeleteValue(name, throwOnMissingValue: false);
    }
}
