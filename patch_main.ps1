$content = Get-Content -Raw 'desktop/p2ptransfer-cli/src/main.rs'
$patch = Get-Content -Raw 'patch_body.rs'

$start_marker = '    // --- Signal handling for graceful pause ---'
$end_marker = "Ok(())
}"

$cmd_idx = $content.IndexOf('async fn cmd_send(')
if ($cmd_idx -lt 0) { throw 'cmd_send not found' }

$start_idx = $content.IndexOf($start_marker, $cmd_idx)
if ($start_idx -lt 0) { throw 'start marker not found' }

$end_idx = $content.IndexOf($end_marker, $start_idx)
if ($end_idx -lt 0) { 
    $end_marker = "Ok(())
}"
    $end_idx = $content.IndexOf($end_marker, $start_idx)
    if ($end_idx -lt 0) { throw 'end marker not found' }
}

$end_idx += $end_marker.Length

$new_content = $content.Substring(0, $start_idx) + $patch + $content.Substring($end_idx)
Set-Content -Path 'desktop/p2ptransfer-cli/src/main.rs' -Value $new_content -NoNewline
Write-Host 'Patched successfully'