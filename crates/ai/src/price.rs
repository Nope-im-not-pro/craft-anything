//! The cost meter: USD per million tokens by model, overridable in settings.

use crate::config::Price;
use crate::locality::Locality;
use crate::model::Usage;
use std::collections::BTreeMap;

const fn p(input: f64, output: f64, cache_read: f64) -> Price {
    Price { input_per_mtok: input, output_per_mtok: output, cache_read_per_mtok: Some(cache_read), cache_write_per_mtok: None }
}

/// Built-in list prices (USD per million tokens, first-party API rates, checked 2026-10).
pub fn builtin(model: &str) -> Option<Price> {
    let m = model.rsplit('/').next().unwrap_or(model);
    Some(match m {
        "claude-opus-5-5" => p(4.0, 20.0, 0.20),
        // Claude 5.5 cache reads are 0.05× input: Sonnet 5.5 $0.10 (platform.claude.com pricing
        // and prompt-caching pages, checked 2026-11-07).
        "claude-sonnet-5-5" => p(2.0, 10.0, 0.10),
        // Haiku 5.5's rate up to 100k input tokens; above that it is 0.50 / 2.50 / 0.05, which a
        // single-tier price can't hold yet (R2.11, prices by provider and model).
        "claude-haiku-5-5" => p(0.10, 0.50, 0.01),
        "claude-fable-5-1" => p(10.0, 50.0, 0.25),
        "claude-opus-5" => p(5.0, 25.0, 0.50),
        "claude-sonnet-5" => p(2.0, 10.0, 0.20),
        // OpenRouter's list price (checked 2026-11-03).
        "kimi-k3.1-pro" => p(2.0, 10.0, 0.20),
        _ => return None,
    })
}

/// Cost of one turn's usage, or `None` when the price is unknown. Local models cost nothing. A
/// gateway is priced only by a setting for its full model id (`cc/claude-opus-5-5`): what it
/// charges is its own business, and the first-party price of the name after `/` would be a guess.
pub fn cost(model: &str, usage: &Usage, overrides: &BTreeMap<String, Price>, locality: Locality) -> Option<f64> {
    let price = match locality {
        Locality::LocalModel => return Some(0.0),
        Locality::Gateway => overrides.get(model).copied()?,
        Locality::Hosted => overrides.get(model).copied().or_else(|| builtin(model))?,
    };
    let per = |tokens: u64, rate: f64| tokens as f64 * rate / 1_000_000.0;
    let read = price.cache_read_per_mtok.unwrap_or(price.input_per_mtok * 0.1);
    let write = price.cache_write_per_mtok.unwrap_or(price.input_per_mtok * 1.25);
    Some(
        per(usage.input_tokens, price.input_per_mtok)
            + per(usage.output_tokens, price.output_per_mtok)
            + per(usage.cache_read_tokens, read)
            + per(usage.cache_write_tokens, write),
    )
}

/// Running totals for a conversation.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Meter {
    pub usage: Usage,
    pub cost_usd: f64,
    /// False once any turn had an unknown price.
    pub cost_known: bool,
    turns: u64,
}

impl Meter {
    pub fn add(&mut self, usage: Usage, cost: Option<f64>) {
        if self.turns == 0 {
            self.cost_known = true;
        }
        self.turns += 1;
        self.usage.add(usage);
        match cost {
            Some(c) => self.cost_usd += c,
            None => self.cost_known = false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn price_math() {
        let u = Usage { input_tokens: 1_000_000, output_tokens: 100_000, cache_read_tokens: 1_000_000, cache_write_tokens: 0 };
        let c = cost("claude-opus-5-5", &u, &BTreeMap::new(), Locality::Hosted).unwrap();
        assert!((c - (4.0 + 2.0 + 0.2)).abs() < 1e-9, "{c}");
        assert_eq!(
            cost("anthropic/claude-sonnet-5-5", &Usage { output_tokens: 1_000_000, ..Default::default() }, &BTreeMap::new(), Locality::Hosted),
            Some(10.0)
        );
        assert_eq!(cost("mystery", &u, &BTreeMap::new(), Locality::Hosted), None);
        // Sonnet 5.5 cache reads are $0.10 per million, not Opus's $0.20.
        let read = Usage { cache_read_tokens: 1_000_000, ..Default::default() };
        let s = cost("claude-sonnet-5-5", &read, &BTreeMap::new(), Locality::Hosted).unwrap();
        assert!((s - 0.10).abs() < 1e-9, "{s}");
        // Kimi K3.1 Pro through OpenRouter is priced by its model id.
        let k = cost(
            "moonshotai/kimi-k3.1-pro",
            &Usage { input_tokens: 1_000_000, output_tokens: 1_000_000, ..Default::default() },
            &BTreeMap::new(),
            Locality::Hosted,
        )
        .unwrap();
        assert!((k - (2.0 + 10.0)).abs() < 1e-9, "{k}");
        assert_eq!(cost("mystery", &u, &BTreeMap::new(), Locality::LocalModel), Some(0.0));
        let mut o = BTreeMap::new();
        o.insert("mystery".into(), Price { input_per_mtok: 1.0, output_per_mtok: 1.0, cache_read_per_mtok: None, cache_write_per_mtok: None });
        let w = Usage { cache_write_tokens: 1_000_000, ..Default::default() };
        assert_eq!(cost("mystery", &w, &o, Locality::Hosted), Some(1.25));
        // A gateway never gets the first-party price of the name after `/`, only its own setting.
        let opus = Usage { output_tokens: 1_000_000, ..Default::default() };
        assert_eq!(cost("cc/claude-opus-5-5", &opus, &BTreeMap::new(), Locality::Gateway), None);
        assert_eq!(cost("claude-opus-5-5", &opus, &BTreeMap::new(), Locality::Gateway), None);
        o.insert("cc/claude-opus-5-5".into(), Price { input_per_mtok: 0.0, output_per_mtok: 3.0, cache_read_per_mtok: None, cache_write_per_mtok: None });
        assert_eq!(cost("cc/claude-opus-5-5", &opus, &o, Locality::Gateway), Some(3.0));
        let mut m = Meter::default();
        m.add(u, Some(1.0));
        m.add(u, None);
        assert!(!m.cost_known);
        assert_eq!(m.usage.input_tokens, 2_000_000);
    }
}
