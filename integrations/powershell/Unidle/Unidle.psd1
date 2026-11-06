@{
    RootModule        = 'Unidle.psm1'
    ModuleVersion     = '0.2.0'
    GUID              = '6f3d2b1e-8c4a-4b7e-9a51-2e0c7d9f4a63'
    Author            = 'Unidle contributors'
    Description       = 'Drive the Unidle tray app from PowerShell: keep the computer awake, let it sleep, timers and status.'
    PowerShellVersion = '5.1'
    FunctionsToExport = @('Get-UnidlePath', 'Get-UnidleStatus', 'Enable-Unidle', 'Disable-Unidle', 'Start-UnidleTimer')
    CmdletsToExport   = @()
    VariablesToExport = @()
    AliasesToExport   = @()
    PrivateData       = @{
        PSData = @{
            LicenseUri = 'https://github.com/ThroughTheWind/unidle/blob/main/LICENSE'
            ProjectUri = 'https://github.com/ThroughTheWind/unidle'
        }
    }
}
