# Unidle from PowerShell (SPEC §13): thin wrappers around `unidle.exe`, which
# drives the Unidle tray app. Power only unless -Nudge is given.

function Get-UnidlePath {
    <#
    .SYNOPSIS
    The unidle.exe these commands run: $env:UNIDLE_PATH, then PATH, then the
    install folders the MSI and the setup program use.
    #>
    [CmdletBinding()]
    param()
    $candidates = @(
        $env:UNIDLE_PATH
        (Get-Command unidle.exe -CommandType Application -ErrorAction SilentlyContinue |
            Select-Object -First 1 -ExpandProperty Source)
        # Unset outside Windows, where Join-Path would refuse them.
        $(if ($env:LOCALAPPDATA) { Join-Path $env:LOCALAPPDATA 'Unidle\unidle.exe' })
        $(if ($env:ProgramFiles) { Join-Path $env:ProgramFiles 'Unidle\unidle.exe' })
    )
    foreach ($candidate in $candidates) {
        if ($candidate -and (Test-Path -LiteralPath $candidate -PathType Leaf)) {
            return $candidate
        }
    }
    throw 'unidle.exe not found. Install Unidle, or set $env:UNIDLE_PATH to unidle.exe.'
}

function Invoke-Unidle {
    param([Parameter(Mandatory)] [string[]] $Arguments)
    $exe = Get-UnidlePath
    # Capturing the output also makes PowerShell wait for the command.
    $output = & $exe @Arguments 2>&1
    if ($LASTEXITCODE -ne 0) {
        throw (($output | Out-String).Trim())
    }
    $output
}

function Get-UnidleStatus {
    <#
    .SYNOPSIS
    Whether Unidle keeps this computer awake, why, and what happens next.
    .EXAMPLE
    (Get-UnidleStatus).on
    #>
    [CmdletBinding()]
    param()
    (Invoke-Unidle -Arguments @('status', '--json') | Out-String) | ConvertFrom-Json
}

function Enable-Unidle {
    <#
    .SYNOPSIS
    Keep this computer awake until Disable-Unidle. Starts Unidle in the tray
    if it isn't running.
    .PARAMETER Nudge
    Also send the input nudges from Unidle's Settings. Without it, Unidle only
    asks Windows not to sleep.
    #>
    [CmdletBinding(SupportsShouldProcess)]
    param([switch] $Nudge)
    $arguments = @('on', '--json')
    if ($Nudge) { $arguments += '--nudge' }
    if ($PSCmdlet.ShouldProcess('this computer', 'Keep awake')) {
        (Invoke-Unidle -Arguments $arguments | Out-String) | ConvertFrom-Json
    }
}

function Disable-Unidle {
    <#
    .SYNOPSIS
    Let this computer sleep again.
    #>
    [CmdletBinding(SupportsShouldProcess)]
    param()
    if ($PSCmdlet.ShouldProcess('this computer', 'Let it sleep')) {
        (Invoke-Unidle -Arguments @('off', '--json') | Out-String) | ConvertFrom-Json
    }
}

function Start-UnidleTimer {
    <#
    .SYNOPSIS
    Keep this computer awake for a while, or until a time.
    .EXAMPLE
    Start-UnidleTimer -Duration (New-TimeSpan -Minutes 90)
    .EXAMPLE
    Start-UnidleTimer -Until '18:00'
    #>
    [CmdletBinding(SupportsShouldProcess, DefaultParameterSetName = 'Duration')]
    param(
        [Parameter(Mandatory, ParameterSetName = 'Duration')] [TimeSpan] $Duration,
        [Parameter(Mandatory, ParameterSetName = 'Until')] [ValidatePattern('^\d{1,2}:\d{2}$')] [string] $Until,
        [switch] $Nudge
    )
    $length = if ($PSCmdlet.ParameterSetName -eq 'Until') {
        $Until
    } else {
        [string][int][Math]::Ceiling($Duration.TotalMinutes)
    }
    $arguments = @('timer', $length, '--json')
    if ($Nudge) { $arguments += '--nudge' }
    if ($PSCmdlet.ShouldProcess('this computer', "Keep awake ($length)")) {
        (Invoke-Unidle -Arguments $arguments | Out-String) | ConvertFrom-Json
    }
}

Export-ModuleMember -Function Get-UnidlePath, Get-UnidleStatus, Enable-Unidle, Disable-Unidle, Start-UnidleTimer
