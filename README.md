# asana-omnifocus-sync

English | [日本語](README_ja.md)

A CLI that performs a one-way sync of your incomplete, self-assigned Asana tasks into a designated OmniFocus project.

Each OmniFocus task carries the GID of its source Asana task in its note, and that GID is the source of truth for matching. The tool keeps no database or state file of its own, so running it repeatedly is idempotent.

macOS only. It uses JXA (JavaScript for Automation) to talk to OmniFocus.

## How the sync behaves

Each run reconciles the current state of Asana into OmniFocus:

- Tasks in Asana but not in OmniFocus are created.
- Tasks in both, where the name, due date, note, or project tags differ, are updated.
- Tasks in OmniFocus (incomplete) but absent from the Asana result are completed.

The tool fetches only tasks that are currently assigned to you and incomplete. Completed tasks and tasks reassigned away from you never appear in the result, so the tool treats any such task still present in OmniFocus as completed.

Tasks you complete in OmniFocus first are excluded from matching and are never reopened.

## Asana projects become OmniFocus tags

An Asana task can belong to several projects at once, while an OmniFocus task lives in a single project. To bridge this, the tool represents each Asana project as an OmniFocus tag rather than a project. Tasks stay in the one destination project, and their Asana projects are expressed as tags.

Tags are nested under a root tag (`Asana` by default) with the project name. A task that belongs to multiple Asana projects receives multiple tags. A task that belongs to no project receives the root tag only.

Only tags under the root tag are managed by the sync. When a task's project membership changes in Asana, its managed tags are replaced accordingly, while any other tags you added by hand (contexts, locations) are preserved. Tags that fall out of use are left in place rather than deleted, and an Asana project rename is treated as a new tag.

## Requirements

- macOS with OmniFocus
- An Asana personal access token (create one in the [Asana developer console](https://app.asana.com/0/my-apps))
- A Rust toolchain, or nix

## Installation

With nix, build a single binary at `./result/bin/asana-omnifocus-sync`:

```
nix build
```

With a Rust toolchain directly:

```
cargo build --release
```

## Configuration

Place a config file at `~/.config/asana-omnifocus-sync/config.toml` (`XDG_CONFIG_HOME` is honored). Use `config.example.toml` as a template.

```toml
workspace_gid = "1234567890"   # GID of the target Asana workspace (required)
omnifocus_project = "Asana"    # destination OmniFocus project name (defaults to "Asana")
omnifocus_tag_root = "Asana"   # root tag for project tags (defaults to "Asana")
tls_insecure = false           # set true to disable TLS certificate verification (defaults to false)
```

The authentication token is passed via the `ASANA_TOKEN` environment variable, not the config file.

```
export ASANA_TOKEN="<your-personal-access-token>"
```

You can find the workspace GID in the Asana URL (`https://app.asana.com/0/<gid>/...`).

## Usage

First check the planned operations with `--dry-run`. This mode does not modify OmniFocus.

```
asana-omnifocus-sync --dry-run
```

When it looks right, run it without the flag to apply the changes.

```
asana-omnifocus-sync
```

On exit it prints a summary in the form `created=N updated=N completed=N`.

### Options

- `--dry-run`: print the planned operations without applying them
- `--config <path>`: override the config file path
- `--insecure`: disable TLS certificate verification (takes precedence over `tls_insecure`)

## TLS and corporate proxies

HTTPS uses native-tls (Security.framework on macOS) and trusts the root certificates in the system keychain. A corporate CA injected by a TLS-intercepting proxy such as Netskope is accepted with verification intact, as long as that CA is installed in the keychain.

When verification cannot succeed (for example, the corporate CA is not in the keychain), you can disable it with `tls_insecure = true` or `--insecure`. Because this turns verification off, use it only on networks you trust.

## Development

Development happens inside the nix flake devShell (direnv loads `use flake` via `.envrc`).

```
cargo test       # unit tests for the reconcile logic
cargo clippy --all-targets -- --deny warnings
nix flake check  # run clippy, tests, and the build together
```

Design and detailed specifications live in `docs/`:

- `docs/use-cases.md`: use cases
- `docs/requirements.md`: requirements
- `docs/design.md`: design (module layout and data model)
- `docs/spec.md`: detailed spec (API parameters, note format, sync rules)
