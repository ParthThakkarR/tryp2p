for ($session = 1; $session -le 3; $session++) {
    Write-Output "====== STARTING SESSION $session ======"
    
    # Ensure no old processes are lingering
    taskkill /F /IM p2p.exe /T 2>$null
    taskkill /F /IM copyparty.exe /T 2>$null
    taskkill /F /IM cloudflared.exe /T 2>$null
    
    # Start sender in background using Start-Process
    Remove-Item -Force "d:\tryp2p\sender_out.log" -ErrorAction SilentlyContinue
    $Env:PATH = "C:\msys64\msys64\mingw64\bin;" + $Env:PATH
    $SenderProc = Start-Process -FilePath "d:\tryp2p\target\release\p2p.exe" -ArgumentList "send test_200m_rnd.bin --wan" -RedirectStandardOutput "d:\tryp2p\sender_out.log" -RedirectStandardError "d:\tryp2p\sender_err.log" -WindowStyle Hidden -PassThru
    
    # Wait for the tunnel URL to appear in the output file
    $Url = $null
    for ($i = 0; $i -lt 30; $i++) {
        Start-Sleep -Seconds 2
        if (Test-Path "d:\tryp2p\sender_out.log") {
            $Output = Get-Content "d:\tryp2p\sender_out.log"
            foreach ($line in $Output) {
                if ($line -match "https://[a-zA-Z0-9.-]+\.trycloudflare\.com/[a-zA-Z0-9]+") {
                    $Url = $matches[0]
                    break
                }
            }
        }
        if ($Url) { break }
    }
    
    if ($null -eq $Url) {
        Write-Output "Failed to get URL for session $session"
        Stop-Process -Id $SenderProc.Id -Force
        continue
    }
    
    Write-Output "Session $session URL: $Url"
    
    # Wait for tunnel to stabilize by polling
    Write-Output "Waiting for tunnel to stabilize..."
    $TunnelReady = $false
    for ($i = 0; $i -lt 60; $i++) {
        $env:PATH = "C:\msys64\msys64\mingw64\bin;" + $env:PATH
        $out = curl.exe -I -f --connect-timeout 5 "$Url/test_200m_rnd.bin"
        Write-Output "Poll attempt $i, Exit code: $LASTEXITCODE"
        if ($LASTEXITCODE -eq 0) {
            $TunnelReady = $true
            break
        }
        Start-Sleep -Seconds 2
    }
    
    if (-not $TunnelReady) {
        Stop-Process -Id $SenderProc.Id -Force
        continue
    }
    Write-Output "Tunnel is ready!"
    
    foreach ($c in 1,2,4,8) {
        Write-Output "=== Session ${session}: curl C=${c} ==="
        .\test_wan.ps1 -Url "$Url/test_200m_rnd.bin" -Concurrency $c
    }
    
    Write-Output "=== Session ${session}: App C=8 ==="
    $env:PATH = "C:\msys64\msys64\mingw64\bin;" + $env:PATH
    $AppOutput = d:\tryp2p\target\release\p2p.exe receive $Url --output "test_output_s$session" --concurrency 8 2>&1
    
    $AppThroughput = $null
    foreach ($line in $AppOutput) {
        if ($line -match "Throughput: ([0-9.]+) Mbps") {
            $AppThroughput = $matches[1]
        }
    }
    if ($AppThroughput) {
        Write-Output "App C=8 Throughput: $AppThroughput Mbps"
    } else {
        Write-Output $AppOutput
        Write-Output "Failed to get App throughput"
    }
    
    # Kill the sender process
    Stop-Process -Id $SenderProc.Id -Force
    taskkill /F /IM p2p.exe /T 2>$null
    taskkill /F /IM copyparty.exe /T 2>$null
    taskkill /F /IM cloudflared.exe /T 2>$null
}
