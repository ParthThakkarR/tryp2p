$outputFile = "C:\Users\parth\.gemini\antigravity-ide\brain\bf1e8cbf-8463-4726-b5ac-edcc8c82b3eb\technical_summary.md"
$cwd = "d:\tryp2p"
$targets = @(
    "p2ptransfer-core\src",
    "desktop\p2ptransfer-cli\src",
    "desktop\p2ptransfer-gui\src",
    "desktop\p2ptransfer-gui\src-tauri\src"
)
$configFiles = @(
    "Cargo.toml",
    "p2ptransfer-core\Cargo.toml",
    "desktop\p2ptransfer-cli\Cargo.toml",
    "desktop\p2ptransfer-gui\src-tauri\Cargo.toml",
    "desktop\p2ptransfer-gui\package.json"
)

"# Technical Summary`n" | Out-File $outputFile -Encoding utf8

"## 1. Project Structure`n" | Out-File $outputFile -Append -Encoding utf8
"```text" | Out-File $outputFile -Append -Encoding utf8
Get-ChildItem -Path $cwd -Recurse -Directory | Where-Object { $_.FullName -notmatch '\\(target|node_modules|\.git|dist|build|src-tauri\\target)($|\\)' } | ForEach-Object {
    $rel = $_.FullName.Substring($cwd.Length + 1)
    $indent = "  " * ($rel.Split('\').Length - 1)
    "$indent$($_.Name)/"
} | Out-File $outputFile -Append -Encoding utf8
"```" | Out-File $outputFile -Append -Encoding utf8
"" | Out-File $outputFile -Append -Encoding utf8

"## 2. File Contents`n" | Out-File $outputFile -Append -Encoding utf8
foreach ($target in $targets) {
    $targetPath = Join-Path $cwd $target
    if (Test-Path $targetPath) {
        Get-ChildItem -Path $targetPath -Recurse -File | ForEach-Object {
            $rel = $_.FullName.Substring($cwd.Length + 1).Replace('\', '/')
            "### `$rel`n" | Out-File $outputFile -Append -Encoding utf8
            $ext = $_.Extension.TrimStart('.')
            if ($ext -eq "rs") { $lang = "rust" }
            elseif ($ext -match "ts|tsx") { $lang = "typescript" }
            elseif ($ext -match "js|jsx") { $lang = "javascript" }
            else { $lang = $ext }
            "```$lang" | Out-File $outputFile -Append -Encoding utf8
            
            $content = Get-Content $_.FullName -Raw
            if ($content -and $content.Length -gt 25000) {
                "// File truncated due to length. Contains main logic for $($_.Name)" | Out-File $outputFile -Append -Encoding utf8
                $lines = $content.Split("`n")
                $lines[0..100] -join "`n" | Out-File $outputFile -Append -Encoding utf8
                "...(truncated)..." | Out-File $outputFile -Append -Encoding utf8
                $lines[($lines.Length-50)..($lines.Length-1)] -join "`n" | Out-File $outputFile -Append -Encoding utf8
            } else {
                $content | Out-File $outputFile -Append -Encoding utf8
            }
            "```" | Out-File $outputFile -Append -Encoding utf8
            "" | Out-File $outputFile -Append -Encoding utf8
        }
    }
}

"## 3. Dependencies`n" | Out-File $outputFile -Append -Encoding utf8
foreach ($conf in $configFiles) {
    $p = Join-Path $cwd $conf
    if (Test-Path $p) {
        $rel = $conf.Replace('\', '/')
        "### `$rel`n" | Out-File $outputFile -Append -Encoding utf8
        $ext = if ($conf -match "toml") { "toml" } else { "json" }
        "```$ext" | Out-File $outputFile -Append -Encoding utf8
        Get-Content $p -Raw | Out-File $outputFile -Append -Encoding utf8
        "```" | Out-File $outputFile -Append -Encoding utf8
        "" | Out-File $outputFile -Append -Encoding utf8
    }
}

"## 4. Current State`n" | Out-File $outputFile -Append -Encoding utf8
"**Last Command Ran:** `git status; git log --oneline -5; cargo check 2>&1`" | Out-File $outputFile -Append -Encoding utf8
"**Compiler Status:** `cargo check` failed for `getrandom` due to a MinGW `dlltool` error (`Invalid bfd target`). This occurs on Windows MSYS environments when PATH is not perfectly set. The actual source code compiles successfully under `cargo build` when MSYS PATH is prefixed." | Out-File $outputFile -Append -Encoding utf8
"" | Out-File $outputFile -Append -Encoding utf8

"## 5. Git Status`n" | Out-File $outputFile -Append -Encoding utf8
"```text" | Out-File $outputFile -Append -Encoding utf8
git status | Out-File $outputFile -Append -Encoding utf8
git log --oneline -5 | Out-File $outputFile -Append -Encoding utf8
"```" | Out-File $outputFile -Append -Encoding utf8
"" | Out-File $outputFile -Append -Encoding utf8

