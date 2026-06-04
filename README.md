# lorcana-engine

[![CI](https://github.com/bjorvack/lorcana-engine/actions/workflows/ci.yml/badge.svg)](https://github.com/bjorvack/lorcana-engine/actions/workflows/ci.yml)
[![codecov](https://codecov.io/gh/bjorvack/lorcana-engine/graph/badge.svg?token=GCZZO49E10)](https://codecov.io/gh/bjorvack/lorcana-engine)

A game engine for the Lorcana trading card game.

## Development

See [docs/development/CONTRIBUTING.md](docs/development/CONTRIBUTING.md) for development setup and guidelines.

## Web board viewer

A browser board viewer is in [`web/`](web), powered by the engine compiled to
WebAssembly. It's a Cargo workspace:

- `lorcana-engine` (this crate) — the headless engine, unchanged.
- `crates/lorcana-wasm` — thin `wasm-bindgen` bindings exposing a stable view
  model (TypeScript types generated via `tsify`). A real game runs in the
  browser.
- `web/` — a [Svelte](https://svelte.dev) + [Vite](https://vite.dev) app that
  renders the playmat (zones, lore, phase, ready/exerted/drying/damage) using
  the real card art from the set TOML `image` fields.

```bash
# one-time: the WASM build needs the rustup toolchain + wasm32 target + wasm-pack
rustup target add wasm32-unknown-unknown
cargo install wasm-pack            # or: brew install wasm-pack

cd web
npm install
npm run wasm                       # build the WASM package into src/lib/wasm/
npm run dev                        # http://localhost:5173
```

Other web scripts: `npm run lint` (ESLint + Stylelint + Prettier),
`npm run check` (svelte-check), `npm test` (Vitest), `npm run build`
(type-check + production bundle). The viewer deploys to GitHub Pages via
[`.github/workflows/pages.yml`](.github/workflows/pages.yml) — see that file for
the private-repo / plan caveats.

## Documentation

- [Architecture](docs/architecture/ARCHITECTURE.md) - Engine architecture and design principles
- [Implementation Plan](docs/planning/IMPLEMENTATION_PLAN.md) - Detailed implementation roadmap
- [Contributing Guide](docs/development/CONTRIBUTING.md) - Development guidelines and best practices

## License

This project is licensed under MIT OR Apache-2.0.