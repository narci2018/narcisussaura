# Fails the build unless every shipped core engine is an AMD64 PE image.
# Two real escapes this gate would have caught: v0.2.116 shipped an i386 (machine 0x014C)
# psiphon-tunnel-core.exe that died at spawn with os error 216, and the cross-platform job
# checked out with lfs disabled, so psiphon/aether/wintun were packaged as 133-byte Git LFS
# pointer text files instead of engines.
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

function Test-GitLfsPointer([string]$path) {
  if (-not (Test-Path $path)) { return $false }
  $first = Get-Content -LiteralPath $path -TotalCount 1 -ErrorAction SilentlyContinue
  return ($first -like "version https://git-lfs.github.com*")
}

$failed = $false
foreach ($file in $Files) {
  $machine = Get-PeMachine $file
  if ($null -eq $machine) {
    if (-not (Test-Path $file)) {
      Write-Host "ERROR: $file is missing from the checkout"
    } elseif (Test-GitLfsPointer $file) {
      Write-Host "ERROR: $file is a Git LFS pointer that was never pulled - the checkout must run with lfs: true"
    } else {
      Write-Host "ERROR: $file is not a PE image"
    }
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
