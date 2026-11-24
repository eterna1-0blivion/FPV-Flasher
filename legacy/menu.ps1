# author: eterna1_0blivion
$version = 'v0.2.1-dev'

# Планируемые направления:
# - перевести приложение на Rust (см. docs/ROADMAP.md), legacy-скрипты остаются запасным путём;
# - доработать способы прошивки полётника - убрать лишнее, добавить функции для авто-бекапа и авто-перехода в режим DFU;
# - полная поддержка ESPtool (прошивка способами ExpressLRS Configurator - через Полётник, через Wi-Fi, через USB)
# - реализовать функцию прошивки многих компонентов сразу (например, прошивка полётника + RX в одном действии)
# - реализовать функции Caddx PC Tool для прошивки Walksnail систем прямо через терминал (перехватить команды и прошивки)

# Принудительно заставляем любую версию PowerShell работать в UTF-8
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
[Console]::InputEncoding = [System.Text.Encoding]::UTF8

# Подгружаем сборку графических диалогов WPF для совместимости с PowerShell 5.0
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

function Show-RadioModuleStub {
    Show-RadioModuleHeader
    Show-Separator
    Show-Message -Message @"
    Этот раздел выведен из эксплуатации вместе с прежней реализацией на PowerShell.

    Прошивка модулей управления (ESP32: ExpressLRS, ZLRS и совместимые)
    переписывается заново на Rust: сначала разбор ExpressLRS Configurator
    и образцов прошивок, затем собственный сценарий прошивки.

    Старый код сохранён в analyze\RadioModule\legacy-tools\powershell-legacy
    и в истории Git прежнего репозитория.
"@ -Color "White"

    Show-Exit
}

function Show-MainMenu {
    Show-MainHeader
    Show-Separator
    Show-Message -Message @"
    1. Работа с полётным контроллером (STM32)
    2. Работа с модулем управления (ESPtool)

    0. Выход из программы
"@ -Color "White"

    Show-Message -Message "Выбери действие [0-2]: " -Input Choice -NewLine
}

while ($true) {
    $choice = Show-MainMenu
    if ($null -eq $choice) {
        return
    }

    switch ($choice) {
        "1" { Show-FlightControllerMenu -Config $appConfig }
        "2" { Show-RadioModuleStub }
        "0" { return }
        default { Show-WrongInput -Menu "Main" }
    }
}
