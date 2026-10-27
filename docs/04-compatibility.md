# Стратегия совместимости

## 1. Разделить несовпадающие задачи

Совместимость исходного кода, бинарного ABI, поведения API, форматов файлов, драйверов и сетевых служб — разные свойства. Компиляция Rust-программы для Windows не доказывает работу Microsoft Office. Заголовок PE не доказывает даже успешную инициализацию процесса.

Wine предоставляет загрузчик и реализацию Windows API поверх Unix; WinCraft управляет его поставкой, профилями и интеграцией. Собственный Rust launcher не должен повторно реализовывать его DLL. Основа: [Wine README и исходники](https://github.com/wine-mirror/wine).

## 2. Матрица направлений

Все статусы ниже — **план поддержки**, кроме ограниченного существующего Win32 demo. Ни одна перечисленная программа или категория не считается уже сертифицированной.

| Категория | Механизм | Приоритет | Условия/ограничения |
| --- | --- | --- | --- |
| Нативные Unix CLI/GUI | ELF и согласованные библиотеки | P1 | CPU/libc/dependencies |
| Windows x64 desktop | Wine | P1 | Фиксированные версии, полный сценарий |
| Windows x86 desktop | Проверенный WoW64/runtime | P1 | Отдельно проверить build и зависимости |
| MSI/EXE installers | Wine installer services | P1 | Install/update/uninstall, не только запуск setup |
| Win32 console | Wine console integration | P1 | stdin/out, Unicode, pipes, exit status |
| .NET Framework desktop | Wine + допустимый runtime | P2 | WinForms/WPF/COM тестируются отдельно |
| Современный .NET | Native Linux где возможно; Windows build через Wine | P2 | Windows-specific UI/API не становятся переносимыми автоматически |
| DirectX игры | Wine/Proton + graphics runtime | P2 | GPU, Vulkan, launcher, controller, audio |
| DOS COM/MZ | DOSBox runtime | P2 | CPU emulation, DOS API, память, звук |
| Windows 3.x/Win16 | Отдельное legacy окружение | P3 | Не включать в гарантию x64 Wine |
| Windows services/COM/RPC | Реализации runtime | P2/P3 | Зависимости и взаимодействия по приложениям |
| UWP/MSIX/Store | Исследование по подсистемам | R&D | Packaging не равно всей платформе |
| NT kernel drivers | Нет общего механизма в Linux-ветке | R&D | Нужны NT kernel ABI и модель драйверов |
| Античиты/DRM | Только поддержанные издателем сценарии | Отдельно | Не обещать и не подменять аттестацию |
| Windows VM | Отдельный VM backend | P3 | Гостевая Windows, лицензия, ресурсы; не native pass |
| Microsoft cloud/account | Документированные протоколы/официальные клиенты | P3 | Доступность и политика поставщика |

## 3. Win32 и NT user mode

Покрытие группировать по реальным потребителям:

- Process/thread: creation, handles, waits, synchronization, job objects, TLS.
- Memory/exceptions: virtual memory, mappings, SEH, unwinding и взаимодействие JIT.
- Files: locking/share modes, overlapped I/O, IOCP, notifications, paths.
- UI: USER/GDI, fonts, DPI, input, clipboard, accessibility и печать.
- Registry/services: per-prefix registry, service lifetime, installer transactions.
- COM/OLE/RPC: apartments, marshaling, registrations, out-of-process server.
- Network/crypto: Winsock, TLS, certificates, proxy, authentication.
- Media: codecs, audio sessions, video playback/capture, Media Foundation paths.

Не создавать собственный «ntdll-lite» и не объявлять поддержку NT по наличию нескольких экспортов. При дефекте: минимальный reproducer → определить upstream/runtime/integration проблему → regression test → upstream patch, если применимо → зафиксированный backport.

Windows NT syscalls и Linux syscalls не имеют взаимно однозначного отображения. Объекты, security tokens, APC, wait semantics и IRP требуют поведения системы, а не только таблицы номеров. Прямая загрузка `.sys` в Linux не является расширением EXE-launcher. Microsoft отдельно описывает различие [user mode и kernel mode](https://learn.microsoft.com/en-us/windows-hardware/drivers/gettingstarted/user-mode-and-kernel-mode).

## 4. x86 и x64

Начальная ОС — x86-64. Для Windows x86 выбрать и закрепить конкретный вариант Wine/WoW64, проверить 32-битные installers, COM servers, child processes и графические компоненты. Наличие слова WoW64 в версии не заменяет теста готового образа. В manifest записывать архитектуры Wine, PE-модулей и host-библиотек. 16-битное приложение относится к legacy backend.

ARM64 требует другого загрузчика/ABI и, для x86-программ, дополнительной трансляции CPU. Его следует планировать после стабильной x86-64 базы.

## 5. DOS и историческое ПО

DOS subsystem отделяется от Windows NT. Использовать сопровождаемый DOSBox runtime; [DOSBox Staging](https://www.dosbox-staging.org/) — кандидат для исследования, особенно интерактивных DOS-программ. Покрытие деловых DOS-приложений, DPMI/extenders и специфических устройств проверять отдельно, не выводить из игровой совместимости.

Профиль задаёт cycles, память, VGA/SVGA, Sound Blaster/MIDI, раскладку, code page, mount directories и print/export policy. Поддержка MZ требует различать DOS executable и DOS stub PE-файла. Нужны тесты COM, MZ, DOS extender, файлов, русского текста, звука и выхода. Старые версии Windows внутри эмуляции устанавливаются только из допустимых пользовательских носителей; не брать системные файлы из случайных архивов.

## 6. Игры, DirectX и runtime

Desktop Wine и игровой runtime поставляются раздельно. Proton — инструмент Steam Play, включающий Wine и другие компоненты; интеграция должна учитывать его предназначение, версии и окружение. Это не универсальный переключатель для всех EXE: [Proton](https://github.com/ValveSoftware/Proton).

Тестировать D3D9/10/11/12 отдельно, shader cache, presentation, input latency, controllers, audio, fullscreen, overlay и suspend. Для каждой игры фиксировать renderer, GPU, driver и параметры. DXVK/vkd3d-proton не заменяют kernel GPU driver. Anti-cheat, DRM, VR и protected video выделять как отдельные зависимости.

## 7. Каталог приложений и результаты

Первые кандидаты: архиватор, текстовый редактор, файловая утилита, простое Qt-приложение, .NET WinForms fixture, MSI fixture, несколько D3D fixtures, DOS editor и DOS game. Конкретные версии 7-Zip, Notepad++, VLC или других известных программ можно включить после проверки прав распространения и получения установщиков из первичного источника. Это список кандидатов, не заявление о совместимости.

Каждая запись теста содержит:

- app ID, название, точную версию, архитектуру, SHA-256 installer и источник;
- OS build, kernel, runtime build, GPU/driver, VM configuration;
- сценарии install/start/open/edit/save/reopen/print/network/update/uninstall;
- expected result, observed result, логи, screenshot при необходимости;
- статус, дату, исполнителя, issue и воспроизводимые шаги;
- необходимость Windows VM или стороннего закрытого компонента.

Статусы: `untested`, `blocked`, `fails`, `partial`, `passes`, `regression`. `passes` означает все обязательные сценарии данной записи. Отдельная метка `vm-only` не превращается в `passes` Wine. «Запускается» — промежуточный результат.

Метрики публикуются по версиям и категориям: полностью прошедшие / весь замороженный каталог; partial отдельно; x86 отдельно от x64; native отдельно от VM; аппаратные конфигурации отдельно. Для каждого выпуска хранить прежний каталог и причины изменения набора.

## 8. VM fallback

Опциональный backend использует QEMU/KVM там, где доступно; на других хостах производительность и ускорение проверяются отдельно. Пользователь самостоятельно предоставляет допустимую гостевую ОС. Интеграция окна/clipboard/files требует гостевого агента, разрешений и собственной проверки безопасности.

VM может помочь конкретному несовместимому приложению, но не гарантирует работу античита, USB dongle, GPU feature или DRM. GPU passthrough требует подходящего оборудования и конфигурации. VM не включается в минимальный ISO и не используется как скрытая зависимость обещанной открытой системы.
