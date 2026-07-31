//! `ticker` — a rustline widget showing a coin price via CoinGecko's keyless
//! simple-price API, fetched through the host's TTL-cached HTTP capability.
//!
//! Pure logic lives here and is unit-tested on the host target; the Extism
//! guest glue below only compiles for wasm32.

use serde::Deserialize;

/// This plugin's `[plugins.ticker.options]` table.
#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct Options {
    /// A CoinGecko coin id (lowercase slug), e.g. "bitcoin", "ethereum".
    pub coin: String,
    /// A CoinGecko vs-currency (lowercase slug), e.g. "usd", "eur".
    pub currency: String,
    /// Cache TTL in seconds (the host owns the cache).
    pub refresh_secs: i64,
    /// Render format. `{price}`, `{coin}`, `{currency}` substitute.
    pub format: String,
    /// Click-toggle alternate view.
    pub alt_format: String,
    /// Shown when no usable answer exists. Empty renders nothing.
    pub down_format: String,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            coin: "bitcoin".to_string(),
            currency: "usd".to_string(),
            refresh_secs: 300,
            format: "\u{f0813} {price}".to_string(), // nf-md-bitcoin + space
            alt_format: String::new(),
            down_format: String::new(),
        }
    }
}

/// CoinGecko ids and currencies are lowercase slugs. Refusing anything else
/// keeps config text from smuggling arbitrary content into the fetched URL.
pub fn valid_slug(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// The simple-price endpoint for one coin/currency pair.
pub fn price_url(coin: &str, currency: &str) -> String {
    format!("https://api.coingecko.com/api/v3/simple/price?ids={coin}&vs_currencies={currency}")
}

/// Pull the price out of CoinGecko's `{"bitcoin":{"usd":118000.5}}` shape.
pub fn extract_price(body: &str, coin: &str, currency: &str) -> Option<f64> {
    let v: serde_json::Value = serde_json::from_str(body).ok()?;
    v.get(coin)?.get(currency)?.as_f64()
}

/// Human price formatting: values at or above 1000 round to a
/// thousands-separated integer; at or above 1 keep two decimals; at or above
/// 0.0001 keep four decimals; below that eight (small coins would otherwise
/// render as 0.0000).
pub fn format_price(p: f64) -> String {
    if p >= 1000.0 {
        let s = format!("{:.0}", p.round());
        let mut out = String::new();
        for (i, c) in s.chars().enumerate() {
            if i > 0 && (s.len() - i) % 3 == 0 {
                out.push(',');
            }
            out.push(c);
        }
        out
    } else if p >= 1.0 {
        format!("{p:.2}")
    } else if p >= 0.0001 {
        format!("{p:.4}")
    } else {
        format!("{p:.8}")
    }
}

/// Substitute `{price}`/`{coin}`/`{currency}`; unknown placeholders pass
/// through untouched.
pub fn render_format(format: &str, price: &str, coin: &str, currency: &str) -> String {
    format
        .replace("{price}", price)
        .replace("{coin}", coin)
        .replace("{currency}", currency)
}

/// The capability manifest, embedded as the `rustline-manifest` custom
/// section so `rustline plugin approve ticker` can read the requests straight
/// out of the distributed `.wasm` — `plugin install` downloads only this
/// binary, so a sidecar file would never reach the user's plugin dir.
#[cfg(target_arch = "wasm32")]
#[used]
#[unsafe(link_section = "rustline-manifest")]
pub static MANIFEST: [u8; include_bytes!("../manifest.toml").len()] =
    *include_bytes!("../manifest.toml");

#[cfg(target_arch = "wasm32")]
mod guest {
    use super::*;
    use rustline_plugin_sdk::{
        GuestRender, LogLevel, Segment, active_format, export_plugin, http_get_cached, log,
    };

    fn render(input: &GuestRender) -> Vec<Segment> {
        let opts: Options = serde_json::from_value(input.config.clone()).unwrap_or_default();
        if !valid_slug(&opts.coin) || !valid_slug(&opts.currency) {
            log(
                LogLevel::Warn,
                &format!(
                    "ticker: invalid coin/currency slug {:?}/{:?}",
                    opts.coin, opts.currency
                ),
            );
            return down_segment(&opts.down_format);
        }
        let format = active_format(&input.context, "ticker", &opts.format, &opts.alt_format);
        let url = price_url(&opts.coin, &opts.currency);

        match http_get_cached(&url, opts.refresh_secs, &input.context.now) {
            Ok(r) if r.ok => match extract_price(&r.body, &opts.coin, &opts.currency) {
                Some(p) => {
                    let price = format_price(p);
                    vec![Segment::new(render_format(
                        format,
                        &price,
                        &opts.coin,
                        &opts.currency,
                    ))]
                }
                None => {
                    log(
                        LogLevel::Warn,
                        &format!(
                            "ticker: no {}/{} price in the response",
                            opts.coin, opts.currency
                        ),
                    );
                    down_segment(&opts.down_format)
                }
            },
            Ok(r) => {
                log(
                    LogLevel::Warn,
                    &format!("ticker: fetch failed: {}", r.error),
                );
                down_segment(&opts.down_format)
            }
            Err(e) => {
                log(LogLevel::Warn, &format!("ticker: host call failed: {e}"));
                down_segment(&opts.down_format)
            }
        }
    }

    fn down_segment(down_format: &str) -> Vec<Segment> {
        if down_format.is_empty() {
            Vec::new()
        } else {
            vec![Segment::new(down_format.to_string())]
        }
    }

    export_plugin!(name: "ticker", render: render);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_validation() {
        assert!(valid_slug("bitcoin"));
        assert!(valid_slug("matic-network"));
        assert!(!valid_slug(""));
        assert!(!valid_slug("Bitcoin"));
        assert!(!valid_slug("btc&x=1"));
    }

    #[test]
    fn price_url_shape() {
        assert_eq!(
            price_url("bitcoin", "usd"),
            "https://api.coingecko.com/api/v3/simple/price?ids=bitcoin&vs_currencies=usd"
        );
    }

    #[test]
    fn extracts_price_from_coingecko_shape() {
        let body = r#"{"bitcoin":{"usd":118000.5}}"#;
        assert_eq!(extract_price(body, "bitcoin", "usd"), Some(118000.5));
        assert_eq!(extract_price(body, "ethereum", "usd"), None);
        assert_eq!(extract_price("not json", "bitcoin", "usd"), None);
    }

    #[test]
    fn price_formatting_buckets() {
        assert_eq!(format_price(118000.5), "118,001");
        assert_eq!(format_price(1234567.0), "1,234,567");
        assert_eq!(format_price(999.994), "999.99");
        assert_eq!(format_price(2.5), "2.50");
        assert_eq!(format_price(0.0834), "0.0834");
        assert_eq!(format_price(0.00001234), "0.00001234");
    }

    #[test]
    fn render_format_substitutes_all() {
        assert_eq!(
            render_format("{coin} {price} {currency}", "1,000", "bitcoin", "usd"),
            "bitcoin 1,000 usd"
        );
    }
}
