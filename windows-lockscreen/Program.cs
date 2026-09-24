using System.Runtime.InteropServices;

namespace AiUsageLockScreen;

internal static class Program
{
    private static readonly Guid ProviderClassId = new("07f8fe4a-6e76-4a69-9c86-894b80444b8e");

    [DllImport("ole32.dll")]
    private static extern int CoRegisterClassObject(
        [MarshalAs(UnmanagedType.LPStruct)] Guid classId,
        [MarshalAs(UnmanagedType.IUnknown)] object factory,
        uint context,
        uint flags,
        out uint cookie);

    [DllImport("ole32.dll")]
    private static extern int CoRevokeClassObject(uint cookie);

    private static int Main(string[] args)
    {
        if (args.Length == 2 && args[0] == "--preview")
        {
            QuotaWidgetProvider.RenderPreview(args[1]);
            return 0;
        }
        var result = CoRegisterClassObject(ProviderClassId, new WidgetProviderFactory<QuotaWidgetProvider>(), 0x4, 0x1, out var cookie);
        if (result < 0) return result;
        using var keepAlive = new ManualResetEventSlim(false);
        keepAlive.Wait();
        return CoRevokeClassObject(cookie);
    }
}
