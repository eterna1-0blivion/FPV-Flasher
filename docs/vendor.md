# Каталог vendor/

Сторонние инструменты, без которых приложение не работает «из коробки». В git не входят:
восстанавливаются из архива с исходниками или с официальных сайтов. Контрольные суммы —
`../vendor.sha256` (формат `sha256sum`: хэш, два пробела, путь с прямыми слэшами).

## Состав

| Путь | Назначение | Размер |
|---|---|---|
| `STM32CubeCLT/bin/` | `STM32_Programmer_CLI.exe` и его DLL | ~40 МБ |
| `STM32CubeCLT/Data_Base/` | описания чипов STM32 для CLI | |
| `CP210x_Universal_Windows_Driver/` | драйвер USB-UART CP210x (ESP-модули) | ~0,6 МБ |
| `CH341SER.EXE` | установщик драйвера CH340/CH341 | ~0,8 МБ |
| `ImpulseRC_Driver_Fixer.exe` | установка WinUSB для STM32 DFU | ~5,8 МБ |

Текущий манифест рабочего комплекта содержит 130 файлов (около 47,7 МиБ).
Локальный `vendor/Zadig-2.9.exe` добавлен как материал для анализа: он не нужен программе,
не входит в манифест и не должен автоматически включаться в пользовательский архив.
При сборке архива нужно выбирать файлы по манифесту, а не копировать весь `vendor/`.

## Проверка целостности

PowerShell:

```powershell
$root = (Resolve-Path .\vendor).Path
Get-Content .\vendor.sha256 | ForEach-Object {
    $hash, $path = $_ -split '  ', 2
    $actual = (Get-FileHash -LiteralPath (Join-Path $root $path) -Algorithm SHA256).Hash.ToLower()
    if ($actual -ne $hash) { "НЕ СОВПАДАЕТ: $path" }
}
```

Если команда ничего не вывела, все файлы совпадают с манифестом.

## Восстановление

1. Скачать STM32CubeCLT нужной версии с сайта ST, скопировать `bin/` и `Data_Base/` в
   `vendor/STM32CubeCLT/`.
2. Драйверы CP210x и CH341SER взять с сайтов Silicon Labs и WCH.
3. ImpulseRC Driver Fixer — с сайта ImpulseRC.
4. Пересчитать манифест:

```powershell
$root = (Resolve-Path .\vendor).Path
Get-ChildItem -Recurse -File $root | Sort-Object FullName | ForEach-Object {
    '{0}  {1}' -f (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToLower(),
        ($_.FullName.Substring($root.Length + 1).Replace('\', '/'))
} | Set-Content .\vendor.sha256 -Encoding ASCII
```

После обновления сторонних файлов манифест коммитится вместе с изменением состава.

## Что планируется убрать

- STM32CubeCLT — после появления собственного DFU-клиента на Rust (этап 5).
- ImpulseRC Driver Fixer — после собственной установки WinUSB (этап 5).
- Драйверы CP210x и CH341 останутся: это единственная часть, которую нельзя заменить кодом.
