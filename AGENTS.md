# Repository Guidelines

## Project Structure & Module Organization

This repository currently contains the project contract rather than an implementation. Start with `README.md`, then treat `docs/blender-crowd-1.0.md` as the canonical product and engineering specification. Keep additional design decisions in `docs/` and link durable, project-wide guidance from the README.

The planned implementation layout is documented in section 14 of the contract: Blender Python belongs in `addon/`, Rust crates in `crates/`, versioned formats in `schemas/`, cross-layer tests in `tests/`, and redistributable fixtures in `assets/reference/`. Do not create all planned packages preemptively; add a module when an implemented feature or ownership boundary requires it.

## Build, Test, and Development Commands

There is no build system or automated test suite yet. For documentation changes, use these lightweight checks:

```sh
git diff --check                       # detect whitespace errors
rg '^## ' docs/blender-crowd-1.0.md    # review the contract outline
git status --short                     # confirm the intended change set
```

When implementation tooling is introduced, document exact, copy-ready Rust, Python, and headless-Blender commands here and in `README.md`. Never claim a test passed if its runner is not checked into the repository.

## Coding Style & Naming Conventions

Use four spaces for Python and standard `rustfmt` formatting for Rust. Prefer `snake_case` for Python modules, functions, Rust modules, and crate directories (crate package names may use kebab-case, such as `crowd-core`). Use `PascalCase` for types and Blender-facing classes. Keep Python orchestration coarse-grained; per-agent hot loops and authoritative simulation state belong in Rust. Preserve deterministic behavior, stable identifiers, versioned schemas, and the ownership boundaries defined by the contract.

## Testing Guidelines

Add tests with every implemented behavior. Rust unit and property tests should live beside their modules; cross-layer, packaging, and Blender headless tests belong in `tests/`. Name tests after observable behavior, for example `stable_ids_do_not_depend_on_iteration_order`. Include deterministic scenario snapshots, cache round trips, schema migration checks, and failure cases. Performance claims require a reproducible benchmark, fixture, and recorded environment.

## Commit & Pull Request Guidelines

The history currently uses concise, imperative subjects (for example, `Add Blender Crowd 1.0 architecture and MVP`). Keep commits focused and explain contract changes in the body. Pull requests should state scope, link the relevant contract section or issue, list verification performed, and call out schema/cache compatibility effects. Include screenshots or renders for Blender UI, Geometry Nodes, or visual-output changes.
