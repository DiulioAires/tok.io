using System.Drawing;
using System.Drawing.Drawing2D;
using System.Drawing.Imaging;
using System.Drawing.Text;

namespace AiUsageLockScreen;

internal readonly record struct QuotaCardData(
    int? CodexFive,
    int? CodexWeek,
    int? ClaudeFive,
    int? ClaudeWeek,
    string Updated);

internal static class QuotaCardRenderer
{
    private const int Width = 288;
    private const int Height = 100;
    private const int Scale = 2;
    private static readonly Color Background = Color.FromArgb(27, 30, 36);
    private static readonly Color Border = Color.FromArgb(58, 63, 72);
    private static readonly Color Text = Color.FromArgb(244, 245, 247);
    private static readonly Color Muted = Color.FromArgb(159, 165, 175);
    private static readonly Color Track = Color.FromArgb(61, 65, 73);
    private static readonly Color Green = Color.FromArgb(126, 234, 177);
    private static readonly Color Orange = Color.FromArgb(255, 166, 121);

    public static byte[] Render(QuotaCardData data)
    {
        using var bitmap = new Bitmap(Width * Scale, Height * Scale, PixelFormat.Format32bppArgb);
        bitmap.SetResolution(96 * Scale, 96 * Scale);
        using var graphics = Graphics.FromImage(bitmap);
        graphics.SmoothingMode = SmoothingMode.AntiAlias;
        graphics.InterpolationMode = InterpolationMode.HighQualityBicubic;
        graphics.TextRenderingHint = TextRenderingHint.AntiAliasGridFit;
        graphics.Clear(Background);
        graphics.ScaleTransform(Scale, Scale);

        DrawLogo(graphics, "chatgpt.png", 8, 5);
        DrawLogo(graphics, "claude.png", 153, 5);
        DrawText(graphics, "ChatGPT + Codex", 25, 3, 113, 16, 10, Text, FontStyle.Bold);
        DrawText(graphics, "Claude Code", 170, 3, 111, 16, 10, Text, FontStyle.Bold);

        using (var divider = new Pen(Border, 1))
            graphics.DrawLine(divider, 144, 23, 144, 82);

        DrawRing(graphics, 37, data.CodexFive, Green, "5 horas");
        DrawRing(graphics, 107, data.CodexWeek, Green, "Semanal");
        DrawRing(graphics, 181, data.ClaudeFive, Orange, "5 horas");
        DrawRing(graphics, 251, data.ClaudeWeek, Orange, "Semanal");

        using (var divider = new Pen(Border, 1))
            graphics.DrawLine(divider, 8, 86, 280, 86);
        using (var dot = new SolidBrush(Green))
            graphics.FillEllipse(dot, 8, 92, 4, 4);
        DrawText(graphics, "Limites da conta", 16, 88, 122, 12, 8, Muted);
        DrawText(graphics, data.Updated, 166, 88, 114, 12, 8, Muted, alignment: StringAlignment.Far);

        using var output = new MemoryStream();
        bitmap.Save(output, ImageFormat.Png);
        return output.ToArray();
    }

    private static void DrawLogo(Graphics graphics, string file, float x, float y)
    {
        var path = Path.Combine(AppContext.BaseDirectory, "Assets", file);
        if (!File.Exists(path)) return;
        using var image = Image.FromFile(path);
        graphics.DrawImage(image, x, y, 13, 13);
    }

    private static void DrawRing(Graphics graphics, float centerX, int? remaining, Color color, string label)
    {
        const float diameter = 39;
        var circle = new RectangleF(centerX - diameter / 2, 25, diameter, diameter);
        using (var track = new Pen(Track, 4.5f) { StartCap = LineCap.Round, EndCap = LineCap.Round })
            graphics.DrawArc(track, circle, -90, 359.9f);
        if (remaining is > 0)
        {
            using var progress = new Pen(color, 4.5f) { StartCap = LineCap.Round, EndCap = LineCap.Round };
            graphics.DrawArc(progress, circle, -90, Math.Min(359.9f, remaining.Value * 3.6f));
        }
        DrawText(graphics, remaining is null ? "—" : $"{remaining}%", centerX - 21, 36, 42, 18, 12, Text, FontStyle.Bold, StringAlignment.Center);
        DrawText(graphics, label, centerX - 28, 68, 56, 12, 9, Text, FontStyle.Bold, StringAlignment.Center);
    }

    private static void DrawText(
        Graphics graphics,
        string text,
        float x,
        float y,
        float width,
        float height,
        float size,
        Color color,
        FontStyle style = FontStyle.Regular,
        StringAlignment alignment = StringAlignment.Near)
    {
        using var font = new Font("Segoe UI", size, style, GraphicsUnit.Pixel);
        using var brush = new SolidBrush(color);
        using var format = new StringFormat { Alignment = alignment, LineAlignment = StringAlignment.Center, Trimming = StringTrimming.EllipsisCharacter, FormatFlags = StringFormatFlags.NoWrap };
        graphics.DrawString(text, font, brush, new RectangleF(x, y, width, height), format);
    }

}
