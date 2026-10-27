# Источники и границы уверенности

Дата обращения: 14.10.2026. Ссылки — первичные технические источники. Версии upstream меняются; этот план не закрепляет автоматически текущий head. Любая используемая версия должна попасть в build manifest.

## 1. Локальные источники

- [README текущей системы](../README.md) — существующие команды и объявленные ограничения.
- [Cargo.toml](../Cargo.toml) — версия и зависимости Rust.
- [src/init.rs](../src/init.rs) — действующие привилегии, mounts, X startup и recovery shell.
- [src/compat.rs](../src/compat.rs) — фактический уровень PE validation и вызов Wine.
- [src/main.rs](../src/main.rs) — текущий desktop.
- [build-os.sh](../scripts/build-os.sh), [pack-os.sh](../scripts/pack-os.sh) — сборка и упаковка.
- [run-qemu.ps1](../scripts/run-qemu.ps1), [smoke-qemu.ps1](../scripts/smoke-qemu.ps1) — текущий VM mode и тест.
- [wincraft-session](../os/wincraft-session) — root session, software rendering, background wineboot.

Из чтения кода установлено наличие механизмов, но не их успешная работа в текущем окружении. Сборка и smoke должны быть повторены задачами M0.

## 2. Первичные внешние источники

| Источник | Для чего используется |
| --- | --- |
| [Wine repository / README](https://github.com/wine-mirror/wine) | Роль loader/API implementation и структура исходников; не гарантия конкретного приложения |
| [ReactOS architecture](https://reactos.org/architecture/) | Отличие NT-подобного подхода от Unix-базы |
| [Linux Rust documentation](https://www.kernel.org/doc/html/latest/rust/index.html) | Rust в Linux и его инфраструктура; не собственное полностью Rust-ядро |
| [Proton repository](https://github.com/ValveSoftware/Proton) | Gaming runtime на основе Wine и компонентов Steam Play |
| [DXVK repository](https://github.com/doitsujin/dxvk) | Direct3D через Vulkan, требования выбранного release |
| [vkd3d-proton repository](https://github.com/HansKristian-Work/vkd3d-proton) | Direct3D 12 для Proton и аппаратные требования |
| [DOSBox Staging](https://www.dosbox-staging.org/) | Кандидат DOS runtime; проверять нужный тип legacy software |
| [Microsoft: user mode / kernel mode](https://learn.microsoft.com/en-us/windows-hardware/drivers/gettingstarted/user-mode-and-kernel-mode) | Различие приложений и kernel components |
| [Linux OverlayFS](https://docs.kernel.org/filesystems/overlayfs.html) | Слоистый rootfs и ограничения upper layer |

Дополнительные первичные материалы для последующей реализации: [QEMU system documentation](https://www.qemu.org/docs/master/system/), [GNU GRUB manual](https://www.gnu.org/software/grub/manual/grub/grub.html), [Microsoft PE format](https://learn.microsoft.com/en-us/windows/win32/debug/pe-format), [Linux COPYING](https://github.com/torvalds/linux/blob/master/COPYING), [Wine LICENSE](https://github.com/wine-mirror/wine/blob/master/LICENSE). Их конкретные параметры и требования нужно перепроверять при реализации; недоступная страница не считается прочитанным доказательством.

## 3. Что является предложением проекта

Выбор Linux как основной ветки, runtime broker, A/B slots, структура crates, milestones, каталог на 100 приложений, порог 90%, оценки сроков и начальная HCL — инженерные предложения этой документации. Они не взяты из upstream как обещания и не подтверждают сегодняшнюю работоспособность WinCraft.

Нет заявлений о прохождении конкретных современных коммерческих программ, поддержке всех драйверов, юридическом разрешении на распространение proprietary assets или достигнутой воспроизводимости. Для таких утверждений нужны собственные артефакты проверки и решения владельца проекта.
