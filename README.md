# FPV-Flasher

Консольная утилита для прошивки FPV-оборудования с Windows 10/11: полётные контроллеры (STM32),
модули управления (ESP32, ExpressLRS) и системы Walksnail (Caddx Ascent).

Проект распространяется архивом «из коробки»: внутри собранный exe и все сторонние инструменты.
Отдельно устанавливать PowerShell, Python, STM32CubeProgrammer или драйверы не требуется.

## Состояние

| Раздел | Состояние |
|---|---|
| 1. Полётные контроллеры (STM32) | работает на PowerShell (legacy), перенос на Rust запланирован |
| 2. Модули управления (ESP32) | заглушка: реализация пишется заново на Rust |
| 3. Walksnail (Caddx) | в разработке: разбор протокола AR и `.img` |

Идёт переписывание на Rust. Пока нового бинарника нет, рабочим остаётся `legacy\run.cmd`.

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
