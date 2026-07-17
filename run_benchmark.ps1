$ErrorActionPreference = "Stop"
$env:PATH = "C:\msys64\msys64\mingw64\bin;" + $env:PATH
New-Item -ItemType Directory -Force -Path "d:\tryp2p\scratch\baseline_recv" | Out-Null
New-Item -ItemType Directory -Force -Path "d:\tryp2p\scratch\step2_recv" | Out-Null

if (-Not (Test-Path "test_512m.bin")) {
    echo "Creating 512MB test file..."
    fsutil file createnew test_512m.bin 536870912
}

echo "Starting receiver..."
$recvProc = Start-Process -NoNewWindow -FilePath "target\release\p2p.exe" -ArgumentList "listen", "--port", "9877", "--output", "d:\tryp2p\scratch\baseline_recv" -RedirectStandardError "d:\tryp2p\scratch\baseline_recv.err" -RedirectStandardOutput "d:\tryp2p\scratch\baseline_recv.out" -PassThru
Start-Sleep -Seconds 2

echo "Starting sender..."
$sw = [System.Diagnostics.Stopwatch]::StartNew()
cmd.exe /c "target\release\p2p.exe send test_512m.bin 127.0.0.1:9877 2> d:\tryp2p\scratch\baseline_send.err > d:\tryp2p\scratch\baseline_send.out"
$sw.Stop()
$throughput = 512 / $sw.Elapsed.TotalSeconds
echo "Transfer took $($sw.Elapsed.TotalSeconds) seconds ($throughput MB/s)"

echo "Stopping receiver..."
Stop-Process -Id $recvProc.Id -Force
