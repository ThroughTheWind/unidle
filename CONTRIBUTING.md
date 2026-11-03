# Contributing

Thanks for helping out. Issues and pull requests are welcome.

## Before you start

- Read the [spec](docs/SPEC.md). The app's cross-cutting principles are in the [roadmap](docs/ROADMAP.md): never harm the user's work, always stay visible, stay local, and keep OFF safe.
- For anything larger than a small fix, open an issue first so we can agree on the approach.

## Making a change

1. Set up your machine as described in the [README](README.md).
2. Create a branch off `main`: `feature/WI-###-slug` for tracked work, or `fix/`, `chore/` or `docs/` otherwise. How work is tracked is explained in the [backlog guide](docs/backlog/README.md).
3. Keep OS calls inside platform adapters in `crates/core`. The UI only talks to the controller.
4. Run the checks:

   ```bash
   cargo fmt --all --check
   cargo clippy --workspace --all-targets -- -D warnings
   cargo test --workspace
   npm run typecheck
   node .ai/rungs.mjs check
   ```

5. Use conventional commit prefixes (`feat:`, `fix:`, `docs:`, `chore:`) and add a line to the [changelog](CHANGELOG.md) when users would notice the change.

## Code of conduct

Everyone taking part follows the [code of conduct](CODE_OF_CONDUCT.md).
