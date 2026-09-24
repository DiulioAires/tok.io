$ErrorActionPreference = 'Stop'
$artifacts = Join-Path $PSScriptRoot 'artifacts'
$certificate = Join-Path $artifacts 'AIUsageLocal.cer'
$package = Join-Path $artifacts 'AIUsage.LockScreen.msix'
$status = Join-Path $artifacts 'install-status.txt'

try {
    Import-Certificate -FilePath $certificate -CertStoreLocation 'Cert:\LocalMachine\TrustedPeople' | Out-Null
    Add-AppxPackage -Path $package -ErrorAction Stop
    Set-Content -LiteralPath $status -Value 'installed' -Encoding utf8
} catch {
    Set-Content -LiteralPath $status -Value $_.ToString() -Encoding utf8
    exit 1
}
