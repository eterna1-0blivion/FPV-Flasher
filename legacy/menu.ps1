# Автор: eterna1_0blivion
$version = 'v0.2.1-dev'

# UTF-8 для ввода и вывода консоли. Кодировку самого скрипта это не меняет.
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
[Console]::InputEncoding = [System.Text.Encoding]::UTF8

# Подключаем WPF для диалогов выбора и сохранения файлов.
Add-Type -AssemblyName PresentationFramework

# Устанавливаем заголовок консоли, меняем задний фон
$Host.UI.RawUI.WindowTitle = "FPV-Flasher ($version)"
$Host.UI.RawUI.BackgroundColor = "Black"
Clear-Host
 
# Все пути ресурсов вычисляются от каталога приложения, независимо от текущего каталога PowerShell.
if ($MyInvocation.MyCommand.CommandType -eq "ExternalScript") {
    $scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Definition
}
else {
    $scriptDir = [System.IO.Path]::GetDirectoryName([System.Diagnostics.Process]::GetCurrentProcess().MainModule.FileName)
}

# Сторонние инструменты лежат в каталоге vendor проекта, а не рядом со скриптами.
$vendorDirectory = [System.IO.Path]::GetFullPath((Join-Path $scriptDir "..\vendor"))

# Передаём зависимости явно в рабочие функции, а не полагаемся на переменные из области видимости скрипта.
$appConfig = [pscustomobject]@{
    RootDirectory       = $scriptDir
    STM32ProgrammerPath = Join-Path $vendorDirectory "STM32CubeCLT\bin\STM32_Programmer_CLI.exe"
    DriverFixerPath     = Join-Path $vendorDirectory "ImpulseRC_Driver_Fixer.exe"
}

. (Join-Path $scriptDir "src\common\ui.ps1")
. (Join-Path $scriptDir "src\flightController\devices.ps1")
. (Join-Path $scriptDir "src\flightController\firmware.ps1")
. (Join-Path $scriptDir "src\flightController\menu.ps1")

# Главное меню находится в Rust-приложении: скрипт открывает сразу раздел STM32.
Show-FlightControllerMenu -Config $appConfig
