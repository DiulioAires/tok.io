using Microsoft.Windows.Widgets.Providers;
using System.Runtime.InteropServices;
using WinRT;

namespace AiUsageLockScreen;

[ComImport, InterfaceType(ComInterfaceType.InterfaceIsIUnknown), Guid("00000001-0000-0000-C000-000000000046")]
internal interface IClassFactory
{
    [PreserveSig]
    int CreateInstance(IntPtr outer, ref Guid interfaceId, out IntPtr instance);

    [PreserveSig]
    int LockServer(bool locked);
}

[ComVisible(true)]
internal sealed class WidgetProviderFactory<T> : IClassFactory where T : IWidgetProvider, new()
{
    public int CreateInstance(IntPtr outer, ref Guid interfaceId, out IntPtr instance)
    {
        instance = IntPtr.Zero;
        if (outer != IntPtr.Zero) return unchecked((int)0x80040110);
        if (interfaceId != typeof(T).GUID && interfaceId != new Guid("00000000-0000-0000-C000-000000000046"))
            return unchecked((int)0x80004002);
        instance = MarshalInspectable<IWidgetProvider>.FromManaged(new T());
        return 0;
    }

    public int LockServer(bool locked) => 0;
}
