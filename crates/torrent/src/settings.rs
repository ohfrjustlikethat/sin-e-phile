//! The torrent settings a user can change, and the adapters they can bind to.
//!
//! Stored as one JSON value under [`SETTINGS_KEY`] in the global `settings` table:
//! torrent behaviour is a property of the machine, not of a profile.

use serde::{Deserialize, Serialize};

use crate::engine::{EngineConfig, SeedingPolicy};

pub const SETTINGS_KEY: &str = "torrent.settings";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct TorrentSettings {
    /// KiB per second; `None` is unlimited.
    pub download_limit_kib: Option<u32>,
    pub upload_limit_kib: Option<u32>,
    pub peer_limit: u32,
    /// The default is `false`: seeding stops at completion (SPEC.md Phase 7).
    pub keep_seeding: bool,
    /// An adapter's friendly name. `None` uses whatever route the system picks.
    pub bind_interface: Option<String>,
}

impl Default for TorrentSettings {
    fn default() -> Self {
        Self {
            download_limit_kib: None,
            upload_limit_kib: None,
            peer_limit: 128,
            keep_seeding: false,
            bind_interface: None,
        }
    }
}

impl TorrentSettings {
    /// Read stored settings; anything unreadable falls back to the defaults rather than
    /// stopping the engine starting.
    pub fn parse(stored: Option<&str>) -> Self {
        stored
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_default()
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("settings always serialise")
    }

    /// Bounds a settings screen cannot be trusted to enforce.
    pub fn validate(&self) -> Result<(), String> {
        if !(8..=2000).contains(&self.peer_limit) {
            return Err("peers per torrent must be between 8 and 2000".into());
        }
        for (name, v) in [
            ("download", self.download_limit_kib),
            ("upload", self.upload_limit_kib),
        ] {
            if v == Some(0) {
                return Err(format!(
                    "a {name} limit of 0 would stop all {name}s; leave it empty for no limit"
                ));
            }
        }
        if let Some(name) = &self.bind_interface {
            if name.trim().is_empty() || name.len() > 256 {
                return Err("that adapter name is not valid".into());
            }
        }
        Ok(())
    }

    pub fn apply(&self, mut config: EngineConfig) -> EngineConfig {
        config.download_limit = self.download_limit_kib.map(|k| k.saturating_mul(1024));
        config.upload_limit = self.upload_limit_kib.map(|k| k.saturating_mul(1024));
        config.peer_limit = Some(self.peer_limit as usize);
        config.seeding = if self.keep_seeding {
            SeedingPolicy::KeepSeeding
        } else {
            SeedingPolicy::StopAtCompletion
        };
        config.bind_interface = self.bind_interface.clone();
        config
    }
}

/// A network adapter the engine can be bound to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Adapter {
    pub name: String,
    /// Has a routable address — a disconnected adapter has only link-local ones.
    pub connected: bool,
}

/// Every adapter Windows reports, loopback excluded, connected first.
pub fn adapters() -> Vec<Adapter> {
    use network_interface::{NetworkInterface, NetworkInterfaceConfig};
    let Ok(nics) = NetworkInterface::show() else {
        return Vec::new();
    };
    let mut out: Vec<Adapter> = nics
        .into_iter()
        .filter(|n| !n.addr.iter().any(|a| a.ip().is_loopback()))
        .map(|n| Adapter {
            connected: n.addr.iter().any(|a| match a.ip() {
                std::net::IpAddr::V4(ip) => !ip.is_link_local() && !ip.is_unspecified(),
                std::net::IpAddr::V6(ip) => {
                    (ip.segments()[0] & 0xffc0) != 0xfe80 && !ip.is_unspecified()
                }
            }),
            name: n.name,
        })
        .collect();
    out.sort_by(|a, b| b.connected.cmp(&a.connected).then(a.name.cmp(&b.name)));
    out.dedup_by(|a, b| a.name == b.name);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stored_settings_round_trip_and_garbage_falls_back() {
        let s = TorrentSettings {
            download_limit_kib: Some(4096),
            keep_seeding: true,
            bind_interface: Some("VPN".into()),
            ..Default::default()
        };
        assert_eq!(TorrentSettings::parse(Some(&s.to_json())), s);
        assert_eq!(
            TorrentSettings::parse(Some("{not json")),
            TorrentSettings::default()
        );
        assert_eq!(TorrentSettings::parse(None), TorrentSettings::default());
        // A field added later reads with its default, not an error.
        assert_eq!(
            TorrentSettings::parse(Some("{}")),
            TorrentSettings::default()
        );
    }

    #[test]
    fn the_defaults_are_the_spec_and_apply_maps_units() {
        let d = TorrentSettings::default();
        assert!(!d.keep_seeding, "seeding stops at completion by default");
        let c = TorrentSettings {
            upload_limit_kib: Some(100),
            ..d
        }
        .apply(EngineConfig::new("x"));
        assert_eq!(c.upload_limit, Some(102_400));
        assert_eq!(c.seeding, SeedingPolicy::StopAtCompletion);
    }

    #[test]
    fn nonsense_is_refused_with_a_reason() {
        let zero = TorrentSettings {
            download_limit_kib: Some(0),
            ..Default::default()
        };
        assert!(zero.validate().unwrap_err().contains("leave it empty"));
        let few = TorrentSettings {
            peer_limit: 1,
            ..Default::default()
        };
        assert!(few.validate().is_err());
        assert!(TorrentSettings::default().validate().is_ok());
    }
}
