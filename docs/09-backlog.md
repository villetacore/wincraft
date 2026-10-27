# Начальный backlog

Таблицы задач определяют scope и acceptance; фактический статус существующих задач указан в разделе 6. Наличие кода не закрывает gate без проверок. Новые FIX-01…04, TEST-02, DESK-00 открыты. P0 — блокирует безопасный baseline/релиз, P1 — ближайший функциональный scope, P2 — дальнейшее развитие, R — исследование. Роли — предполагаемые владельцы, а не уже назначенные люди.

## 1. Основание проекта

| ID | Приоритет / этап | Задача | Зависит от | Acceptance / роль |
| --- | --- | --- | --- | --- |
| BASE-01 | P0 / M0 | Зафиксировать исходники и правила изменений | — | Git history или другой проверенный source archive; platform |
| BASE-02 | P0 / M0 | Inventory лицензий и собственный LICENSE | BASE-01 | Каждый поставляемый компонент имеет источник/условия; maintainer |
| BUILD-01 | P0 / M0 | Чистая сборка текущего образа | BASE-01 | Не использует старый chroot; сохраняет inputs/logs; build |
| BUILD-02 | P0 / M0 | Lock всех входных зависимостей | BUILD-01 | Toolchain, APK snapshot, Cargo, hashes; build |
| TEST-01 | P0 / M0 | Повторить текущий QEMU smoke | BUILD-01 | Отчёт boot/PE/file с manifest; QA |
| COMP-01 | P1 / M0 | Протестировать x64 и x86 Wine | TEST-01 | Два реальных fixture, не только parser; compatibility |
| ADR-01 | P1 / M0 | Сравнить musl и glibc runtime | COMP-01 | Один corpus, два build, решение с данными; platform |
| COMP-02 | P1 / M0 | Выбрать 5–10 реальных приложений, отдельно fixtures | BASE-02 | Версии, hashes, источники и сценарии; QA |

## 2. Live ISO и системные права

| ID | Приоритет / этап | Задача | Зависит от | Acceptance / роль |
| --- | --- | --- | --- | --- |
| BOOT-01 | P0 / M1 | Разделить early init и system init | BUILD-02 | switch_root с ошибками/recovery; platform |
| BOOT-02 | P1 / M1 | Squashfs + RAM overlay | BOOT-01 | Root читается с носителя, writes уходят в upper; platform |
| ISO-01 | P1 / M1 | UEFI bootloader и ISO packaging | BOOT-02 | VM boot с CD без `-kernel`; build |
| ISO-02 | P1 / M1 | ISO QEMU test runner | ISO-01 | 10 cold boots, firmware manifest, таймаут; QA |
| SEC-01 | P0 / M1 | Обычный пользователь и X authorization | TEST-01 | EXE и desktop UID не 0, нет `-ac`; platform |
| SEC-02 | P0 / M1 | Разделить debug/release конфигурации | SEC-01 | Release без открытого serial shell; security |
| INIT-01 | P0 / M1 | Critical mounts/readiness/crash backoff | BOOT-01 | Fault injection переводит в recovery; platform |
| INIT-02 | P1 / M1 | Управляемый shutdown | INIT-01 | Apps/services stopped, sync/unmount, clean reboot; platform |

### Срочные исправления существующей 0.3

| ID | Приоритет / этап | Задача | Зависит от | Acceptance / роль |
| --- | --- | --- | --- | --- |
| FIX-01 | P0 / M1 | Состояния Wine: preparing/ready/failed, timeout/cancel и журнал | BASE-01 | Неуспешный wineboot не создаёт ready; system.reg не единственная проверка; GUI различает ожидание и запуск; тест первого и повторного запуска после ошибки; Rust |
| FIX-02 | P0 / M1 | Проверять bind-mount /home и /apps | BASE-01 | Частичный отказ не выдаёт успешную persistence; явно показан временный режим или recovery; fault test проверяет фактические mounts; platform |
| FIX-03 | P0 / M1 | Retry/recovery при ошибке spawn сессии | BASE-01 | Отсутствующий/неисполняемый xinit даёт ограниченный backoff, затем диагностируемый отказ; нет бесконечного restart; platform |
| FIX-04 | P0 / M1 | Безопасное сохранение документа | BASE-01 | Временный файл в том же каталоге, sync и атомарная замена с учётом ОС; error/ENOSPC сохраняет старый файл; успех объявляется после завершения; Rust |
| TEST-02 | P0 / M1 | Данные после reboot и матрица отказов | FIX-01, FIX-02, FIX-03, FIX-04, ISO-01 | На одноразовом диске документ сохраняется через две загрузки; faults дают ожидаемые ошибки; logs и hashes; QA |

BUILD-02 включает закрепление APK, toolchain и доступных для повторной сборки артефактов. packages.txt фиксирует результат, но не заменяет lock входов. FIX-01 чинит нынешний общий prefix и не ждёт разработки per-app менеджера APP-04.

## 3. Приложения

| ID | Приоритет / этап | Задача | Зависит от | Acceptance / роль |
| --- | --- | --- | --- | --- |
| APP-01 | P1 / M2 | Вынести launcher из UI | TEST-01 | UI/CLI используют одну типизированную операцию с профилем, argv/cwd, runtime id, состоянием и журналом; Rust |
| APP-02 | P1 / M2 | Классификация PE/ELF/DOS/NE/MSI | APP-01 | Malformed inputs не crash; unknown не исполняется; Rust |
| APP-03 | P1 / M2 | Runtime manifest и pinning | ADR-01 | Запуск использует точный runtime hash; compatibility |
| APP-04 | P1 / M2 | Per-app prefix и wineboot readiness | APP-03 | Параллельный first launch не повреждает prefix; Rust |
| APP-05 | P1 / M2 | argv/cwd/env/job lifecycle | APP-04 | Spaces/Unicode/child processes/exit корректны; Rust |
| APP-06 | P1 / M2 | MSI install/uninstall и ярлыки | APP-05 | Fresh install → use → uninstall без чужих изменений; compatibility |
| APP-07 | P0 / M2 | Files/network isolation policy | SEC-01, APP-04 | Denied paths действительно недоступны, ограничения X11 описаны; security |
| APP-08 | P2 / M2 | DOS backend | APP-02 | COM/MZ/extender fixtures, mount boundary, русская раскладка; compatibility |
| APP-09 | P1 / M2 | Каталог и машинные результаты | COMP-02, APP-05 | Все записи имеют сценарии и статус, unknown не pass; QA |
| APP-10 | P1 / M2 | Prefix backup/migration | APP-04 | Неудачный upgrade позволяет восстановить совместимый prefix и старый runtime; документы не откатываются молча; Rust |

## 4. Установка, обновления и desktop

| ID | Приоритет / этап | Задача | Зависит от | Acceptance / роль |
| --- | --- | --- | --- | --- |
| STORE-01 | P0 / M3 | Storage inventory и план GPT | ISO-02 | Идентичность диска, exclude boot media, no-write preview; storage |
| INSTALL-01 | P1 / M3 | Установка на выделенный VM disk | STORE-01, SEC-02, DESK-00 | Извлечён ISO, boot с диска, persistent user data; storage |
| UPDATE-01 | P0 / M3 | Подписи и slot manifest | BUILD-02 | Bad signature/version/size отвергаются; security |
| UPDATE-02 | P1 / M3 | A/B trial boot и rollback | INSTALL-01, UPDATE-01 | Failed health check возвращает предыдущую ОС; platform |
| UPDATE-03 | P0 / M3 | Power-loss matrix | UPDATE-02 | Каждая фаза interruption оставляет recovery path; QA |
| DATA-01 | P0 / M3 | Миграции конфигурации и backups | APP-10, UPDATE-02 | Rollback не ломает schema/prefix/documents; storage |
| DESK-00 | P1 / M2.5 | Сеть, звук, браузер и базовый RU/EN в VM | APP-09, TEST-02 | HTTPS/download/save/reboot, звук и reconnect на заданной VM; desktop |
| DESK-01 | P1 / M4 | Расширение network/audio/locale/input на оборудование | DESK-00 | Сквозные сценарии, ошибки и reconnect; desktop |
| DESK-02 | P1 / M4 | Desktop usability/accessibility | DESK-01 | Keyboard-only, RU/EN, DPI, save dialogs; desktop |
| GPU-01 | P1 / M4 | Hardware graphics runtime | APP-03 | Vulkan capability и D3D fixtures на реальном GPU; graphics |
| HCL-01 | P1 / M4 | 2–3 аппаратные конфигурации | ISO-02, GPU-01 | Boot/network/audio/power/display reports; QA |
| WAY-01 | P1 / M2.5 | Wayland-сценарии после перехода (ADR-018) | TEST-01 | Clipboard/DnD Wayland↔XWayland, IME, fullscreen, DPI, `winewayland.drv` в профиле приложения; desktop |
| REL-01 | P0 / M5 | Release и security maintenance | UPDATE-03, HCL-01 | RC checklist, ключи, owners, source bundles; release |
| KERN-01 | R / R0 | Rust kernel feasibility | Отдельный ресурс | Boot/serial/panic и ADR без изменения main desktop; kernel |

## 5. Первые пять задач в работу

После фиксации текущего baseline: FIX-01 → FIX-02 → FIX-03 → FIX-04 → TEST-02. BUILD-01/02 и TEST-01/ISO-02 обеспечивают воспроизводимую приёмку, BASE-02 остаётся открытым. Затем COMP-01/02 и ADR-01 → APP-01…10 → DESK-00 → установка/обновления → оборудование. DOS остаётся P2 и не блокирует основной preview.

## 6. Статус после 0.3 (developer live preview, 14.10.2026)

Предыдущая запись документа сообщала о smoke в QEMU q35 + OVMF (UEFI, без Secure Boot, TCG). При обновлении плана VM не запускалась; эти результаты не переподтверждены. Статусы ниже описывают наличие реализации и прежние записи, а не новую приёмку. «Частично» означает неполный acceptance.

| ID | Статус | Что сделано / чего нет |
| --- | --- | --- |
| BASE-01 | Сделано | Git-репозиторий, `.gitattributes` (LF для guest-скриптов) |
| BASE-02 | Частично выполнено | Собственный код — MIT; для ISO нужен полный inventory внешних компонентов (см. docs/RELEASING.md) |
| BUILD-01 | Частично | Свежий chroot по умолчанию, блокировка параллельных сборок, журнал; `-Reuse` только для итераций |
| BUILD-02 | Открыто | APK-репозиторий не закреплён snapshot-ом |
| BOOT-01 | Частично | `os/early-init` → `switch_root`; при ошибке — сообщение, в debug — shell |
| BOOT-02 | Сделано | `rootfs.squashfs` (zstd) + tmpfs overlay; носитель ищется по build id, только ISO 9660 read-only |
| ISO-01 | Сделано | GRUB hybrid ISO, UEFI и BIOS; меню normal / verify media / debug |
| ISO-02 | Частично | Автотест 1 debug + 1 release загрузка; 10 холодных загрузок подряд ещё не автоматизированы |
| SEC-01 | Сделано | Desktop и EXE под UID 1000 через `su-exec`; Xorg с MIT-cookie, без `-ac` |
| SEC-02 | Частично | Serial root shell только в debug (cmdline `wincraft.debug` или QEMU fw_cfg); release-образ отдельно не собирается |
| INIT-01 | Частично | Критичные mounts → `WINCRAFT_INIT_FAILED`; backoff перезапуска сессии, стоп после 5 быстрых падений |
| INIT-02 | Частично | SIGTERM → 5 с → SIGKILL, sync, umount/remount ro данных, poweroff/reboot по запросу пользователя |
| APP-04 | Частично | Есть общий prefix и маркер; ошибка wineboot не проверяется, ожидание не ограничено (FIX-01). Per-app prefixes и перенос повреждённого prefix в текущем коде не реализованы |
| COMP-01 | Частично | x64 fixture проходит; x86 fixture запускается в отчёте smoke как информационный |

### Обновление 14.10.2026 (0.4, Wayland и управляемый Win32)

Проверено `scripts/smoke-iso.ps1` (QEMU q35 + OVMF, WHPX): 18 из 18 проверок, включая release-загрузку, на собственном дистрибутиве `wincraft-os.iso` и на переходной базе Alpine.

| ID | Статус | Что сделано / чего нет |
| --- | --- | --- |
| SEC-01 | Сделано (заново) | Xorg удалён (ADR-018): композитор labwc работает от UID 1000, привилегирован только seatd; smoke проверяет UID композитора и отсутствие Xorg |
| FIX-01 | Сделано | `wc-runtime::prefix`: состояния preparing/ready/failed с причиной в `/run/user/1000/wine-prefix.state`; `wineboot`/`wineserver` с таймаутом и проверкой кода выхода и реестра; лаунчер ждёт не дольше 240 с и сообщает причину. Тесты: unit + smoke |
| APP-01 | Частично | Лаунчер `start`: одна операция для Проводника, «Пуска», «Выполнить» и CLI; argv без shell, журнал `~/.local/state/wincraft/launches.log` (argv, cwd, runtime, PID, код выхода). Нет профилей приложений и выбора runtime |
| APP-02 | Частично | Понятные отказы для DOS (MZ, COM), Win16 (NE), ARM64 и неизвестных CPU, .NET без Wine Mono; DLL не запускаются. DOS-runtime (APP-08) нет |
| APP-03 | Частично | Идентичность runtime (`wine-11.0`) записывается в prefix; при смене сборки Wine prefix обновляется `wineboot --update`. Manifest с хешами нет |
| APP-04 | Частично | Повреждённый prefix переносится в `.wine.broken-*` (проверка реестра); per-app prefixes нет |
| WAY-01 | Открыто | Clipboard/DnD между Wayland и XWayland, IME, fullscreen, `winewayland.drv` не проверены |

## 7. Шаблон задачи

```text
ID и этап:
Проблема / пользовательский сценарий:
Текущее поведение и доказательство:
Желаемое поведение:
Границы задачи:
Зависимости:
Изменяемые компоненты:
Риски данных/привилегий/совместимости:
Acceptance criteria:
Как проверить и какие артефакты сохранить:
Recovery / rollback:
Документация и лицензии:
Владелец и статус:
```

Большую задачу разбивать по проверяемому результату: «загрузить ISO в UEFI VM», «запустить x86 fixture», «откатить неудачный update». Формулировки «добавить всю NT-совместимость» и «сделать современную ОС» не являются исполнимыми issue.
