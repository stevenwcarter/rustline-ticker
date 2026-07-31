# rustline-ticker

A coin price in your tmux status line, as a
[rustline](https://github.com/stevenwcarter/rustline) WASM plugin.

Fetches a configured CoinGecko coin/currency pair from CoinGecko's keyless
`simple/price` API through rustline's TTL-cached, capability-gated HTTP host
function: the network is hit at most once per `refresh_secs`, a stale answer
is served while the endpoint is unreachable, and the plugin can only reach
URLs you explicitly allow.

## Install

    rustline plugin install stevenwcarter/rustline-ticker --name ticker
    rustline plugin approve ticker        # grants: https://api.coingecko.com/api/v3/simple/price*

(`--name ticker` is only needed on rustline v0.1.0, whose default install
name came from the repo; newer builds derive it from the release asset.)

Place it:

    rustline widget enable ticker

## Configure

    [plugins.ticker.options]
    # coin = "bitcoin"        # any CoinGecko coin id
    # currency = "usd"
    # refresh_secs = 300
    # format = "󰠓 {price}"    # {price} {coin} {currency} substitute
    # alt_format = ""
    # down_format = ""

`coin`/`currency` must be lowercase CoinGecko slugs (`[a-z0-9-]`); anything
else is refused rather than reaching the fetched URL, and the widget renders
`down_format`.

## Building from source

    cargo build --release --target wasm32-unknown-unknown
    cp target/wasm32-unknown-unknown/release/ticker.wasm ~/.local/share/rustline/plugins/

The capability manifest (`manifest.toml`) is embedded into the `.wasm` as the
`rustline-manifest` custom section at build time, so `plugin approve` works
on the distributed binary alone. This repo doubles as a worked example of a
standalone (out-of-tree) rustline plugin: scaffolded with `rustline plugin
new`, SDK via a git dependency pinned to a rustline release tag, CI + tag
releases publishing the installable `.wasm`.

## License

MIT
