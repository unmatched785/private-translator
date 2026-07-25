param(
    [string]$OutputDirectory = (Join-Path $PSScriptRoot "..\alpha")
)

Add-Type -AssemblyName System.Drawing

foreach ($size in @(192, 512)) {
    $bitmap = [System.Drawing.Bitmap]::new($size, $size)
    $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
    $graphics.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
    $graphics.TextRenderingHint = [System.Drawing.Text.TextRenderingHint]::AntiAliasGridFit

    try {
        $graphics.Clear([System.Drawing.ColorTranslator]::FromHtml("#0b0f0d"))
        $margin = [int]($size * 0.14)
        $radius = [int]($size * 0.19)
        $rect = [System.Drawing.Rectangle]::new($margin, $margin, $size - (2 * $margin), $size - (2 * $margin))
        $path = [System.Drawing.Drawing2D.GraphicsPath]::new()
        $diameter = 2 * $radius
        $path.AddArc($rect.Left, $rect.Top, $diameter, $diameter, 180, 90)
        $path.AddArc($rect.Right - $diameter, $rect.Top, $diameter, $diameter, 270, 90)
        $path.AddArc($rect.Right - $diameter, $rect.Bottom - $diameter, $diameter, $diameter, 0, 90)
        $path.AddArc($rect.Left, $rect.Bottom - $diameter, $diameter, $diameter, 90, 90)
        $path.CloseFigure()

        $brush = [System.Drawing.SolidBrush]::new([System.Drawing.ColorTranslator]::FromHtml("#69e3a5"))
        $graphics.FillPath($brush, $path)

        $font = [System.Drawing.Font]::new("Arial", $size * 0.45, [System.Drawing.FontStyle]::Bold, [System.Drawing.GraphicsUnit]::Pixel)
        $textBrush = [System.Drawing.SolidBrush]::new([System.Drawing.ColorTranslator]::FromHtml("#082117"))
        $format = [System.Drawing.StringFormat]::new()
        $format.Alignment = [System.Drawing.StringAlignment]::Center
        $format.LineAlignment = [System.Drawing.StringAlignment]::Center
        $graphics.DrawString("P", $font, $textBrush, [System.Drawing.RectangleF]$rect, $format)

        $output = Join-Path $OutputDirectory "icon-$size.png"
        $bitmap.Save($output, [System.Drawing.Imaging.ImageFormat]::Png)
    }
    finally {
        if ($null -ne $format) { $format.Dispose() }
        if ($null -ne $textBrush) { $textBrush.Dispose() }
        if ($null -ne $font) { $font.Dispose() }
        if ($null -ne $brush) { $brush.Dispose() }
        if ($null -ne $path) { $path.Dispose() }
        $graphics.Dispose()
        $bitmap.Dispose()
    }
}
