# Checks the module without Pester or a running app: it loads, exports its
# commands, and passes the right arguments to unidle. Run by CI on every OS:
#   pwsh -NoProfile -File integrations/powershell/Unidle.Tests.ps1
$ErrorActionPreference = 'Stop'
$module = Join-Path $PSScriptRoot 'Unidle'

$manifest = Test-ModuleManifest (Join-Path $module 'Unidle.psd1')
Import-Module $module -Force
$expected = 'Disable-Unidle', 'Enable-Unidle', 'Get-UnidlePath', 'Get-UnidleStatus', 'Start-UnidleTimer'
$exported = (Get-Command -Module Unidle).Name | Sort-Object
if (Compare-Object $expected $exported) { throw "Exported: $($exported -join ', ')" }
if ($manifest.Version -ne [version]'0.2.0') { throw "Version: $($manifest.Version)" }

if ($IsWindows) {
    Write-Output 'Unidle module: loads (argument checks run on Linux and macOS)'
    exit 0
}

# A stand-in for unidle that prints its arguments as the status JSON.
$fake = Join-Path ([IO.Path]::GetTempPath()) "unidle-fake-$PID"
Set-Content -LiteralPath $fake -Value "#!/bin/sh`nprintf '{""args"":""%s""}\n' ""`$*"""
chmod +x $fake
$env:UNIDLE_PATH = $fake
try {
    $cases = @(
        @{ Got = (Enable-Unidle).args; Want = 'on --json' }
        @{ Got = (Enable-Unidle -Nudge).args; Want = 'on --json --nudge' }
        @{ Got = (Disable-Unidle).args; Want = 'off --json' }
        @{ Got = (Get-UnidleStatus).args; Want = 'status --json' }
        @{ Got = (Start-UnidleTimer -Duration (New-TimeSpan -Minutes 90)).args; Want = 'timer 90 --json' }
        @{ Got = (Start-UnidleTimer -Until '18:00' -Nudge).args; Want = 'timer 18:00 --json --nudge' }
    )
    foreach ($case in $cases) {
        if ($case.Got -ne $case.Want) { throw "Got '$($case.Got)', wanted '$($case.Want)'" }
    }
} finally {
    Remove-Item -LiteralPath $fake
}
Write-Output "Unidle module: $($cases.Count) argument checks pass"
