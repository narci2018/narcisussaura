# Fails the build unless every shipped core engine is an AMD64 PE image.
# A 32-bit (i386, machine 0x014C) psiphon-tunnel-core.exe passed the "file exists and is
# big enough" check, shipped in v0.2.116, and then died at spawn time on the user's Win11
# box with ERROR_EXE_MACHINE_TYPE_MISMATCH (os error 216).
param(
  [string[]]$Files = @(
    "src-tauri/binaries/sing-box.exe",
    "src-tauri/binaries/mihomo.exe",
    "src-tauri/binaries/psiphon-tunnel-core.exe",
    "src-tauri/binaries/aether.exe",
    "src-tauri/binaries/wintun.dll"
  ),
  [int]$ExpectedMachine = 0x8664
)

function Get-PeMachine([string]$path) {
  if (-not (Test-Path $path)) { return $null }
  $fs = [IO.File]::OpenRead($path)
  try {
    $b = New-Object byte[] 4096
    $null = $fs.Read($b, 0, 4096)
    if ($b[0] -ne 0x4D -or $b[1] -ne 0x5A) { return $null }
    $pe = [BitConverter]::ToInt32($b, 0x3C)
    return [int][BitConverter]::ToUInt16($b, $pe + 4)
  } finally { $fs.Dispose() }
}

function Get-PeMachineName([int]$machine) {
  switch ($machine) {
    0x8664 { "AMD64" }
    0x014C { "i386 (32-bit)" }
    0xAA64 { "ARM64" }
    default { "unknown" }
  }
}

$failed = $false
foreach ($file in $Files) {
  $machine = Get-PeMachine $file
  if ($null -eq $machine) {
    Write-Host "ERROR: $file is missing or is not a PE image (an un-pulled Git LFS pointer reads as text)"
    $failed = $true
  } elseif ($machine -ne $ExpectedMachine) {
    Write-Host ("ERROR: {0} is 0x{1:X4} ({2}); the installer ships {3} engines only" -f `
        $file, $machine, (Get-PeMachineName $machine), (Get-PeMachineName $ExpectedMachine))
    $failed = $true
  } else {
    Write-Host ("OK: {0} is 0x{1:X4} ({2})" -f $file, $machine, (Get-PeMachineName $machine))
  }
}

if ($failed) {
  Write-Host "FATAL: core engine architecture mismatch - aborting before packaging."
  exit 1
}
Write-Host "SUCCESS: all core engines are $(Get-PeMachineName $ExpectedMachine)."
