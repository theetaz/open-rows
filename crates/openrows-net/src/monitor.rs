// SPDX-License-Identifier: Apache-2.0
//! Path monitor (docs/24 §6): notices system sleep/wake and network changes
//! without OS hooks, so open sessions and tunnels can be probed at once
//! instead of after a TCP timeout that can take minutes.
//!
//! - **Wake**: the monotonic clock ([`Instant`]) stops while the machine
//!   sleeps (macOS `mach_absolute_time`, Linux `CLOCK_MONOTONIC`) but the
//!   wall clock does not, so a sample whose wall time advanced well past its
//!   monotonic time means the machine slept in between.
//! - **Network change**: the local address the OS would use for the default
//!   IPv4 and IPv6 route (an unconnected UDP socket "connected" to a
//!   documentation address; no packet is sent) changes when Wi-Fi, a cable
//!   or a full-tunnel VPN comes or goes.

use std::{
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, UdpSocket},
    time::{Duration, Instant, SystemTime},
};

/// Wall time that must pass beyond monotonic time to count as a sleep.
const SLEEP_THRESHOLD: Duration = Duration::from_secs(4);
/// Documentation addresses (RFC 5737, RFC 3849): only routed, never reached.
const ROUTE_PROBES: [SocketAddr; 2] = [
    SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 0, 2, 1)), 9),
    SocketAddr::new(
        IpAddr::V6(Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 1)),
        9,
    ),
];

/// What changed between two samples.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PathChange {
    /// The machine slept for about this long and woke up.
    Woke {
        /// Wall time not covered by the monotonic clock.
        asleep: Duration,
    },
    /// The local address of the default route changed (or appeared or vanished).
    RouteChanged,
}

/// Clocks and default-route addresses at one moment.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PathSample {
    wall: SystemTime,
    monotonic: Instant,
    routes: [Option<IpAddr>; 2],
}

impl PathSample {
    /// Read both clocks and the default-route source addresses now.
    #[must_use]
    pub fn now() -> Self {
        Self {
            wall: SystemTime::now(),
            monotonic: Instant::now(),
            routes: ROUTE_PROBES.map(route_source),
        }
    }

    /// What changed since `earlier`; a wake wins over a route change, as
    /// both lead to the same probing.
    #[must_use]
    pub fn change_since(&self, earlier: &Self) -> Option<PathChange> {
        let monotonic = self.monotonic.saturating_duration_since(earlier.monotonic);
        // A wall clock set backwards is not a sleep.
        let wall = self.wall.duration_since(earlier.wall).unwrap_or_default();
        let asleep = wall.saturating_sub(monotonic);
        if asleep >= SLEEP_THRESHOLD {
            return Some(PathChange::Woke { asleep });
        }
        (self.routes != earlier.routes).then_some(PathChange::RouteChanged)
    }
}

/// The local address the OS picks to reach `probe`; `None` without a route.
fn route_source(probe: SocketAddr) -> Option<IpAddr> {
    let any: SocketAddr = match probe {
        SocketAddr::V4(_) => (Ipv4Addr::UNSPECIFIED, 0).into(),
        SocketAddr::V6(_) => (Ipv6Addr::UNSPECIFIED, 0).into(),
    };
    let socket = UdpSocket::bind(any).ok()?;
    socket.connect(probe).ok()?;
    socket.local_addr().ok().map(|address| address.ip())
}

/// Sample every `tick` and call `on_change` for each change; runs until
/// the future is dropped (spawn it on the caller's runtime).
pub async fn watch_path<F>(tick: Duration, on_change: F)
where
    F: Fn(PathChange) + Send,
{
    let mut interval = tokio::time::interval(tick);
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut last = PathSample::now();
    loop {
        interval.tick().await;
        let sample = PathSample::now();
        if let Some(change) = sample.change_since(&last) {
            on_change(change);
        }
        last = sample;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(wall: SystemTime, monotonic: Instant, v4: Option<IpAddr>) -> PathSample {
        PathSample {
            wall,
            monotonic,
            routes: [v4, None],
        }
    }

    #[test]
    fn a_wall_clock_gap_beyond_monotonic_time_is_a_wake() {
        let (wall, monotonic) = (SystemTime::now(), Instant::now());
        let before = sample(wall, monotonic, None);
        let slept = sample(
            wall + Duration::from_secs(62),
            monotonic + Duration::from_secs(2),
            None,
        );
        assert_eq!(
            slept.change_since(&before),
            Some(PathChange::Woke {
                asleep: Duration::from_secs(60)
            })
        );
        let ticked = sample(
            wall + Duration::from_secs(3),
            monotonic + Duration::from_secs(2),
            None,
        );
        assert_eq!(ticked.change_since(&before), None);
    }

    #[test]
    fn a_clock_set_backwards_is_not_a_wake() {
        let (wall, monotonic) = (SystemTime::now(), Instant::now());
        let before = sample(wall + Duration::from_secs(3600), monotonic, None);
        let after = sample(wall, monotonic + Duration::from_secs(2), None);
        assert_eq!(after.change_since(&before), None);
    }

    #[test]
    fn a_new_default_route_address_is_a_network_change() {
        let (wall, monotonic) = (SystemTime::now(), Instant::now());
        let wifi = Some(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 20)));
        let vpn = Some(IpAddr::V4(Ipv4Addr::new(10, 8, 0, 2)));
        let before = sample(wall, monotonic, wifi);
        let later = |route| {
            sample(
                wall + Duration::from_secs(2),
                monotonic + Duration::from_secs(2),
                route,
            )
        };
        assert_eq!(later(wifi).change_since(&before), None);
        assert_eq!(
            later(vpn).change_since(&before),
            Some(PathChange::RouteChanged)
        );
        assert_eq!(
            later(None).change_since(&before),
            Some(PathChange::RouteChanged)
        );
    }

    #[test]
    fn sampling_the_real_path_never_panics() {
        let first = PathSample::now();
        let second = PathSample::now();
        assert_eq!(second.change_since(&first), None);
    }
}
