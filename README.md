<p align="center">
  <img src="docs/icon.png" alt="" width="72">
</p>

# E-Rng

macOS app for e-invoices: fill in an invoice, get a **ZUGFeRD / Factur-X PDF** (or an **XRechnung**
XML) that was checked before it can be saved.

&nbsp;

**Use** – download the dmg from the [latest release](https://github.com/enke-dev/e-rng/releases/latest)
(Apple silicon; not signed yet, so open it via right click).

**Develop** – needs [Bun](https://bun.sh) and Rust:

```sh
bun install
bun run generate   # icons and the bundled tools (Typst, Mustang, a Java runtime)
bun run dev
```

More in [docs/development.md](docs/development.md), licences in [NOTICE.md](NOTICE.md).
