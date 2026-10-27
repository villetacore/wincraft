# Собственное Unix-подобное ядро на Rust

## 1. Место в проекте

Это долгосрочная исследовательская программа, а не скрытое обещание переписать Linux за несколько релизов. Она нужна, если собственное ядро — самостоятельная ценность проекта. Продуктовая Linux-ветка позволяет проверять UI и сценарии приложений до готовности kernel R&D.

Если собственное ядро обязательно с первого ISO, порядок меняется: R0–R3 → минимальный userspace → ISO → ограниченные приложения. При этом современная Windows-совместимость откладывается до R4–R6. Нельзя сохранить одновременно ранний срок, новое ядро и широту зрелой платформы без огромного увеличения ресурсов.

## 2. Кандидат архитектуры

Рекомендуемый исследовательский вариант: модульное Unix-подобное ядро с небольшими внутренними интерфейсами и максимально безопасными Rust abstractions. Это предложение, которое нужно сравнить с microkernel/hybrid на прототипах IPC, memory и driver lifetime.

Минимальный trusted core: privilege transitions, page tables, scheduler, interrupt routing, process isolation и handle/capability checks. Driver code разделяется по привилегиям там, где это практически оправдано. Нельзя назвать ядро memory-safe, если небезопасные DMA, FFI и lifetime contracts не проверены.

```text
Rust user programs / POSIX libc / future Wine port
        | native syscalls / Unix personality
        | optional NT user-mode services
------------------------------------------------
processes | VM | scheduler | IPC | VFS | security
------------------------------------------------
interrupts | timers | SMP | HAL | device framework
------------------------------------------------
UEFI / x86-64 / QEMU virtio first
```

Сначала один native syscall ABI с явным versioning. Затем Unix/POSIX personality; Linux ABI compatibility — отдельный проект, не следствие наличия `open/read/write`. NT kernel personality не включается автоматически в эту архитектуру.

## 3. Этапы R&D

| Этап | Реализация | Gate |
| --- | --- | --- |
| R0: boot | UEFI handoff, memory map, serial, framebuffer, panic | Повторная загрузка в QEMU, полезный crash report |
| R1: CPU/memory | Exceptions, page tables, allocator, timers, interrupts | Guard pages, deliberate page fault, allocation stress |
| R2: execution | Ring 3, syscall boundary, threads, scheduler, ELF loader | Два изолированных процесса; падение одного не рушит другой |
| R3: devices/storage | PCI, virtio block/input/net, VFS, initramfs, базовая FS | Read/write persistence, ограничение доступа, сетевой fixture |
| R4: Unix userspace | FD, pipes, signals, mmap, threads, polling, libc port | Shell и набор Unix-программ; систематические ABI tests |
| R5: graphics/runtime | Display/input stack, fonts, audio и порт нужных библиотек | GUI-приложение, resource cleanup, долгий soak |
| R6: Wine feasibility | Thread/memory/exception/IPC primitives и Wine port | Одинаковый PE corpus на Linux и новом ядре с отчётом различий |

SMP, locking и atomics должны иметь отдельные stress tests; не добавлять несколько CPU без доказанного correctness на одном. Networking сначала минимальный функциональный scope с fuzzing входных пакетов. Собственный TLS stack не нужен: портировать сопровождаемый userspace-компонент.

## 4. Что понадобится для Wine

Одной POSIX-совместимости недостаточно. Понадобятся mmap и точные memory protections, executable mappings, thread-local state, signals/exceptions, многопоточность, shared memory, synchronization, clocks, файловые notifications, sockets, IPC и графические зависимости. Linux-specific paths Wine могут требовать адаптации или новых backends. Полный Linux binary ABI потребует также соответствующих ioctls, `/proc`, driver API и behaviour, а не только syscall numbers.

Создать compatibility requirements inventory из фактического выбранного Wine build: какие функции libc/ядра и сторонние библиотеки используются, какие обязательны для fixtures, какие можно временно отключить. Результат — проверяемая таблица зависимостей, не лозунг «портируем Wine».

## 5. NT personalities и драйверы

Если позже вводить NT-like executive, нужны object manager, handles/namespaces, processes/threads, tokens/ACL, sections, dispatcher objects, APC, I/O manager, IRP, PnP, power management, registry, security и exception semantics. Каждая подсистема взаимодействует с другими; нельзя заменять её набором stubs, возвращающих success.

Поддержка `.sys` дополнительно требует kernel ABI, calling conventions, memory layout, IRQL, DMA, interrupt synchronization, WDM/KMDF/NDIS/WDDM и соответствующей модели оборудования. NT-драйверы предполагают свойства NT kernel, которых Unix-ядро может не иметь. Запуск driver code внутри privileged host также расширяет trusted computing base.

Возможные варианты: вообще не поддерживать Windows drivers; реализовать узкую модель для отдельного класса; запускать драйвер с гостевой Windows и device passthrough; разрабатывать NT kernel personality. Выбор делается после конкретного аппаратного use case, а не по желанию «вся Microsoft тема». Для Linux-ветки приоритет — существующие Linux-драйверы.

## 6. Инженерные правила ядра

- `no_std`, контролируемый allocator и минимальный assembler только для архитектурной границы.
- Чёткое владение physical pages, mappings, DMA buffers и interrupt handlers.
- Typed handles с проверкой поколения для защиты от use-after-free через IDs.
- Lock ordering, interrupt-safe critical sections, отсутствие blocking под spinlock.
- User pointers всегда проверяются и копируются через безопасные интерфейсы.
- Panic/oom и частичная инициализация устройств имеют определённое поведение.
- Тесты чистых структур запускаются на хосте; VM tests проверяют real hardware semantics.
- Fuzz syscalls, malformed ELF, filesystem structures и network inputs.
- Kernel debugger, symbolized stack traces и crash artifacts нужны до сложного GUI.

## 7. Условия смены основной базы

Перевод desktop на собственное ядро рассматривается только после: сохранности данных; достаточного набора драйверов; network/security updates; сравнимого набора приложений; устойчивого build/recovery; понятного сопровождения. Должен существовать документированный способ миграции данных и возврата на Linux-редакцию.

Даже после успешного R6 собственное ядро может остаться отдельной экспериментальной редакцией. Это полноценный исследовательский результат; заменять рабочую платформу без пользовательского выигрыша не требуется.
