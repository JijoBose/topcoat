use clap::ValueEnum;

use super::vcs::VersionControl;

/// Placeholder substituted in the template sources with the package name. It is
/// chosen to never occur in valid Rust, so a plain string replacement is
/// unambiguous.
const NAME_PLACEHOLDER: &str = "__PROJECT_NAME__";

/// Where a generated project's `topcoat` dependency points: the crates.io
/// version matching the CLI, or a local checkout depended on by path (for
/// testing an unreleased `topcoat`, via `topcoat new --path`).
pub(super) enum TopcoatSource {
    /// A crates.io version requirement, e.g. `"0.4.0"`.
    Version(String),
    /// An absolute path to a local `topcoat` crate directory.
    Path(String),
}

impl TopcoatSource {
    /// The right-hand side of a `topcoat = ...` dependency line, given the
    /// features to enable and whether default features stay on.
    fn dependency(&self, features: &[&str], default_features: bool) -> String {
        // The bare-string form (`topcoat = "0.4.0"`) is only possible for a
        // version with nothing else to say.
        if let TopcoatSource::Version(version) = self
            && features.is_empty()
            && default_features
        {
            return format!(r#""{version}""#);
        }

        let mut parts = vec![match self {
            TopcoatSource::Version(version) => format!(r#"version = "{version}""#),
            TopcoatSource::Path(path) => format!(r#"path = "{path}""#),
        }];
        if !default_features {
            parts.push("default-features = false".to_string());
        }
        if !features.is_empty() {
            let list = features
                .iter()
                .map(|feature| format!(r#""{feature}""#))
                .collect::<Vec<_>>()
                .join(", ");
            parts.push(format!("features = [{list}]"));
        }
        format!("{{ {} }}", parts.join(", "))
    }
}

/// A file the scaffolder writes into the new project: a path relative to the
/// project root and the contents to write there.
pub(super) struct ScaffoldFile {
    pub path: &'static str,
    pub contents: String,
}

/// A starter project layout selectable with `--template` or from the
/// interactive picker. Each one scaffolds a complete, compilable app.
#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(super) enum Template {
    /// A single page rendering a component
    Minimal,
    /// A Tailwind-styled page with the build script wired up
    Tailwind,
    /// An interactive counter driven by client-side signals
    Runtime,
}

impl Template {
    /// Every template, in the order shown by the interactive picker.
    pub(super) const ALL: &'static [Template] =
        &[Template::Minimal, Template::Tailwind, Template::Runtime];

    /// The name used with `--template <name>` and in reporting.
    pub(super) fn name(self) -> &'static str {
        match self {
            Template::Minimal => "minimal",
            Template::Tailwind => "tailwind",
            Template::Runtime => "runtime",
        }
    }

    /// A one-line summary shown beside the name in the interactive picker.
    pub(super) fn summary(self) -> &'static str {
        match self {
            Template::Minimal => "a single page rendering a component",
            Template::Tailwind => "a Tailwind-styled page with the build wired up",
            Template::Runtime => "an interactive counter driven by client-side signals",
        }
    }

    /// The files to write for a new project named `name`, depending on
    /// `topcoat` from `source` and placed under `vcs`.
    pub(super) fn files(
        self,
        name: &str,
        source: &TopcoatSource,
        vcs: VersionControl,
    ) -> Vec<ScaffoldFile> {
        let fill = |template: &str| template.replace(NAME_PLACEHOLDER, name);

        let mut files = vec![ScaffoldFile {
            path: "Cargo.toml",
            contents: self.cargo_toml(name, source),
        }];

        files.extend(
            vcs.ignore_files()
                .iter()
                .map(|&(path, contents)| ScaffoldFile {
                    path,
                    contents: contents.to_string(),
                }),
        );

        files.push(ScaffoldFile {
            path: "README.md",
            contents: fill(README),
        });
        files.push(ScaffoldFile {
            path: "src/main.rs",
            contents: fill(self.main_rs()),
        });

        if let Template::Tailwind = self {
            files.push(ScaffoldFile {
                path: "build.rs",
                contents: BUILD_RS.to_string(),
            });
        }

        files
    }

    /// The manifest for this template. `minimal` and `runtime` only need the
    /// default features; `tailwind` opts into the `tailwind` feature and adds a
    /// build dependency for the build script.
    fn cargo_toml(self, name: &str, source: &TopcoatSource) -> String {
        let (dep, build_section) = match self {
            // We cannot use the default features in the build script, so the
            // build dependency turns them off and enables only `tailwind`.
            Template::Tailwind => (
                source.dependency(&["tailwind"], true),
                format!(
                    "\n[build-dependencies]\ntopcoat = {}\n",
                    source.dependency(&["tailwind"], false)
                ),
            ),
            Template::Minimal | Template::Runtime => (source.dependency(&[], true), String::new()),
        };

        format!(
            r#"[package]
name = "{name}"
version = "0.1.0"
edition = "2024"

[dependencies]
tokio = {{ version = "1", features = ["rt-multi-thread", "macros"] }}
topcoat = {dep}
{build_section}"#
        )
    }

    fn main_rs(self) -> &'static str {
        match self {
            Template::Minimal => MAIN_MINIMAL,
            Template::Tailwind => MAIN_TAILWIND,
            Template::Runtime => MAIN_RUNTIME,
        }
    }
}

const README: &str = r"# __PROJECT_NAME__

A web app built with [Topcoat](https://github.com/tokio-rs/topcoat).

## Development

Install the Topcoat CLI, then start the dev server:

```sh
cargo install topcoat-cli
topcoat dev
```

Open <http://127.0.0.1:3000> to view the app. The dev server watches your
sources and rebuilds, rebundles, and reloads on every change.
";

const BUILD_RS: &str = r#"fn main() {
    // Tailwind scans `src` for class names. Scanning the package root instead
    // would rely on an ignore file to stay out of `target`, which not every
    // version control system leaves behind.
    topcoat::tailwind::BuildConfig::new()
        .cwd("src")
        .render()
        .unwrap();
}
"#;

const MAIN_MINIMAL: &str = r#"use topcoat::{
    Result,
    router::{Router, RouterBuilderDiscoverExt, page},
    view::{component, view},
};

#[tokio::main]
async fn main() {
    topcoat::start(Router::builder().discover().build())
        .await
        .unwrap();
}

#[page("/")]
async fn home() -> Result {
    view! {
        <!DOCTYPE html>
        <html>
            <head>
                <title>"__PROJECT_NAME__"</title>
                topcoat::dev::script()
            </head>
            <body>hello(name: "World")</body>
        </html>
    }
}

#[component]
async fn hello(name: &str) -> Result {
    view! {
        <h1>
            "Hello, "
            (name)
            "!"
        </h1>
    }
}
"#;

const MAIN_TAILWIND: &str = r#"use topcoat::{
    Result,
    asset::{AssetBundle, RouterBuilderAssetExt},
    router::{Router, layout, page},
    tailwind,
    view::view,
};

#[tokio::main]
async fn main() {
    let router = Router::builder()
        .layout(root_layout)
        .page(home)
        .assets(AssetBundle::load().unwrap())
        .build();

    topcoat::start(router).await.unwrap();
}

#[layout("/")]
async fn root_layout(slot: Result) -> Result {
    view! {
        <!DOCTYPE html>
        <html>
            <head>
                <title>"__PROJECT_NAME__"</title>
                topcoat::dev::script()
                <link rel="stylesheet" href=(tailwind::stylesheet!())>
            </head>
            <body
                class="flex min-h-screen items-center justify-center bg-slate-100 font-sans"
            >
                (slot?)
            </body>
        </html>
    }
}

#[page("/")]
async fn home() -> Result {
    view! {
        <main
            class="mx-4 w-full max-w-md rounded-2xl bg-white p-8 shadow-lg ring-1 ring-slate-200"
        >
            <h1 class="text-2xl font-bold tracking-tight text-slate-900">
                "__PROJECT_NAME__"
            </h1>
            <p class="mt-2 text-slate-600">
                "Utility classes in your Rust sources are compiled into this \
                 page's stylesheet by the standalone Tailwind CLI."
            </p>
            <a
                href="https://tailwindcss.com/docs"
                class="mt-6 inline-block rounded-lg bg-blue-600 px-4 py-2 font-semibold text-white shadow-sm hover:bg-blue-500"
            >
                "Read the Tailwind docs"
            </a>
        </main>
    }
}
"#;

const MAIN_RUNTIME: &str = r#"use topcoat::{
    Result,
    asset::{AssetBundle, RouterBuilderAssetExt},
    router::{Router, RouterBuilderDiscoverExt, layout, page},
    view::view,
};

#[tokio::main]
async fn main() {
    // The client runtime is served from the asset bundle, so the runtime
    // script in the layout below needs it registered. `topcoat dev` builds the
    // bundle for you before starting the app.
    let router = Router::builder()
        .assets(AssetBundle::load().unwrap())
        .discover()
        .build();

    topcoat::start(router).await.unwrap();
}

#[layout("/")]
async fn root_layout(slot: Result) -> Result {
    view! {
        <!DOCTYPE html>
        <html>
            <head>
                <title>"__PROJECT_NAME__"</title>
                topcoat::dev::script()
                topcoat::runtime::script()
            </head>
            <body>(slot?)</body>
        </html>
    }
}

#[page("/")]
async fn home() -> Result {
    view! {
        signal count = 0.0;

        <button @click=$(|_e| count.set(count.get() + 1.0))>"increment"</button>
        <button @click=$(|_e| count.set(count.get() - 1.0))>"decrement"</button>

        <br>
        <br>

        $(count.get())
    }
}
"#;

#[cfg(test)]
mod tests {
    use super::*;

    fn version() -> TopcoatSource {
        TopcoatSource::Version("0.4.0".to_string())
    }

    fn file<'a>(files: &'a [ScaffoldFile], path: &str) -> &'a ScaffoldFile {
        files
            .iter()
            .find(|file| file.path == path)
            .unwrap_or_else(|| panic!("missing {path}"))
    }

    #[test]
    fn crates_io_dependency_rendering() {
        let source = version();
        assert_eq!(source.dependency(&[], true), r#""0.4.0""#);
        assert_eq!(
            source.dependency(&["tailwind"], true),
            r#"{ version = "0.4.0", features = ["tailwind"] }"#
        );
        assert_eq!(
            source.dependency(&["tailwind"], false),
            r#"{ version = "0.4.0", default-features = false, features = ["tailwind"] }"#
        );
    }

    #[test]
    fn path_dependency_rendering() {
        let source = TopcoatSource::Path("/abs/crates/topcoat".to_string());
        assert_eq!(
            source.dependency(&[], true),
            r#"{ path = "/abs/crates/topcoat" }"#
        );
        assert_eq!(
            source.dependency(&["tailwind"], false),
            r#"{ path = "/abs/crates/topcoat", default-features = false, features = ["tailwind"] }"#
        );
    }

    #[test]
    fn every_template_generates_valid_sources() {
        for &template in Template::ALL {
            let files = template.files("my-app", &version(), VersionControl::Git);

            // The manifest is valid TOML naming the requested package and
            // pinning the requested topcoat version.
            let manifest = file(&files, "Cargo.toml");
            let parsed: toml::Table =
                toml::from_str(&manifest.contents).expect("Cargo.toml is valid TOML");
            assert_eq!(parsed["package"]["name"].as_str(), Some("my-app"));
            assert!(
                manifest.contents.contains("0.4.0"),
                "{} pins the topcoat version",
                template.name()
            );

            // The Rust sources parse, with the name placeholder filled in.
            let main = file(&files, "src/main.rs");
            syn::parse_file(&main.contents).expect("src/main.rs is valid Rust");
            assert!(!main.contents.contains(NAME_PLACEHOLDER));

            // The common files are always present.
            file(&files, ".gitignore");
            file(&files, "README.md");
        }
    }

    #[test]
    fn no_generated_source_carries_a_doubled_backslash() {
        // The templates are raw strings, which perform no escape processing, so
        // a backslash written twice reaches the generated file as two
        // characters. `MAIN_TAILWIND` ends a line with one to continue a string
        // literal; doubled, the generated file still parses and still compiles,
        // just with a stray backslash in the rendered text. Nothing else here
        // would notice.
        for &template in Template::ALL {
            for file in template.files("my-app", &version(), VersionControl::Git) {
                assert!(
                    !file.contents.contains(r"\\"),
                    "{} in the {} template",
                    file.path,
                    template.name()
                );
            }
        }
    }

    #[test]
    fn the_ignore_files_follow_the_version_control_system() {
        for &template in Template::ALL {
            // `none` writes no ignore file, so it is the project without any:
            // everything a system adds on top of it is an ignore file of its
            // own, and nothing else about the project may change.
            let bare = template.files("my-app", &version(), VersionControl::None);
            let bare_paths: Vec<_> = bare.iter().map(|file| file.path).collect();

            for &vcs in VersionControl::value_variants() {
                let files = template.files("my-app", &version(), vcs);
                let added: Vec<_> = files
                    .iter()
                    .filter(|file| !bare_paths.contains(&file.path))
                    .map(|file| (file.path, file.contents.as_str()))
                    .collect();

                assert_eq!(
                    added,
                    vcs.ignore_files().to_vec(),
                    "{} under {}",
                    template.name(),
                    vcs.name()
                );
                assert_eq!(
                    files.len(),
                    bare.len() + added.len(),
                    "{} under {} drops nothing",
                    template.name(),
                    vcs.name()
                );
            }
        }
    }

    #[test]
    fn the_tailwind_build_script_does_not_depend_on_an_ignore_file() {
        // Scanning the package root would need an ignore file to keep Tailwind
        // out of `target`, and `--vcs none` writes none.
        let files = Template::Tailwind.files("my-app", &version(), VersionControl::None);
        let build = file(&files, "build.rs");
        assert!(build.contents.contains(r#".cwd("src")"#));
    }

    #[test]
    fn path_source_produces_a_valid_path_manifest() {
        for &template in Template::ALL {
            let source = TopcoatSource::Path("/home/dev/topcoat/crates/topcoat".to_string());
            let files = template.files("app", &source, VersionControl::Git);
            let manifest = &file(&files, "Cargo.toml").contents;

            let parsed: toml::Table = toml::from_str(manifest).expect("valid TOML");
            let dep = &parsed["dependencies"]["topcoat"];
            assert_eq!(
                dep["path"].as_str(),
                Some("/home/dev/topcoat/crates/topcoat"),
                "{} points topcoat at the local path",
                template.name()
            );
            assert!(
                dep.get("version").is_none(),
                "a path dependency carries no version"
            );
        }
    }

    #[test]
    fn only_tailwind_has_a_build_script() {
        for &template in Template::ALL {
            let files = template.files("app", &version(), VersionControl::Git);
            let build = files.iter().find(|f| f.path == "build.rs");
            match template {
                Template::Tailwind => {
                    let build = build.expect("tailwind template has a build script");
                    syn::parse_file(&build.contents).expect("build.rs is valid Rust");
                }
                Template::Minimal | Template::Runtime => {
                    assert!(build.is_none(), "{} has no build script", template.name());
                }
            }
        }
    }

    #[test]
    fn tailwind_manifest_enables_the_feature() {
        let files = Template::Tailwind.files("app", &version(), VersionControl::Git);
        let manifest = &file(&files, "Cargo.toml").contents;
        assert!(manifest.contains(r#"features = ["tailwind"]"#));
        assert!(manifest.contains("[build-dependencies]"));
        assert!(manifest.contains("default-features = false"));
    }
}
