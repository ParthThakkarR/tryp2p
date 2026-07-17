param(
    [string]$Url,
    [int]$Concurrency = 1
)

$Size = 16 * 1024 * 1024
$Jobs = @()

$StartTime = [DateTime]::UtcNow

for ($i = 0; $i -lt $Concurrency; $i++) {
    $StartByte = $i * $Size
    $EndByte = ($i + 1) * $Size - 1
    
    $ScriptBlock = {
        param($U, $S, $E)
        $env:PATH = "C:\msys64\msys64\mingw64\bin;" + $env:PATH
        $out = curl.exe -f -s -r "$S-$E" "$U" -o NUL
        if ($LASTEXITCODE -ne 0) { throw "Curl failed with exit code $LASTEXITCODE" }
    }
    
    $Jobs += Start-Job -ScriptBlock $ScriptBlock -ArgumentList $Url, $StartByte, $EndByte
}

Wait-Job $Jobs | Out-Null
Receive-Job $Jobs | Out-Null

$EndTime = [DateTime]::UtcNow
$Elapsed = ($EndTime - $StartTime).TotalSeconds

$TotalMB = ($Concurrency * 16)
$Mbps = ($TotalMB * 8) / $Elapsed

Write-Output "Downloaded $TotalMB MB in $($Elapsed.ToString('F2'))s"
Write-Output "Throughput: $($Mbps.ToString('F2')) Mbps"
