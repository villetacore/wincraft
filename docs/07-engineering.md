# Разработка, качество и сопровождение

## 1. Предлагаемая структура репозитория

Это будущая организация; текущие `src/` и scripts сохраняются до отдельных refactoring tasks.

```text
crates/
  init/                 PID 1 / supervisor
  desktop/              UI оболочки
  app-manager/          каталог, установка, ярлыки
  runtime-broker/       управление запусками
  runtime-wine/         Wine backend
  runtime-dos/          DOS backend
  formats/              PE/ELF/MZ/NE classification
  platform/             OS abstraction для UI
  ipc/                  версии сообщений и типы ошибок
  storage/              планирование операций с диском
  installer/            UI и orchestration
  updater/              manifests, slots, rollback
  diagnostics/          журналы и отчёты
os/                     конфигурация guest и политики
build/                  декларативные recipes и lock manifests
tests/fixtures/         небольшие воспроизводимые программы
tests/scenarios/        OS и app acceptance scenarios
docs/                   архитектура, RFC, release notes
research/kernel/        только после отдельного R&D решения
```

Разделять библиотеки с чистой логикой и privileged executables. Desktop host preview не должен монтировать диски, выключать хост или выполнять guest administrative path. Для platform APIs нужны реальные и mock/preview backends с явной конфигурацией.

## 2. Rust policy

- Stable Rust по умолчанию; pin toolchain и MSRV. Nightly только при документированной необходимости.
- `cargo fmt`, `cargo clippy`, `cargo test --locked` для изменённых crates и platform CI.
- `unsafe` локализуется; каждый блок объясняет safety invariants, ownership и lifetime.
- FFI проверяет размеры, ABI layouts, nulls, ownership и ошибки; Rust panic не пересекает C ABI.
- Использовать `Result` и типизированные ошибки в системных службах; panic не является обработкой обычного I/O failure.
- Не собирать shell-команды из пользовательских строк; argv и allowlisted operations.
- Новая зависимость оценивается по сопровождаемости, лицензии, unsafe surface и transitive graph.
- Avoid Rust-specific binary ABI across service boundaries: IPC/C ABI с версионированием.
- `no_std` нужен исследовательскому ядру, а не всем desktop crates.

## 3. Тестовая пирамида

| Уровень | Проверка | Пример |
| --- | --- | --- |
| Unit | Чистые инварианты | Bounds PE, profile migration, slot selection |
| Property/fuzz | Недоверенные входы | PE headers, manifest, IPC framing, archive paths |
| Integration | Реальное взаимодействие | Broker → Wine → file/exit/log result |
| OS VM | Загрузка и система | ISO boot, user UID, restart service, persistent data |
| Install/update faults | Отказы транзакций | Power loss, full disk, bad signature, unplug |
| Application scenarios | Пользовательский результат | Install/edit/save/reopen/uninstall |
| Hardware | Поведение реального ПК | GPU, Wi-Fi, sound, suspend, external monitor |
| Soak | Длительная стабильность | Запуски/остановки, RAM/FD leaks, повторные update |

Не писать тесты, которые лишь повторяют реализацию. Проверять наблюдаемое поведение и независимые инварианты. Для desktop screenshots применять только устойчивые области и допуски; они не заменяют проверки результата файла, текста, статуса процесса и доступности.

QEMU job фиксирует firmware, machine type, CPU, RAM, disk image hash, acceleration и timeout. TCG/программная графика не используются для заявлений о native performance. Тесты, меняющие диск, работают только с disposable images. После провала сохраняются serial/QMP logs, screenshot, manifest и причина timeout.

Сравнительные Windows-тесты помогают находить семантические различия, но не требуют копирования кода Windows. Фиксировать входы/выходы и допустимые права использования тестовой среды. Performance сравнивать по медиане и хвостовым задержкам нескольких повторов, на одинаковых данных и аппаратуре.

## 4. CI и выпуск

PR: format/lint + затронутые unit/integration checks; changes в boot/init/runtime требуют соответствующей VM проверки. Nightly: clean image build, ISO boot и основной compatibility corpus. Release candidate: полная установка/update/rollback, HCL subset, manifest/source audit и publish rehearsal.

Release gate блокируют: data loss, privilege escalation, broken boot/update, invalid signatures, незаявленная регрессия обязательного приложения. Нестабильный тест не отключается молча: issue, владелец, ограниченный срок и влияние на release decision.

Выпуск содержит release notes, known issues, supported hardware/apps, checksums/signatures, SBOM, source location и upgrade instructions. Не публиковать секреты signing keys в CI env/logs или ISO. Отдельные dev/release keys; release signing только после проверки артефакта, который реально протестирован.

## 5. Модель угроз

| Угроза | Базовая защита | Остаточная проблема |
| --- | --- | --- |
| Вредоносный EXE | Non-root, namespaces, resource limits, ограниченные mounts | Wine не sandbox; shared desktop/files остаются каналами |
| Подмена runtime/update | Проверяемые manifests, подписи, pinned provenance | Компрометация signing infrastructure |
| Malformed file/IPC | Safe parsers, size limits, fuzzing | Ошибки FFI и upstream |
| Привилегированный helper | Узкие операции, peer auth, повторная валидация | Диски и mount — критическая поверхность |
| Межприложный доступ | Per-app storage + OS isolation | Общие X11 session/IPC могут нарушать изоляцию |
| Утечка отчётов | Локальный export и просмотр перед отправкой | Логи приложений могут содержать документы/токены |

Sandbox строится отдельно от prefix: mount/user namespaces где применимо, seccomp с совместимым профилем, capabilities drop, cgroups и ограничение сети. Нельзя просто запретить syscalls без regression suite: JIT, Wine и graphics runtimes используют сложные механизмы. Для сильной границы недоверенного ПО предпочтительна VM; её shared resources также ограничиваются.

Крупные закрытые приложения не считаются безопасными только из-за известного издателя. Проверка цифровой подписи файла показывает происхождение в пределах цепочки доверия, но не доказывает отсутствие вредоносного поведения.

## 6. Лицензии и происхождение

Собственные компоненты распространяются по MIT (см. корневой LICENSE). Ядро, Wine, runtime, fonts, codecs и firmware сохраняют свои условия; объединённый ISO не становится целиком MIT.

Для каждого пакета хранить SPDX identifier, версию/hash, первичный источник, локальные изменения, текст лицензии и способ получения соответствующих исходников. Перед публичным релизом проверить конкретные обязанности выбранных лицензий и формы распространения, а также включить необходимые notices и исходные материалы. Первичные тексты доступны в [Linux COPYING](https://github.com/torvalds/linux/blob/master/COPYING) и [Wine LICENSE](https://github.com/wine-mirror/wine/blob/master/LICENSE); требования определяются конкретными компонентами, а не названием проекта.

Не использовать утечки Microsoft, неизвестные бинарники из repack-сборок и случайные DLL-сайты. Proprietary runtimes включать только при проверенном разрешении на распространение; иначе дать пользователю официальный путь установки. Протоколы и форматы изучать по документации и допустимым поведенческим тестам; для чувствительных независимых реализаций вести clean-room records.

Товарные знаки, иконки, шрифты и звуки имеют отдельное происхождение. Сделать собственное визуальное оформление и не заявлять одобрение Microsoft. Название WinCraft требует отдельной проверки перед публичным брендингом; эта работа ещё не выполнена.

## 7. Поддержка и управление проектом

Назначить владельцев boot/storage, compatibility, desktop, security и release. PR должен объяснять проблему, наблюдаемое изменение и проверку; архитектурные изменения оформляются ADR. Для security reports нужен приватный канал, triage и политика выпуска исправлений; публичный issue с exploit не должен быть единственным способом сообщения.

Предлагаемые каналы: development, preview, stable. Stable получает только проверенные исправления и объявленные миграции. Срок поддержки определяется ресурсами до выпуска; не обещать LTS, пока нет людей и инфраструктуры для обновления kernel/runtime.

Definition of Done: поведение реализовано, релевантные проверки выполнены, ошибки и recovery описаны, документация обновлена, dependencies/provenance зафиксированы, ограничения отражены в матрице. Количество строк кода и красивый screenshot не являются критериями завершения ОС.
