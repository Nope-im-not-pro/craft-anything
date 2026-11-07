//! Where a backend's model runs, which decides its context budget, its tools and its cost.
//!
//! "On this computer" is not the same as "a local model": a gateway such as 9router listens on
//! this computer but passes the conversation on to cloud models, which have a large context and a
//! bill. Only a model that runs here or on this network (Ollama, LM Studio…) is a local model.

use crate::BackendKind;
use crate::config::BackendEntry;
use serde::{Deserialize, Serialize};

/// Where a backend's model runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Locality {
    /// A cloud API: large context, all tools, its list price.
    Hosted,
    /// A program on this computer or network that passes requests on to cloud models: large
    /// context, all tools, a cost that is unknown unless a price is set for the full model id.
    Gateway,
    /// A model running on this computer or network: small context, lean tools, no cost.
    LocalModel,
}

impl Locality {
    pub const ALL: [Locality; 3] = [Locality::Hosted, Locality::Gateway, Locality::LocalModel];

    /// The stable lowercase id used in settings and on the command line.
    pub fn as_str(self) -> &'static str {
        match self {
            Locality::Hosted => "hosted",
            Locality::Gateway => "gateway",
            Locality::LocalModel => "local_model",
        }
    }

    pub fn parse(s: &str) -> Option<Locality> {
        Locality::ALL.into_iter().find(|l| l.as_str() == s.trim())
    }
}

/// A gateway recognised by the port it listens on, on this computer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KnownGateway {
    pub name: &'static str,
    pub port: u16,
}

/// Gateways found by their usual port on a loopback address.
pub const GATEWAYS: &[KnownGateway] = &[KnownGateway { name: "9router", port: 20128 }];

/// The [`GATEWAYS`] entry `base` points at: a loopback address on a gateway's port.
pub fn known_gateway(base: &str) -> Option<&'static KnownGateway> {
    let rest = base.trim().strip_prefix("http://").or_else(|| base.trim().strip_prefix("https://"))?;
    let hostport = rest.split('/').next().unwrap_or_default();
    let (host, port) = match hostport.strip_prefix('[') {
        Some(v6) => {
            let (host, after) = v6.split_once(']')?;
            (host, after.strip_prefix(':')?)
        }
        None => hostport.rsplit_once(':')?,
    };
    let port: u16 = port.parse().ok()?;
    let loopback = host.eq_ignore_ascii_case("localhost") || host.parse::<std::net::IpAddr>().is_ok_and(|ip| ip.is_loopback());
    if !loopback {
        return None;
    }
    GATEWAYS.iter().find(|g| g.port == port)
}

/// Where a backend's model runs: the `locality` it was given, else a known gateway, else an
/// OpenAI-compatible server on this computer or its network (or reached over ssh) is a local
/// model, and everything else is hosted (#374).
pub fn of_entry(e: &BackendEntry) -> Locality {
    if let Some(l) = e.options.locality {
        return l;
    }
    let c = &e.config;
    if c.kind != BackendKind::OpenaiCompatible {
        return Locality::Hosted;
    }
    let base = c.base_url.as_deref().map(str::trim).filter(|b| !b.is_empty());
    // A gateway reached over ssh is named by its address on the far computer, as here.
    if base.and_then(known_gateway).is_some() {
        return Locality::Gateway;
    }
    let over_ssh = c.ssh_host.as_deref().is_some_and(|h| !h.trim().is_empty());
    if over_ssh || base.is_none_or(crate::detect::on_this_network) { Locality::LocalModel } else { Locality::Hosted }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gateways_are_found_by_loopback_port_only() {
        for base in ["http://127.0.0.1:20128/v1", "http://localhost:20128", "http://[::1]:20128/v1", "https://127.0.0.2:20128/v1/"] {
            assert_eq!(known_gateway(base).map(|g| g.name), Some("9router"), "{base}");
        }
        for base in
            ["http://192.168.1.20:20128/v1", "http://127.0.0.1:11434/v1", "http://127.0.0.1/v1", "https://router.test:20128/v1", "20128", "http://[::1/v1"]
        {
            assert!(known_gateway(base).is_none(), "{base}");
        }
    }

    #[test]
    fn localities_round_trip_through_their_ids() {
        for l in Locality::ALL {
            assert_eq!(Locality::parse(l.as_str()), Some(l));
            assert_eq!(serde_json::to_value(l).unwrap(), serde_json::json!(l.as_str()));
        }
        assert_eq!(Locality::parse("local"), None);
    }
}
