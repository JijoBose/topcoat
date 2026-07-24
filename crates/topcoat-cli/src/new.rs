mod template;

use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use clap::Args;
use console::style;

use template::{ScaffoldFile, Template, TopcoatSource};

#[derive(Args)]
pub struct NewCommand {
    /// Path of the project directory to create; its final component becomes the
    /// package name unless `--name` is given
    #[arg(value_name = "PATH")]
    path: PathBuf,
    /// Package name to use (defaults to the directory's final component)
    #[arg(long)]
    name: Option<String>,
    /// Project template to scaffold; prompts for one when omitted
    #[arg(short, long)]
    template: Option<Template>,
    /// Depend on a local `topcoat` checkout by path instead of crates.io, for
    /// testing an unreleased `topcoat`. Accepts the crate directory or a
    /// workspace root containing `crates/topcoat`
    #[arg(long = "path", value_name = "DIR")]
    topcoat_path: Option<PathBuf>,
    /// Do not initialize a git repository in the new project
    #[arg(long)]
    no_git: bool,
}

impl NewCommand {
    pub fn run(self) {
        if let Err(error) = self.run_inner() {
            eprintln!("{}", style(error).red());
            std::process::exit(1);
        }
    }

    fn run_inner(self) -> Result<(), String> {
        let name = self.package_name()?;
        validate_package_name(&name)?;

        // `symlink_metadata` (an lstat) rather than `exists` (which follows
        // symlinks): any entry at the destination, including a dangling
        // symlink, should be reported as already present rather than slipping
        // through to a confusing failure when the directory is created.
        if self.path.symlink_metadata().is_ok() {
            return Err(format!(
                "destination {} already exists",
                self.path.display()
            ));
        }

        let template = match self.template {
            Some(template) => template,
            None => choose_template()?,
        };

        let source = self.topcoat_source()?;
        let files = template.files(&name, &source);
        write_files(&self.path, &files)?;

        let git = !self.no_git && init_git(&self.path);

        report(&name, template, &self.path, &files, git);
        Ok(())
    }

    /// The package name: `--name` when given, otherwise the final component of
    /// the path.
    fn package_name(&self) -> Result<String, String> {
        if let Some(name) = &self.name {
            return Ok(name.clone());
        }
        self.path
            .file_name()
            .and_then(|component| component.to_str())
            .map(str::to_string)
            .ok_or_else(|| {
                "could not derive a package name from the path; pass --name <name>".to_string()
            })
    }

    /// Where the generated project's `topcoat` dependency points: a local
    /// checkout when `--path` is given, otherwise the crates.io version that
    /// matches this CLI.
    fn topcoat_source(&self) -> Result<TopcoatSource, String> {
        let Some(dir) = &self.topcoat_path else {
            return Ok(TopcoatSource::Version(
                env!("CARGO_PKG_VERSION").to_string(),
            ));
        };

        // An absolute path keeps the generated manifest valid regardless of
        // where the new project ends up relative to the checkout.
        let crate_dir = locate_topcoat_crate(dir)?;
        let absolute = crate_dir
            .canonicalize()
            .map_err(|error| format!("failed to resolve {}: {error}", crate_dir.display()))?;
        Ok(TopcoatSource::Path(absolute.to_string_lossy().into_owned()))
    }
}

/// Resolve `--path` to a `topcoat` crate directory, accepting either the crate
/// directory itself or a workspace root that holds `crates/topcoat`.
fn locate_topcoat_crate(dir: &Path) -> Result<PathBuf, String> {
    for candidate in [dir.to_path_buf(), dir.join("crates").join("topcoat")] {
        if is_topcoat_crate(&candidate) {
            return Ok(candidate);
        }
    }
    Err(format!(
        "no `topcoat` crate found at {}; pass the crate directory or a workspace root containing crates/topcoat",
        dir.display()
    ))
}

/// Whether `dir` holds the `topcoat` facade crate, identified by its manifest's
/// package name.
fn is_topcoat_crate(dir: &Path) -> bool {
    std::fs::read_to_string(dir.join("Cargo.toml")).is_ok_and(|manifest| {
        manifest
            .lines()
            .any(|line| line.trim() == "name = \"topcoat\"")
    })
}

/// Reject names cargo would refuse: a name must be non-empty, start with an
/// ASCII letter or `_`, and otherwise contain only letters, digits, `-`, and
/// `_`.
fn validate_package_name(name: &str) -> Result<(), String> {
    let mut chars = name.chars();
    let valid = match chars.next() {
        Some(first) if first.is_ascii_alphabetic() || first == '_' => {
            chars.all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        }
        _ => false,
    };

    if valid {
        Ok(())
    } else {
        Err(format!(
            "`{name}` is not a valid package name; use letters, digits, `-`, and `_`, \
             starting with a letter or `_`"
        ))
    }
}

/// Write every scaffolded file under `root`, creating `root` and any nested
/// parent directories (like `src/`) along the way.
fn write_files(root: &Path, files: &[ScaffoldFile]) -> Result<(), String> {
    std::fs::create_dir_all(root)
        .map_err(|error| format!("failed to create {}: {error}", root.display()))?;

    for file in files {
        let path = root.join(file.path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
        }
        std::fs::write(&path, &file.contents)
            .map_err(|error| format!("failed to write {}: {error}", path.display()))?;
    }

    Ok(())
}

/// Prompt for a template on the terminal, navigating with the arrow keys and
/// selecting with enter. Errors when there is no terminal to prompt on
/// (non-interactive use must pass `--template`) and when the prompt is
/// cancelled (e.g. ctrl-c / esc).
fn choose_template() -> Result<Template, String> {
    use dialoguer::{Select, theme::ColorfulTheme};

    if !std::io::stdin().is_terminal() {
        let names: Vec<_> = Template::ALL.iter().map(|t| t.name()).collect();
        return Err(format!(
            "no template selected and no terminal to prompt on; pass --template <name> \
             (available: {})",
            names.join(", ")
        ));
    }

    let items: Vec<_> = Template::ALL
        .iter()
        .map(|t| format!("{} - {}", t.name(), t.summary()))
        .collect();

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Choose a template")
        .items(&items)
        .default(0)
        .interact_opt()
        .map_err(|error| format!("failed to read input: {error}"))?;

    match selection {
        Some(index) => Ok(Template::ALL[index]),
        None => Err("no template selected".to_string()),
    }
}

/// Initialize a git repository in `root`, unless git is unavailable or `root`
/// already sits inside an existing repository (a nested repo would surprise).
/// Returns whether a repository was created.
fn init_git(root: &Path) -> bool {
    if inside_git_repo(root) {
        return false;
    }

    Command::new("git")
        .arg("init")
        .arg("--quiet")
        .arg(root)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

/// Whether the new project's location already lies within a git work tree.
/// `root` does not exist yet, so its parent directory is queried.
fn inside_git_repo(root: &Path) -> bool {
    let dir = root
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));

    Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["rev-parse", "--is-inside-work-tree"])
        .output()
        .is_ok_and(|output| output.status.success() && output.stdout.starts_with(b"true"))
}

/// Print the created files and the commands to run next.
fn report(name: &str, template: Template, path: &Path, files: &[ScaffoldFile], git: bool) {
    println!(
        "{} created {} {}",
        style("+").green(),
        style(name).bold(),
        style(format!("({} template)", template.name())).dim(),
    );
    for file in files {
        println!(
            "  {} {}",
            style("+").green().dim(),
            style(path.join(file.path).display()).dim(),
        );
    }
    if git {
        println!(
            "  {} {}",
            style("+").green().dim(),
            style("initialized a git repository").dim(),
        );
    }

    println!();
    println!("{}", style("Next steps:").bold());
    println!("  {} cd {}", style("-").dim(), path.display());
    println!("  {} topcoat dev", style("-").dim());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_valid_package_names() {
        for name in ["app", "my-app", "my_app", "_hidden", "a1", "App2"] {
            assert!(
                validate_package_name(name).is_ok(),
                "{name} should be valid"
            );
        }
    }

    #[test]
    fn rejects_invalid_package_names() {
        for name in ["", "1abc", "-leading", "my app", "my.app", "na/me"] {
            assert!(
                validate_package_name(name).is_err(),
                "{name} should be invalid"
            );
        }
    }

    fn command(path: &str, name: Option<&str>) -> NewCommand {
        NewCommand {
            path: PathBuf::from(path),
            name: name.map(str::to_string),
            template: None,
            topcoat_path: None,
            no_git: true,
        }
    }

    #[test]
    fn package_name_is_inferred_from_the_path() {
        assert_eq!(command("foo/bar", None).package_name().unwrap(), "bar");
        assert_eq!(command("my-app", None).package_name().unwrap(), "my-app");
    }

    #[test]
    fn explicit_name_overrides_the_path() {
        assert_eq!(
            command("some/dir", Some("chosen")).package_name().unwrap(),
            "chosen"
        );
    }

    #[test]
    fn locates_the_topcoat_crate() {
        // A throwaway fake workspace: a root holding `crates/topcoat` with a
        // manifest whose package name marks the facade crate.
        let base = std::env::temp_dir().join(format!("topcoat-new-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let crate_dir = base.join("crates").join("topcoat");
        std::fs::create_dir_all(&crate_dir).unwrap();
        std::fs::write(
            crate_dir.join("Cargo.toml"),
            "[package]\nname = \"topcoat\"\n",
        )
        .unwrap();

        // The workspace root and the crate directory both resolve to the crate.
        assert_eq!(locate_topcoat_crate(&base).unwrap(), crate_dir);
        assert_eq!(locate_topcoat_crate(&crate_dir).unwrap(), crate_dir);
        // A directory with no topcoat crate is an error, not a silent miss.
        assert!(locate_topcoat_crate(&base.join("crates")).is_err());
        // A sibling crate (`topcoat-router`) must not be mistaken for the facade.
        std::fs::write(
            crate_dir.join("Cargo.toml"),
            "[package]\nname = \"topcoat-router\"\n",
        )
        .unwrap();
        assert!(!is_topcoat_crate(&crate_dir));

        std::fs::remove_dir_all(&base).ok();
    }
}
