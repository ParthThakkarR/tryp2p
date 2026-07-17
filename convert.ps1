$bytes = [System.IO.File]::ReadAllBytes('desktop/p2ptransfer-cli/src/main.rs')
$str = [System.Text.Encoding]::Unicode.GetString($bytes)
$utf8Bytes = [System.Text.Encoding]::UTF8.GetBytes($str)
[System.IO.File]::WriteAllBytes('desktop/p2ptransfer-cli/src/main.rs', $utf8Bytes)