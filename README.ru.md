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

> **Статус:** Net Lattice предоставляет кроссплатформенный просмотр сети, изменение маршрутов и адресов, синхронный и опциональный асинхронный мониторинг изменений через нативные API операционных систем в Linux, Windows и macOS. Реализован Stage 0.11 плана архитектуры; см. «Текущий статус» ниже.

## Обзор

Операционные системы предоставляют доступ к сетевой конфигурации и состоянию через совершенно разные, низкоуровневые и зачастую платформо-специфичные интерфейсы: Linux Netlink, Windows IP Helper API, BSD routing facilities в macOS и другие механизмы. Приложениям, которым необходимо анализировать или настраивать сеть — IP-адреса, маршруты, интерфейсы, соседей и многое другое, — как правило, приходится либо вызывать внешние утилиты через shell, либо парсить текстовый вывод, либо писать и поддерживать отдельные платформо-специфичные интеграции.

Net Lattice призвана объединить эти интерфейсы под единым, строго типизированным, идиоматичным API на Rust, чтобы потребителям никогда не приходилось иметь дело с сырыми платформенными структурами, shell-командами или произвольным парсингом строк.

## Мотивация

Кроссплатформенные сетевые инструменты в экосистеме Rust фрагментированы. Существующие решения зачастую платформо-специфичны, неполны или построены на вызове системных утилит, таких как `ip`, `netsh` или `route`. Это хрупко, сложно тестировать и не подходит для создания надёжного, production-grade программного обеспечения для управления сетью.

Net Lattice призвана закрыть этот пробел, предоставив единый, хорошо спроектированный уровень абстракции над нативными сетевыми API операционных систем.

## Философия

- **Строгая типизация вместо строк.** Потребители взаимодействуют с типизированными значениями Rust — адресами, префиксами, маршрутами, интерфейсами — а не с сырыми строками или shell-командами.
- **Нативные API, а не подпроцессы.** Net Lattice обращается напрямую к платформенным сетевым API (Netlink, IP Helper API, route sockets), а не вызывает внешние CLI-инструменты.
- **Кроссплатформенность по замыслу.** Единая поверхность API с платформо-специфичными реализациями, чтобы приложениям не приходилось делать особые случаи для каждой операционной системы.
- **Корректность и безопасность прежде всего.** Настройка сети — чувствительная область; библиотека должна затруднять представление некорректных состояний.
- **Постепенный, продуманный рост.** Функциональность добавляется осознанно, с вниманием к дизайну API и долгосрочной поддерживаемости, а не поспешно, чтобы покрыть все мыслимые сценарии использования.

## Дорожная карта возможностей

Уже реализовано:

- IPv4/IPv6-адреса и префиксы
- Просмотр и изменение маршрутов
- Просмотр интерфейсов
- Просмотр конфигурации DNS-резолвера
- Таблицы соседей (ARP/NDP)
- Мониторинг сети и уведомления об изменениях
- Опциональный runtime-agnostic async stream событий

Запланировано:

- Изменение DNS
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

Реализован этап 0.11 плана поэтапной поставки из [архитектуры](ARCHITECTURE.ru.md):

- `net-lattice-core`, `net-lattice-ip`
- модули `route`, `mac`, `interface`, `dns`, `neighbor`, `ifaddr` и `event` в `net-lattice-model`; `NewInterfaceAddress` выражает намерение назначить адрес отдельно от наблюдаемого `InterfaceAddress`
- `RouteProvider`, `InterfaceProvider`, `DnsProvider`, `NeighborProvider`, `AddressProvider`, `AddressMutator`, `CapabilityProvider`, синхронные `EventProvider`/bounded `EventReceiver` и feature-gated `TokioEventProvider` в `net-lattice-platform`
- `net-lattice-backend-linux` (маршруты, интерфейсы, соседи, чтение и изменение адресов и мониторинг через Netlink; DNS через `/etc/resolv.conf`)
- `net-lattice-backend-windows` (маршруты и интерфейсы через Windows IP Helper API, DNS через `GetAdaptersAddresses`, соседи через `GetIpNetTable2`, чтение и изменение адресов через unicast-address API IP Helper, мониторинг через уведомления IP Helper)
- `net-lattice-backend-darwin` (маршруты, соседи и мониторинг через BSD routing facilities в macOS; интерфейсы и чтение адресов через `getifaddrs`; изменение адресов через нативные address ioctl; DNS через `/etc/resolv.conf`)
- `net-lattice-async`, предоставляющий единый runtime-agnostic тип `EventStream`
- фасад `net-lattice`, включая `Lattice::add_address()`, `Lattice::remove_address()`, `Lattice::capabilities()`, `Lattice::supports()`, `Lattice::watch()` и feature-gated `Lattice::watch_async()`

Это даёт реальное управление маршрутами и IP-адресами интерфейсов, просмотр интерфейсов, чтение DNS-конфигурации резолвера, чтение таблиц соседей (ARP/NDP) и bounded-мониторинг сетевых изменений на Linux, Windows и macOS. Создание адреса принимает `NewInterfaceAddress` и возвращает результирующий наблюдаемый `InterfaceAddress`, поэтому потребитель не конструирует ID адреса самостоятельно. `Lattice::watch_filtered(EventFilter::none().routes())` ограничивает доставляемые домены; при `Event::ResyncRequired` перечитайте затронутое состояние, потому что медленный consumer переполнил bounded-очередь. В переносимом коде перед watching проверяйте `Lattice::supports(Capability::MONITORING)`. С опциональной feature `async` `Lattice::watch_async(filter)` возвращает одинаковый `EventStream` на всех платформах: Linux читает Netlink через свой Tokio reactor, Windows пишет нативные callbacks IP Helper в Tokio transport, а macOS соединяет свой нативный PF_ROUTE reader с этим transport. Stream реализует `futures::Stream` и не выбирает executor приложения. Это всё ещё не полноценная библиотека: изменение DNS, VLAN, VRF, namespaces, интеграция с firewall, транзакционная конфигурация, декларативная настройка сети и другие продвинутые возможности ещё впереди; см. [ARCHITECTURE.ru.md](ARCHITECTURE.ru.md) для поэтапной дорожной карты и [CHANGELOG.md](CHANGELOG.md) для того, что реально вышло.

## Краткий пример

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

Включите опциональный async-фасад через `net-lattice = { version = "0.11", features = ["async"] }`. На каждой поддерживаемой платформе он возвращает одинаковый `futures::Stream`:

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

## Дорожная карта

1. **Bootstrap** *(завершён)* — инфраструктура репозитория, лицензирование, файлы для сообщества и настройка инструментов.
2. **Проектирование** *(завершено)* — структура крейтов, базовые абстракции и стратегия абстрагирования платформ реализованы на этапе 0.1. См. [ARCHITECTURE.ru.md](ARCHITECTURE.ru.md).
3. **Фундамент** *(завершено)* — реализованы базовые типы IP/маршрутов/интерфейсов и все три платформенных бэкенда.
4. **Паритет платформ** *(завершён)* — реализованы Linux/Windows/macOS backend'ы для изменения маршрутов и адресов, интерфейсов, чтения DNS, чтения соседей, чтения адресов и мониторинга.
5. **Stage 0.10: Семантика событий** *(завершён)* — bounded delivery, сигнализация overflow и resynchronization, filtering, cancellation и распространение ошибок.
6. **Stage 0.11: Async events** *(завершён)* — опциональная feature фасада `async`, единый runtime-agnostic `EventStream` и нативная Tokio-backed доставка в каждом платформенном backend.
7. **Поздние этапы** — дальнейший паритет операций записи, транзакционная конфигурация и декларативная настройка сети.

## Участие в проекте

Вклад в проект приветствуется. См. [CONTRIBUTING.md](CONTRIBUTING.md) для рекомендаций, [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md) для правил поведения в сообществе и [SECURITY.md](SECURITY.md) для сообщения о проблемах безопасности.

## Лицензия

Net Lattice распространяется под лицензией [Mozilla Public License 2.0](LICENSE) (`MPL-2.0`).
