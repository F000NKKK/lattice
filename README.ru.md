# Net Lattice

**Языки**

🇺🇸 [English](README.md) | 🇷🇺 **Русский**

[![License: MPL 2.0](https://img.shields.io/badge/license-MPL--2.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/language-Rust-orange.svg)](https://www.rust-lang.org)
[![crates.io](https://img.shields.io/crates/v/net-lattice.svg)](https://crates.io/crates/net-lattice)
[![docs.rs](https://img.shields.io/docsrs/net-lattice)](https://docs.rs/net-lattice)
[![Downloads](https://img.shields.io/crates/d/net-lattice.svg)](https://crates.io/crates/net-lattice)
[![MSRV](https://img.shields.io/badge/MSRV-1.93-lightgrey.svg)](Cargo.toml)

![Linux](https://img.shields.io/badge/Linux-supported-success)
![Windows](https://img.shields.io/badge/Windows-supported-success)
![macOS](https://img.shields.io/badge/macOS-supported-success)

**Net Lattice** — это современная кроссплатформенная библиотека для Rust, предназначенная для настройки и анализа сетевой конфигурации операционной системы через единый строго типизированный API.

> **Статус:** Net Lattice предоставляет кроссплатформенный просмотр сети,
> изменение маршрутов, адресов, DNS, administrative state и MTU интерфейсов,
> inspectable планы mutation-операций и упорядоченное исполнение транзакций с
> cancellation, snapshots, compensation и фазовыми отчётами через нативные API
> Linux, Windows и macOS. Stage 0.16 проверен privileged CI-задачами для
> Linux, Windows и macOS; см. «Текущий статус» ниже.

## Обзор

Операционные системы предоставляют доступ к сетевой конфигурации и состоянию через совершенно разные, низкоуровневые и зачастую платформо-специфичные интерфейсы: Linux Netlink, Windows IP Helper API, BSD routing facilities в macOS и другие механизмы. Приложениям, которым необходимо анализировать или настраивать сеть — IP-адреса, маршруты, интерфейсы, соседей и многое другое, — как правило, приходится либо вызывать внешние утилиты через shell, либо парсить текстовый вывод, либо писать и поддерживать отдельные платформо-специфичные интеграции.

Net Lattice призвана объединить эти интерфейсы под единым, строго типизированным, идиоматичным API на Rust, чтобы потребителям никогда не приходилось иметь дело с сырыми платформенными структурами, shell-командами или произвольным парсингом строк.

## Crates workspace

Workspace разделён на отдельные crates. У каждого crate есть собственный
README с назначением и примером использования:

| Crate | Назначение |
|---|---|
| [`net-lattice`](crates/net-lattice/README.md) | Публичный фасад и transaction executor |
| [`net-lattice-model`](crates/net-lattice-model/README.md) | Observed state, intent, события и mutation plans |
| [`net-lattice-platform`](crates/net-lattice-platform/README.md) | Provider- и capability-контракты |
| [`net-lattice-core`](crates/net-lattice-core/README.md) | Общие ошибки, результаты и ID |
| [`net-lattice-ip`](crates/net-lattice-ip/README.md) | IPv4/IPv6 адреса и сети |
| [`net-lattice-async`](crates/net-lattice-async/README.md) | Runtime-independent адаптер event stream |
| [`net-lattice-backend-linux`](crates/net-lattice-backend-linux/README.md) | Linux Netlink backend |
| [`net-lattice-backend-windows`](crates/net-lattice-backend-windows/README.md) | Windows IP Helper backend |
| [`net-lattice-backend-darwin`](crates/net-lattice-backend-darwin/README.md) | macOS BSD/PF_ROUTE backend |

## Экосистема Lattice

Net Lattice — первый crate в более широком семействе Lattice: композируемых,
кроссплатформенных Rust-библиотек для сети. Остальные репозитории находятся
на стадии инициализации — служебная инфраструктура и упаковка уже есть, но
реализации и публичного API ещё нет — и проектируются так, чтобы дополнять
Net Lattice, а не дублировать его.

| Crate | Назначение |
|---|---|
| [net-lattice](https://github.com/F000NKKK/net-lattice) | Инспекция и настройка сетевого стека ОС (маршруты, DNS, интерфейсы) |
| [tunnel-lattice](https://github.com/F000NKKK/tunnel-lattice) | TUN/TAP туннельные интерфейсы |
| [dns-lattice](https://github.com/F000NKKK/dns-lattice) | Программируемый DNS control plane |
| [flow-lattice](https://github.com/F000NKKK/flow-lattice) | Компилятор политик: правила -> платформенно-нейтральные сетевые планы |
| [sdk-lattice](https://github.com/F000NKKK/sdk-lattice) | Прикладной SDK, объединяющий crate'ы выше |

Направление зависимостей между репозиториями и границы API ещё не определены;
они будут зафиксированы в архитектурных документах и ADR каждого репозитория
по мере того, как эта проработка будет происходить.

## Мотивация

Кроссплатформенные сетевые инструменты в экосистеме Rust фрагментированы. Существующие решения зачастую платформо-специфичны, неполны или построены на вызове системных утилит, таких как `ip`, `netsh` или `route`. Это хрупко, сложно тестировать и не подходит для создания надёжного, production-grade программного обеспечения для управления сетью.

Net Lattice призвана закрыть этот пробел, предоставив единый, хорошо спроектированный уровень абстракции над нативными сетевыми API операционных систем.

## Философия

- **Строгая типизация вместо строк.** Потребители взаимодействуют с типизированными значениями Rust — адресами, префиксами, маршрутами, интерфейсами — а не с сырыми строками или shell-командами.
- **Нативные API, а не подпроцессы.** Net Lattice обращается напрямую к платформенным сетевым API (Netlink, IP Helper API, route sockets), а не вызывает внешние CLI-инструменты.
- **Кроссплатформенность по замыслу.** Единая поверхность API с платформо-специфичными реализациями, чтобы приложениям не приходилось делать особые случаи для каждой операционной системы.
- **Корректность и безопасность прежде всего.** Настройка сети — чувствительная область; библиотека должна затруднять представление некорректных состояний.
- **Постепенный, продуманный рост.** Функциональность добавляется осознанно, с вниманием к дизайну API и долгосрочной поддерживаемости, а не поспешно, чтобы покрыть все мыслимые сценарии использования.

## Возможности

Уже реализовано:

- Типы IPv4/IPv6-адресов и префиксов
- Просмотр и изменение адресов интерфейсов
- Просмотр и изменение маршрутов
- Просмотр интерфейсов
- Настройка administrative state и MTU интерфейсов
- Просмотр и изменение конфигурации DNS-резолвера
- Inspectable планы mutation-операций для маршрутов, адресов и DNS
- Упорядоченное исполнение mutation-планов с cancellation, snapshots, явной
  compensation и фазовыми отчётами
- Таблицы соседей (ARP/NDP)
- Мониторинг сети и уведомления об изменениях
- Опциональный runtime-agnostic async stream событий

Запланировано:

- VLAN
- VRF
- Сетевые пространства имён (namespaces)
- Интеграция с firewall
- Декларативная настройка сети

## Не входит в задачи проекта

- Net Lattice не является заменой полноценным демонам управления сетью (например, NetworkManager, systemd-networkd).
- Net Lattice не ставит целью предоставление интерфейса командной строки или графического интерфейса в составе основной библиотеки.
- Net Lattice не ставит целью парсинг или оборачивание вывода внешних CLI-инструментов в качестве долгосрочной стратегии.
- Net Lattice не ставит целью поддержку всех мыслимых сетевых протоколов или вендорских расширений с первого дня.

## Текущий статус

Реализация этапа 0.16 плана поэтапной поставки из
[архитектуры](ARCHITECTURE.ru.md) проверена privileged CI-задачами:

- `net-lattice-core`, `net-lattice-ip`
- модули `route`, `mac`, `interface`, `dns`, `neighbor`, `ifaddr`, `event` и `mutation` в `net-lattice-model`; `NewInterfaceAddress` и `NewDnsConfig` выражают намерение изменения отдельно от наблюдаемого состояния
- `RouteProvider`, `InterfaceProvider`, `InterfaceMutator`, `DnsProvider`, `DnsMutator`, `NeighborProvider`, `AddressProvider`, `AddressMutator`, `CapabilityProvider`, синхронные `EventProvider`/bounded `EventReceiver` и опциональная async-поддержка мониторинга в `net-lattice-platform`
- `net-lattice-async`, предоставляющий единый runtime-agnostic тип `EventStream`
- фасад `net-lattice`, включая `Lattice::add_address()`, `Lattice::remove_address()`, `Lattice::set_dns_config()`, `Lattice::set_interface_config()`, `Lattice::capabilities()`, `Lattice::supports()`, `Lattice::watch()`, `Lattice::watch_filtered()`, `Lattice::execute_plan()` и feature-gated `Lattice::watch_async()`

Это даёт реальное управление маршрутами и IP-адресами интерфейсов, desired-патчи `InterfaceConfig` для administrative state и MTU, просмотр интерфейсов, просмотр и изменение DNS-конфигурации резолвера, чтение таблиц соседей (ARP/NDP), inspectable планы mutation-операций, упорядоченное исполнение транзакций и bounded-мониторинг сетевых изменений на Linux, Windows и macOS. `InterfaceConfig` не переиспользует observed `Interface`: он выбирает один интерфейс и запрашивает одно или оба поддерживаемых свойства. Для каждого свойства проверяйте `Capability::INTERFACE_ADMIN_STATE` и `Capability::INTERFACE_MTU`. Native backend может применять свойства разными вызовами, поэтому ошибка combined patch может означать partial application; перечитайте состояние и при необходимости используйте явный compensator executor'а. Создание адреса принимает `NewInterfaceAddress` и возвращает результирующий наблюдаемый `InterfaceAddress`; замена конфигурации резолвера принимает `NewDnsConfig` и возвращает результирующий наблюдаемый `DnsConfig`. `MutationPlan` — только данные, а `Lattice::execute_plan` исполняет его через единый `ExecutionOptions` с runtime-проверками, cancellation на границах операций, типизированными snapshots, явной compensation и фазовыми отчётами. `EventFilter` сочетает селекторы доменов (`routes()`) и объектов (`route(route_id)`); каждый backend применяет filter до помещения обычного события в очередь. Перед watching проверяйте capability каждого выбранного filter-домена; `Capability::MONITORING` означает, что доступны все текущие домены. Feature `async` в Net Lattice использует и реэкспортирует реализацию `EventStream` из `net-lattice-async`; приложению достаточно включить эту feature фасада. Это всё ещё не полноценная библиотека: VLAN, VRF, namespaces, интеграция с firewall, декларативная настройка сети и другие продвинутые возможности ещё впереди; см. [ARCHITECTURE.ru.md](ARCHITECTURE.ru.md) для поэтапной дорожной карты и [CHANGELOG.md](CHANGELOG.md) для того, что реально вышло.

| Возможность | Linux | Windows | macOS |
|---|:---:|:---:|:---:|
| Просмотр маршрутов | ✅ | ✅ | ✅ |
| Изменение маршрутов | ✅ | ✅ | ✅ |
| Просмотр интерфейсов | ✅ | ✅ | ✅ |
| Настройка administrative state/MTU интерфейсов | ✅ | ✅ | ✅ |
| Просмотр адресов интерфейсов | ✅ | ✅ | ✅ |
| Изменение адресов интерфейсов | ✅ | ✅ | ✅ |
| Просмотр таблицы соседей | ✅ | ✅ | ✅ |
| Просмотр DNS-резолвера | ✅ | ✅ | ✅ |
| Изменение DNS-резолвера | ✅ | ✅ | ✅ |
| Мониторинг изменений маршрутов/интерфейсов/адресов | ✅ | ✅ | ✅ |
| Мониторинг изменений соседей | ✅ | — | ✅ |
| Мониторинг всех доменов (`watch()`) | ✅ | — | ✅ |
| Async-мониторинг маршрутов/интерфейсов/адресов | ✅ | ✅ | ✅ |
| Async-мониторинг соседей/всех доменов | ✅ | — | ✅ |

### Доставка событий

Потоки событий bounded. Если consumer не успевает обрабатывать события, watcher запоминает и выдаёт `Event::ResyncRequired { .. }` перед последующим обычным событием, а не сохраняет неограниченный backlog. Прежде чем полагаться на последующие события, перечитайте состояние затронутого provider.

Capabilities мониторинга описывают фактическую native-доставку. Netlink в
Linux и PF_ROUTE в macOS доставляют изменения маршрутов, интерфейсов, адресов
интерфейсов и соседей, поэтому публикуют aggregate
`Capability::MONITORING`. IP Helper в Windows доставляет только маршруты,
интерфейсы и unicast-адреса: используйте соответствующую capability
`ROUTE_MONITORING`, `INTERFACE_MONITORING` или `ADDRESS_MONITORING` вместе с
`watch_filtered`. Запрос neighbors или всех доменов в Windows завершается
`Error::Unsupported` до native-регистрации — выбранный домен никогда не
теряется молча.

```rust
let route_events = EventFilter::none().route(route_id);
if lattice.supports(Capability::ROUTE_MONITORING) {
    let watcher = lattice.watch_filtered(route_events)?;
    # let _ = watcher;
}
```

## Примеры

Запускаемые исходники в
[`crates/net-lattice/examples`](crates/net-lattice/examples) покрывают каждую
доступную сейчас операцию фасада. Примеры только для чтения безопасны для
запуска; примеры mutation требуют явного opt-in через переменную окружения и
повышенных прав операционной системы.

| Сценарий | Запускаемый пример | Покрываемый фасад/API |
|---|---|---|
| Полное состояние только для чтения | [`snapshot`](crates/net-lattice/examples/snapshot.rs) | `capabilities`, `interfaces`, `routes`, `addresses`, `dns_config`, `neighbors` |
| Выбор возможностей во время работы | [`capabilities`](crates/net-lattice/examples/capabilities.rs) | `capabilities`, `supports`, все текущие флаги `Capability` |
| Точечное чтение маршрутов | [`list_routes`](crates/net-lattice/examples/list_routes.rs) | `routes` |
| Bounded синхронная доставка | [`sync_monitor`](crates/net-lattice/examples/sync_monitor.rs) | capability-gated `watch_filtered`, `recv_timeout`, `Event::ResyncRequired` |
| Фильтрация доменов и объектов | [`filtered_monitor`](crates/net-lattice/examples/filtered_monitor.rs) | `watch_filtered`, все domain/object selectors `EventFilter` |
| Нативная async-доставка | [`async_monitor`](crates/net-lattice/examples/async_monitor.rs) | capability-gated `watch_async`, `EventStream` |
| Жизненный цикл адреса | [`address_assignment`](crates/net-lattice/examples/address_assignment.rs) | `NewInterfaceAddress`, `add_address`, `remove_address` |
| Жизненный цикл маршрута | [`route_mutation`](crates/net-lattice/examples/route_mutation.rs) | `Route`, `add_route`, `remove_route` |
| Замена конфигурации резолвера | [`dns_mutation`](crates/net-lattice/examples/dns_mutation.rs) | `NewDnsConfig`, `set_dns_config`, read-after-write verification |
| Настройка интерфейса | [`interface_configuration`](crates/net-lattice/examples/interface_configuration.rs) | `InterfaceConfig`, `DesiredAdminState`, capability checks, `set_interface_config` |
| Просмотр mutation | [`mutation_plan`](crates/net-lattice/examples/mutation_plan.rs) | все варианты `Mutation`, `Mutation::semantics`, `MutationPlan` |

Запуск: `cargo run -p net-lattice --example <name>`. Для `async_monitor`
добавьте `--features async`.

Краткое руководство для приложений находится в
[`README` крейта `net-lattice`](crates/net-lattice/README.md). Остальные
руководства из таблицы workspace описывают прямое использование библиотечных
и backend-крейтов без дублирования этих контрактов здесь.

## Дорожная карта

1. **Bootstrap** *(завершён)* — инфраструктура репозитория, лицензирование, файлы для сообщества и настройка инструментов.
2. **Проектирование** *(завершено)* — структура крейтов, базовые абстракции и стратегия абстрагирования платформ реализованы на этапе 0.1. См. [ARCHITECTURE.ru.md](ARCHITECTURE.ru.md).
3. **Фундамент** *(завершено)* — реализованы базовые типы IP/маршрутов/интерфейсов и все три платформенных бэкенда.
4. **Паритет платформ** *(завершён)* — реализованы Linux/Windows/macOS backend'ы для изменения маршрутов и адресов, интерфейсов, чтения DNS, чтения соседей, чтения адресов и мониторинга.
5. **Stage 0.9: Изменение адресов** *(завершён)* — кроссплатформенное назначение и удаление IPv4/IPv6-адресов интерфейсов.
6. **Stage 0.10: Семантика событий** *(завершён)* — bounded delivery, сигнализация overflow и resynchronization, filtering, cancellation и распространение ошибок.
7. **Stage 0.11: Async events** *(завершён)* — опциональная feature фасада `async`, единый runtime-agnostic `EventStream` и нативная Tokio-backed доставка в каждом платформенном backend.
8. **Stage 0.12: Стабилизация API watcher'ов** *(завершён)* — composable filters по объектам/доменам, filtering до помещения в очередь, validation capability мониторинга и одинаковая семантика filter для sync/async watcher'ов с сохранением опубликованного API 0.11.
9. **Stage 0.13: Изменение DNS** *(завершён)* — замена конфигурации резолвера через поддерживаемые системные механизмы, закрытая capability, на Linux, Windows и macOS.
10. **Stage 0.14: Модель mutation-операций** *(завершён)* — inspectable значения `Mutation` и планы `MutationPlan` только из данных для существующих изменений routes, addresses и DNS; явно определены preconditions, idempotency, privileges, confirmation, partial application и reversibility.
11. **Stage 0.15: Исполнение транзакций** *(завершён)* — упорядоченные планы, результаты каждой операции, диагностика фаз и длительностей, границы cancellation и ошибок, а также compensation только для документированно reversible операций.
12. **Stage 0.16: Конфигурация интерфейсов** *(завершён)* — отдельная desired-конфигурация интерфейса, capability-gated изменение admin state и MTU, read-after-write результаты и platform-parity tests.
13. **Stage 0.17: Изменение соседей, паритет IPv6 для DNS и изолированная topology-приёмка** — intent/observed управление статическими ARP/NDP и безопасное кроссплатформенное тестирование деструктивных операций. Детальный план будет подготовлен до начала реализации.
14. **Stage 0.18: Snapshots** — последовательно собранный `CurrentState` с явно определёнными scope, consistency и partial-read семантиками.
15. **Stage 0.19: Декларативный diff** — отдельные конфигурационные типы `DesiredState` и inspectable `Diff` без mutation.
16. **Stage 0.20: Декларативное применение** — компиляция `Diff` в `ApplyPlan` и его исполнение через transaction engine.
17. **Stage 0.21: Pre-1.0 hardening** — заморозка публичных контрактов, правил identity и capability, гарантий событий, матрицы платформ и privileged regression coverage.
18. **Stage 0.22+: Домены Capability** — VLAN, VRF, namespaces, firewall и tunnels, каждый с полным контрактом read/intent/mutation/event/capability/tests. Они не являются prerequisite для 1.0.
19. **1.0** — стабильная основа для реализованных контрактов inspection, monitoring, imperative mutation, transactions и declarative apply. Она закрывается compatibility audit из 0.21, а не каждым будущим сетевым доменом.

Этапы — это границы поставки, а не обещание одного релиза на каждый заголовок: platform validation может разделить этап, а focused hardening-релизы могут появляться между этапами.

## Участие в проекте

Вклад в проект приветствуется. См. [CONTRIBUTING.md](CONTRIBUTING.md) для рекомендаций, [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md) для правил поведения в сообществе и [SECURITY.md](SECURITY.md) для сообщения о проблемах безопасности.

## Лицензия

Net Lattice распространяется под лицензией [Mozilla Public License 2.0](LICENSE) (`MPL-2.0`).
