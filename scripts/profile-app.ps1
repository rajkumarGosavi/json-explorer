# Samples CPU + memory of the running json-explorer app (Rust main process
# and its WebView2 children) at a fixed interval and writes a CSV you can chart.
#
# Run the app first (however you normally launch it), then:
#   powershell -File scripts/profile-app.ps1 -Seconds 120 -IntervalMs 1000
#
# Columns: timestamp, process name, PID, CPU seconds (cumulative), working set
# MB (physical RAM), private MB. Delta CPU seconds between rows / interval =
# fraction of one core used in that window.

param(
  [int]$Seconds = 60,
  [int]$IntervalMs = 1000,
  [string]$OutFile = "app-profile.csv",
  # Match the Rust host process and the WebView2 renderer processes.
  [string[]]$Names = @("json-explorer", "msedgewebview2")
)

$deadline = (Get-Date).AddSeconds($Seconds)
"timestamp,name,pid,cpu_sec,ws_mb,priv_mb" | Out-File -FilePath $OutFile -Encoding utf8

Write-Host "Sampling $($Names -join ', ') every ${IntervalMs}ms for ${Seconds}s -> $OutFile"
Write-Host "(start the app and open/search a big file while this runs)"

while ((Get-Date) -lt $deadline) {
  $ts = (Get-Date).ToString("o")
  foreach ($n in $Names) {
    try { $procs = Get-Process -Name $n -ErrorAction Stop } catch { continue }
    foreach ($p in $procs) {
      $cpu   = if ($p.CPU) { [math]::Round($p.CPU, 2) } else { 0 }
      $wsMb  = [math]::Round($p.WorkingSet64 / 1MB, 1)
      $prvMb = [math]::Round($p.PrivateMemorySize64 / 1MB, 1)
      "$ts,$($p.ProcessName),$($p.Id),$cpu,$wsMb,$prvMb" |
        Out-File -FilePath $OutFile -Append -Encoding utf8
    }
  }
  Start-Sleep -Milliseconds $IntervalMs
}

Write-Host "done -> $OutFile. Peak RAM:"
Import-Csv $OutFile | Group-Object name | ForEach-Object {
  $peak = ($_.Group | Measure-Object ws_mb -Maximum).Maximum
  Write-Host ("  {0}: {1} MB peak working set" -f $_.Name, $peak)
}
