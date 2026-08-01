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

> **Статус:** Net Lattice предоставляет кроссплатформенный просмотр сети, изменение маршрутов, адресов и DNS, inspectable планы mutation-операций и упорядоченное исполнение транзакций с cancellation, snapshots, compensation и фазовыми отчётами через нативные API Linux, Windows и macOS. Stage 0.15 плана архитектуры завершён; см. «Текущий статус» ниже.

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
- Просмотр и изменение конфигурации DNS-резолвера
- Inspectable планы mutation-операций для маршрутов, адресов и DNS
- Таблицы соседей (ARP/NDP)
- Мониторинг сети и уведомления об изменениях
- Опциональный runtime-agnostic async stream событий

Запланировано:

- VLAN
- VRF
- Сетевые пространства имён (namespaces)
- Интеграция с firewall
- Транзакционная конфигурация
- Декларативная настройка сети

## Не входит в задачи проекта

- Net Lattice не является заменой полноценным демонам управления сетью (например, NetworkManager, systemd-networkd).
- Net Lattice не ставит целью предоставление интерфейса командной строки или графического интерфейса в составе основной библиотеки.
- Net Lattice не ставит целью парсинг или оборачивание вывода внешних CLI-инструментов в качестве долгосрочной стратегии.
- Net Lattice не ставит целью поддержку всех мыслимых сетевых протоколов или вендорских расширений с первого дня.

## Текущий статус

Реализован этап 0.15 плана поэтапной поставки из [архитектуры](ARCHITECTURE.ru.md):

- `net-lattice-core`, `net-lattice-ip`
- модули `route`, `mac`, `interface`, `dns`, `neighbor`, `ifaddr`, `event` и `mutation` в `net-lattice-model`; `NewInterfaceAddress` и `NewDnsConfig` выражают намерение изменения отдельно от наблюдаемого состояния
- `RouteProvider`, `InterfaceProvider`, `DnsProvider`, `DnsMutator`, `NeighborProvider`, `AddressProvider`, `AddressMutator`, `CapabilityProvider`, синхронные `EventProvider`/bounded `EventReceiver` и опциональная async-поддержка мониторинга в `net-lattice-platform`
- `net-lattice-async`, предоставляющий единый runtime-agnostic тип `EventStream`
- фасад `net-lattice`, включая `Lattice::add_address()`, `Lattice::remove_address()`, `Lattice::set_dns_config()`, `Lattice::capabilities()`, `Lattice::supports()`, `Lattice::watch()`, `Lattice::watch_filtered()`, `Lattice::execute_plan()` и feature-gated `Lattice::watch_async()`

Это даёт реальное управление маршрутами и IP-адресами интерфейсов, просмотр интерфейсов, просмотр и изменение DNS-конфигурации резолвера, чтение таблиц соседей (ARP/NDP), inspectable планы mutation-операций, упорядоченное исполнение транзакций и bounded-мониторинг сетевых изменений на Linux, Windows и macOS. Создание адреса принимает `NewInterfaceAddress` и возвращает результирующий наблюдаемый `InterfaceAddress`; замена конфигурации резолвера принимает `NewDnsConfig` и возвращает результирующий наблюдаемый `DnsConfig`. `MutationPlan` — только данные, а `Lattice::execute_plan` исполняет его через единый `ExecutionOptions` с runtime-проверками, cancellation на границах операций, типизированными snapshots, явной compensation и фазовыми отчётами. `EventFilter` сочетает селекторы доменов (`routes()`) и объектов (`route(route_id)`); каждый backend применяет filter до помещения обычного события в очередь. В переносимом коде перед watching проверяйте `Lattice::supports(Capability::MONITORING)`, а перед заменой DNS-конфигурации — `Lattice::supports(Capability::DNS_MUTATION)`. Unix-менеджеры резолвера могут позднее перегенерировать `/etc/resolv.conf`; при необходимости постоянного состояния используйте конфигурационный интерфейс владеющего менеджера. Feature `async` в Net Lattice использует и реэкспортирует реализацию `EventStream` из `net-lattice-async`; приложению достаточно включить эту feature фасада. `Lattice::watch_async(filter)` остаётся async API Stage 0.11 и имеет ту же семантику filter, что и `Lattice::watch_filtered(filter)`. Tokio используется внутри там, где этого требует нативная реализация, а приложения взаимодействуют только с runtime-independent интерфейсом `futures::Stream`. Платформенные backend'ы используют Netlink в Linux, IP Helper API в Windows и BSD routing sockets, `getifaddrs` и address ioctl в macOS. Это всё ещё не полноценная библиотека: VLAN, VRF, namespaces, интеграция с firewall, декларативная настройка сети и другие продвинутые возможности ещё впереди; см. [ARCHITECTURE.ru.md](ARCHITECTURE.ru.md) для поэтапной дорожной карты и [CHANGELOG.md](CHANGELOG.md) для того, что реально вышло.

| Возможность | Linux | Windows | macOS |
|---|:---:|:---:|:---:|
| Просмотр маршрутов | ✅ | ✅ | ✅ |
| Изменение маршрутов | ✅ | ✅ | ✅ |
| Просмотр интерфейсов | ✅ | ✅ | ✅ |
| Просмотр адресов интерфейсов | ✅ | ✅ | ✅ |
| Изменение адресов интерфейсов | ✅ | ✅ | ✅ |
| Просмотр таблицы соседей | ✅ | ✅ | ✅ |
| Просмотр DNS-резолвера | ✅ | ✅ | ✅ |
| Изменение DNS-резолвера | ✅ | ✅ | ✅ |
| Мониторинг изменений | ✅ | ✅ | ✅ |
| Асинхронный мониторинг изменений | ✅ | ✅ | ✅ |

### Доставка событий

Потоки событий bounded. Если consumer не успевает обрабатывать события, watcher запоминает и выдаёт `Event::ResyncRequired { .. }` перед последующим обычным событием, а не сохраняет неограниченный backlog. Прежде чем полагаться на последующие события, перечитайте состояние затронутого provider.

```rust
let route_events = EventFilter::none().route(route_id);
let watcher = lattice.watch_filtered(route_events)?;
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
| Bounded синхронная доставка | [`sync_monitor`](crates/net-lattice/examples/sync_monitor.rs) | `watch`, `recv_timeout`, `Event::ResyncRequired` |
| Фильтрация доменов и объектов | [`filtered_monitor`](crates/net-lattice/examples/filtered_monitor.rs) | `watch_filtered`, все domain/object selectors `EventFilter` |
| Нативная async-доставка | [`async_monitor`](crates/net-lattice/examples/async_monitor.rs) | `watch_async`, `EventStream` |
| Жизненный цикл адреса | [`address_assignment`](crates/net-lattice/examples/address_assignment.rs) | `NewInterfaceAddress`, `add_address`, `remove_address` |
| Жизненный цикл маршрута | [`route_mutation`](crates/net-lattice/examples/route_mutation.rs) | `Route`, `add_route`, `remove_route` |
| Замена конфигурации резолвера | [`dns_mutation`](crates/net-lattice/examples/dns_mutation.rs) | `NewDnsConfig`, `set_dns_config`, read-after-write verification |
| Просмотр mutation | [`mutation_plan`](crates/net-lattice/examples/mutation_plan.rs) | все варианты `Mutation`, `Mutation::semantics`, `MutationPlan` |

Запуск: `cargo run -p net-lattice --example <name>`. Для `async_monitor`
добавьте `--features async`.

### Просмотр и наблюдение состояния

```rust
use net_lattice::{Lattice, Result};

fn main() -> Result<()> {
    let lattice = Lattice::connect()?;

    for interface in lattice.interfaces()? {
        println!("{interface:?}");
    }

    for route in lattice.routes()? {
        println!("{route:?}");
    }

    let watcher = lattice.watch()?;
    loop {
        let event = watcher.recv()?;
        println!("{event:?}");
    }
}
```

### Асинхронный мониторинг

Включите опциональный async-фасад через `net-lattice = { version = "0.15", features = ["async"] }`. На каждой поддерживаемой платформе он возвращает одинаковый `futures::Stream`:

```rust
use futures::StreamExt;
use net_lattice::{EventFilter, Lattice, Result};

async fn monitor() -> Result<()> {
    let lattice = Lattice::connect()?;
    let mut events = lattice.watch_async(EventFilter::ALL)?;
    while let Some(event) = events.next().await {
        println!("{:?}", event?);
    }
    Ok(())
}
```

### Назначение адреса

Назначение адреса использует тип запроса, поэтому потребитель не конструирует ID наблюдаемого адреса:

```rust
use net_lattice::{
    Error, Ipv4Address, Ipv4Network, Ipv4PrefixLength, Network, NewInterfaceAddress,
};

let interface = lattice
    .interfaces()?
    .into_iter()
    .next()
    .ok_or(Error::NotFound)?;
let request = NewInterfaceAddress::new(
    interface.id,
    Network::from(Ipv4Network::new(
        Ipv4Address::new(192, 0, 2, 10),
        Ipv4PrefixLength::new(24)?,
    )),
);
let observed = lattice.add_address(request)?;
lattice.remove_address(observed)?;
```

### Добавление и удаление маршрута

Изменение маршрута принимает типизированное значение маршрута. Используйте
безопасный для хоста маршрут и удаляйте только тот маршрут, который приложение
успешно создало:

```rust
let route = Route::new(RouteId::new(0), destination)
    .with_interface_index(interface_index);
lattice.add_route(route.clone())?;
lattice.remove_route(route)?;
```

### Замена конфигурации резолвера

Для замены DNS используется desired-state input, а метод возвращает то, что
платформа затем наблюдает. Как правило, требуются права администратора.

```rust
use net_lattice::{IpAddress, Ipv4Address, NewDnsConfig};

let requested = NewDnsConfig::with(
    vec![IpAddress::from(Ipv4Address::new(1, 1, 1, 1))],
    vec!["example.test".to_string()],
);
let observed = lattice.set_dns_config(requested)?;
```

### Просмотр плана mutation

Stage 0.14 также предоставляет типизированные `MutationOutcome`,
`MutationPlanReport` и `RollbackStatus`, которые использует исполнитель Stage
0.15 для отчёта о частичных отказах и границах компенсации. Сам
`MutationPlan` остаётся чистыми данными; исполнение явно выполняется на
границе подключённого `Lattice`, через единый объект `ExecutionOptions`.
Исполнитель не выводит обратные операции или snapshot автоматически.
Вспомогательный метод фасада `snapshot_for_mutation` читает наблюдаемый route,
interface address или DNS view в типизированный `MutationSnapshot`.

В Stage 0.14 планы являются чистыми данными. Они делают существующие
imperative операции inspectable без их применения; Stage 0.15 добавляет
явное исполнение плана и отчёт по каждой операции.

```rust
let plan = MutationPlan::from_operations([
    Mutation::AddAddress(request),
    Mutation::SetDnsConfig(requested_dns),
]);

for operation in plan.operations() {
    println!("{operation:?}: {:?}", operation.semantics());
}

let preflight = plan.preflight();
println!("операции со snapshot: {:?}", preflight.prior_state_indices());
println!(
    "операции с риском partial application: {:?}",
    preflight.partial_application_indices()
);
let mut options = net_lattice::ExecutionOptions::default();
let report = lattice.execute_plan(&plan, &mut options);
```

`MutationPlan::preflight` не имеет side effects. Он сообщает риски,
выведенные из metadata операций; `lattice.validate_plan(&plan)` выполняет
runtime-проверку capabilities до отправки операций. Проверка privileges и
текущего состояния остаётся обязанностью executor.

`MutationPlanReport::outcomes` остаётся стабильной поверхностью результатов;
`operation_reports` добавляет фазу, длительность и причину остановки. Валидация,
захват snapshot, native execution, cancellation и compensation представлены
раздельными фазами.

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
12. **Stage 0.16: Конфигурация интерфейсов** — отдельная desired-конфигурация интерфейса, capability-gated изменение admin state и MTU и platform-parity tests.
13. **Stage 0.17: Изменение соседей** — intent/observed-типы и capability-gated управление статическими ARP/NDP-записями.
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
