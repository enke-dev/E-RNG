<p align="center">
  <img src="docs/icon.png" alt="E-RNG logo" width="72">
</p>

# E-RNG

macOS app for e-invoices: fill in an invoice, get a **ZUGFeRD / Factur-X PDF** (or an **XRechnung**
XML) that was checked before it can be saved.

## Usage

Download the disc image from the [latest release](https://github.com/enke-dev/e-rng/releases/latest)
(Apple silicon; not signed yet, so open it via right click).

## Development

Needs [Bun](https://bun.sh/docs/installation) and [Rust](https://rust-lang.org/tools/install/):

```sh
bun install
bun run generate   # icons and the bundled tools (Typst, Mustang, a Java runtime)
bun run dev
```

More in [docs/development.md](docs/development.md), licences in [NOTICE.md](NOTICE.md).
