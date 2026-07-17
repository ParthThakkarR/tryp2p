$bytes = [System.IO.File]::ReadAllBytes('desktop/p2ptransfer-cli/src/main.rs')
$start = 0
while ($start -lt $bytes.Length -and $bytes[$start] -gt 127) {
    $start++
}
if ($start -gt 0) {
    $newBytes = new-object byte[] ($bytes.Length - $start)
    [System.Array]::Copy($bytes, $start, $newBytes, 0, $newBytes.Length)
    [System.IO.File]::WriteAllBytes('desktop/p2ptransfer-cli/src/main.rs', $newBytes)
}