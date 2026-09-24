using Microsoft.Windows.Widgets;
using Microsoft.Windows.Widgets.Providers;
using System.Collections.Concurrent;
using System.Text.Json;

namespace AiUsageLockScreen;

internal sealed class QuotaWidgetProvider : IWidgetProvider
{
    private static readonly ConcurrentDictionary<string, byte> WidgetIds = new();
    private static readonly Timer RefreshTimer = new(_ => RefreshAll(), null, TimeSpan.FromMinutes(1), TimeSpan.FromMinutes(1));
    private const string CardTemplate = """
    {
      "$schema": "http://adaptivecards.io/schemas/adaptive-card.json",
      "type": "AdaptiveCard",
      "version": "1.5",
      "body": [
        {
          "type": "Image",
          "url": "${cardImage}",
          "width": "288px",
          "height": "100px",
          "spacing": "None",
          "horizontalAlignment": "Center",
          "altText": "Limites de 5 horas e semanais do ChatGPT e Claude Code"
        }
      ]
    }
    """;

    public QuotaWidgetProvider()
    {
        foreach (var info in WidgetManager.GetDefault().GetWidgetInfos())
        {
            if (info.WidgetContext.DefinitionId == "AiUsage_Limits")
                WidgetIds.TryAdd(info.WidgetContext.Id, 0);
        }
        RefreshAll();
        _ = RefreshTimer;
    }

    public void CreateWidget(WidgetContext context)
    {
        WidgetIds.TryAdd(context.Id, 0);
        Update(context.Id);
    }

    public void DeleteWidget(string widgetId, string customState) => WidgetIds.TryRemove(widgetId, out _);

    public void OnActionInvoked(WidgetActionInvokedArgs args) => Update(args.WidgetContext.Id);

    public void OnWidgetContextChanged(WidgetContextChangedArgs args) => Update(args.WidgetContext.Id);

    public void Activate(WidgetContext context) => Update(context.Id);

    public void Deactivate(string widgetId) { }

    private static void RefreshAll()
    {
        foreach (var id in WidgetIds.Keys) Update(id);
    }

    private static int? Remaining(JsonElement provider, string window)
    {
        if (provider.ValueKind != JsonValueKind.Object ||
            !provider.TryGetProperty(window, out var quota) || quota.ValueKind != JsonValueKind.Object ||
            !quota.TryGetProperty("usedPercent", out var used) || !used.TryGetDouble(out var usedPercent))
            return null;
        return Math.Clamp((int)Math.Round(100 - usedPercent), 0, 100);
    }

    private static QuotaCardData LoadData()
    {
        var path = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.UserProfile), ".ai-usage-widget", "lock-screen.json");
        try
        {
            using var file = File.OpenRead(path);
            using var document = JsonDocument.Parse(file);
            var root = document.RootElement;
            if (!root.TryGetProperty("updatedAt", out var updatedAt) ||
                DateTimeOffset.UtcNow - DateTimeOffset.FromUnixTimeSeconds(updatedAt.GetInt64()) > TimeSpan.FromMinutes(10))
                throw new InvalidDataException("Snapshot antigo");

            root.TryGetProperty("codex", out var codex);
            root.TryGetProperty("claude", out var claude);
            var time = DateTimeOffset.FromUnixTimeSeconds(updatedAt.GetInt64()).ToLocalTime();
            return new QuotaCardData(
                Remaining(codex, "fiveHour"),
                Remaining(codex, "weekly"),
                Remaining(claude, "fiveHour"),
                Remaining(claude, "weekly"),
                $"Atualizado {time:HH:mm}");
        }
        catch
        {
            return new QuotaCardData(null, null, null, null, "Abra o tok.io para atualizar");
        }
    }

    internal static void RenderPreview(string path) => File.WriteAllBytes(path, QuotaCardRenderer.Render(LoadData()));

    private static void Update(string widgetId)
    {
        try
        {
            var image = Convert.ToBase64String(QuotaCardRenderer.Render(LoadData()));
            var request = new WidgetUpdateRequestOptions(widgetId) {
                Template = CardTemplate,
                Data = JsonSerializer.Serialize(new { cardImage = $"data:image/png;base64,{image}" })
            };
            WidgetManager.GetDefault().UpdateWidget(request);
        }
        catch { /* The host may have removed the widget while an update was in flight. */ }
    }
}
