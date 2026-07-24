The `topcoat new` command scaffolds a new, pre-configured Topcoat project: a Cargo package with the dependencies declared, a starter `src/main.rs`, and everything else a chosen template needs to build and run.

# Creating a project

Point the command at the directory to create:

```sh
topcoat new my-app
```

This writes a ready-to-run project into `my-app/` and, when git is available and the directory is not already inside a repository, initializes a git repository for it. The final component of the path (`my-app`) becomes the package name.

With no `--template`, the command prompts for one when run in a terminal. Pick a template up front to skip the prompt:

```sh
topcoat new my-app --template minimal
```

Once it finishes, build and serve the project with the dev server:

```sh
cd my-app
topcoat dev
```

# Templates

Each template scaffolds a complete, compilable app. Choose one with `--template` (short `-t`), or select it from the interactive picker.

- `minimal`: a single page that renders a component. The smallest useful app, and the starting point from the [getting started](../../topcoat/docs/getting_started.md) guide.
- `tailwind`: a [Tailwind](https://tailwindcss.com/)-styled page. Adds a `build.rs` that runs the standalone Tailwind CLI and serves the compiled stylesheet as an asset, with the `tailwind` feature enabled. See the [Tailwind guide](../../topcoat/docs/tailwind.md).
- `runtime`: an interactive counter driven by client-side signals. Wires up the [client runtime](../../topcoat/docs/runtime.md) script and the asset bundle it is served from.

# Options

- `<PATH>`: the directory to create. Its final component is the package name unless `--name` overrides it.
- `--name <NAME>`: set the package name explicitly, independent of the directory name.
- `--template <TEMPLATE>`, `-t <TEMPLATE>`: the template to scaffold (`minimal`, `tailwind`, or `runtime`). When omitted, the command prompts for one; in a non-interactive context (no terminal), a template must be passed.
- `--path <DIR>`: depend on a local `topcoat` checkout by path instead of the crates.io version, for testing an unreleased `topcoat`. See below.
- `--no-git`: do not initialize a git repository in the new project.

# Depending on a local topcoat

By default a generated project depends on the crates.io `topcoat` version that matches the CLI. When developing `topcoat` itself, that published version may be behind your working tree, so the templates (which track the current API) will not compile against it. `--path` points the generated project's `topcoat` dependency (and, for the `tailwind` template, its build dependency) at a local checkout instead:

```sh
topcoat new my-app -t tailwind --path /path/to/topcoat
```

`<DIR>` accepts either the `topcoat` crate directory itself or a workspace root that contains `crates/topcoat`. The path is recorded as an absolute path, so the new project builds from anywhere. Create it outside the `topcoat` workspace so it is not absorbed into it.

# What gets generated

Every template writes:

- `Cargo.toml`: the package manifest, with `topcoat` and `tokio` declared. `topcoat` is pinned to the version matching the CLI, and templates that need extra features (such as `tailwind`) enable them here.
- `src/main.rs`: the application entry point and its pages.
- `.gitignore`: ignores the Cargo `target/` directory.
- `README.md`: a short pointer to the dev server.

The `tailwind` template additionally writes a `build.rs` that compiles the stylesheet.
