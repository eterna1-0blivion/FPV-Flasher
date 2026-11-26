# FPV-Flasher

Консольная утилита для прошивки FPV-оборудования с Windows 10/11: полётные контроллеры (STM32),
модули управления (ESP32, ExpressLRS) и системы Walksnail (Caddx Ascent).

Цель — Windows-архив с готовым exe и необходимыми инструментами, без установки Rust или Python
на пользовательской машине. В текущей версии раздел STM32 использует Windows PowerShell
(либо PowerShell 7, если он установлен) и может требовать отдельной установки USB-драйвера.

## Состояние

| Раздел | Состояние |
|---|---|
| 1. Полётные контроллеры (STM32) | работает на PowerShell (legacy), перенос на Rust запланирован |
| 2. Модули управления (ESP32) | заглушка: реализация пишется заново на Rust |
| 3. Walksnail (Caddx) | в разработке: разбор протокола AR и `.img` |

Главное меню уже запускается на Rust. Пункт 1 открывает существующее меню STM32,
пункты 2 и 3 пока ничего не прошивают. `legacy\run.cmd` можно запускать и напрямую.

Из корня проекта с установленным Rust:

```powershell
cargo run --offline
cargo test --workspace --offline
cargo run --release --offline -p xtask
```

Последняя команда собирает архив `dist\FPV-Flasher-<версия>.zip`: проверяет `vendor.sha256`,
кладёт exe, `legacy\`, `docs\` и файлы из манифеста `vendor/`. Лишние файлы в `vendor/`
(например `Zadig-2.9.exe`) в архив не попадают.

Диагностика без меню: `fpv-flasher.exe --ports` показывает COM-порты с VID:PID и распознанным
типом устройства, а также DFU-загрузчики (STM32 `0483:DF11`, AT32 `2E3C:DF11`); порты не
открываются. Журнал сессии пишется рядом с exe в `fpv-flasher.log`; если каталог только для
чтения, программа продолжает работу.

## Протокол Walksnail

Крейт `crates/walksnail` содержит проверенную на дампах часть протокола: кадр AR, CRC32,
разбор ответа `FIND_DEVICE`, заголовок `.img`, план обновления и `ArFileInfo`. Устройства он
не открывает: запись в порт появится отдельно. Описание протокола — [docs/protocols/walksnail-ar.md](docs/protocols/walksnail-ar.md).

```powershell
cargo test -p fpv-walksnail --offline
```

Меню очищает экран и использует цвета только при выводе в настоящий терминал. В перенаправленном
выводе управляющих последовательностей нет, поэтому логи и пайпы остаются читаемыми.

## Структура

```
Cargo.toml        рабочее пространство Rust (появится на этапе 2)
crates/           app, core, walksnail, radio, fc
legacy/           рабочий код на PowerShell: меню полётников, тесты Pester
docs/             ROADMAP.md, vendor.md, описания протоколов
vendor/           сторонние инструменты (не в git, см. docs/vendor.md)
analyze/          материалы для анализа (не в git)
tools/            портативные анализаторы (не в git)
vendor.sha256     контрольные суммы файлов vendor/
```

## Запуск legacy-версии

```
legacy\run.cmd
```

Скрипт сам выбирает `pwsh` 7, если он установлен, иначе `powershell.exe` 5.1.
Требуется каталог `vendor/` рядом с `legacy/`.

## Тесты

```
Invoke-Pester -Script .\legacy\tests\FlightController.Tests.ps1
```

Тесты не требуют подключённого оборудования. Pester 3.4+.

## Сторонние инструменты

Каталог `vendor/` в репозиторий не входит. Состав, назначение и контрольные суммы описаны в
[docs/vendor.md](docs/vendor.md) и [vendor.sha256](vendor.sha256). Список лицензий —
[THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md).

## Сборка релиза

Появится вместе с Rust-версией (этап 2): `cargo xtask dist` собирает exe, добавляет `vendor/`
по манифесту и упаковывает в `dist\FPV-Flasher-<версия>.zip`.

## Лицензия

MIT, см. [LICENSE](LICENSE).
