$bytes = [System.IO.File]::ReadAllBytes('desktop/p2ptransfer-cli/src/main.rs')
if ($bytes[0] -eq 239 -and $bytes[1] -eq 187 -and $bytes[2] -eq 191) {
    $newBytes = new-object byte[] ($bytes.Length - 3)
    [System.Array]::Copy($bytes, 3, $newBytes, 0, $newBytes.Length)
    [System.IO.File]::WriteAllBytes('desktop/p2ptransfer-cli/src/main.rs', $newBytes)
}