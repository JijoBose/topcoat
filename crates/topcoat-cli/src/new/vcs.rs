use std::path::Path;
use std::process::{Command, Stdio};

use clap::ValueEnum;

/// The version control system a new project is placed under, selected with
/// `--vcs`. The names match cargo's own `--vcs` values, so the two commands
/// accept the same vocabulary.
#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub(super) enum VersionControl {
    /// Git
    Git,
    /// Mercurial
    Hg,
    /// Pijul
    Pijul,
    /// Fossil
    Fossil,
    /// Create no repository at all
    None,
}

impl VersionControl {
    /// The name used with `--vcs <name>` and in reporting.
    pub(super) fn name(self) -> &'static str {
        match self {
            VersionControl::Git => "git",
            VersionControl::Hg => "hg",
            VersionControl::Pijul => "pijul",
            VersionControl::Fossil => "fossil",
            VersionControl::None => "none",
        }
    }

    /// Create a repository of this kind in `root`, which must already exist.
    ///
    /// The commands run from within `root` rather than taking it as an
    /// argument, so a project path beginning with `-` is never read as a flag.
    pub(super) fn init(self, root: &Path) -> Result<(), String> {
        for (program, args) in self.init_commands() {
            let status = Command::new(program)
                .args(*args)
                .current_dir(root)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .map_err(|error| format!("failed to run `{program}`: {error}"))?;

            if !status.success() {
                return Err(format!("`{program} {}` failed", args.join(" ")));
            }
        }

        Ok(())
    }

    /// The commands that create the repository, run in order. Fossil takes two:
    /// the first writes the repository database, the second checks it out in
    /// place. Running them on an empty directory is what keeps `fossil open`
    /// from refusing the checkout.
    fn init_commands(self) -> &'static [(&'static str, &'static [&'static str])] {
        match self {
            VersionControl::Git => &[("git", &["init", "--quiet"])],
            VersionControl::Hg => &[("hg", &["init"])],
            VersionControl::Pijul => &[("pijul", &["init"])],
            VersionControl::Fossil => &[
                ("fossil", &["init", ".fossil"]),
                ("fossil", &["open", ".fossil"]),
            ],
            VersionControl::None => &[],
        }
    }

    /// The ignore files to scaffold, as paths relative to the project root
    /// paired with their contents. Each system reads a file of its own, and in
    /// its own syntax: Mercurial's default is regular expressions, and Fossil
    /// keeps its settings in a directory and distinguishes the files it ignores
    /// from the ones it will delete when cleaning.
    ///
    /// Every entry excludes the cargo `target` directory, and `none` gets no
    /// ignore file, since nothing would read it.
    pub(super) fn ignore_files(self) -> &'static [(&'static str, &'static str)] {
        match self {
            VersionControl::Git => &[(".gitignore", "/target\n")],
            VersionControl::Hg => &[(".hgignore", "^target$\n")],
            VersionControl::Pijul => &[(".ignore", "/target\n")],
            VersionControl::Fossil => &[
                (".fossil-settings/ignore-glob", "target\n"),
                (".fossil-settings/clean-glob", "target\n"),
            ],
            VersionControl::None => &[],
        }
    }
}

/// Whether the new project's location already lies within a repository, in
/// which case creating another one inside it would surprise. `root` does not
/// exist yet, so its parent directory is queried.
///
/// Only git and Mercurial are detected, as in cargo. The check only picks the
/// default, which `--vcs` overrides, so missing a repository of another kind
/// costs nothing beyond an unwanted nested one having to be deleted.
pub(super) fn existing_repo(root: &Path) -> bool {
    let dir = root
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));

    in_git_repo(dir) || in_hg_repo(dir)
}

/// Whether `dir` lies within a git work tree. A bare repository is not one, and
/// a project cannot be created inside it, so the work tree is what to ask about.
fn in_git_repo(dir: &Path) -> bool {
    Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["rev-parse", "--is-inside-work-tree"])
        .output()
        .is_ok_and(|output| output.status.success() && output.stdout.starts_with(b"true"))
}

/// Whether `dir` lies within a Mercurial repository, which `hg root` answers by
/// printing the enclosing one's path and failing when there is none.
fn in_hg_repo(dir: &Path) -> bool {
    Command::new("hg")
        .arg("--cwd")
        .arg(dir)
        .arg("root")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_name_parses_back_to_its_variant() {
        // The vocabulary is shared with cargo's `--vcs`, so it is a contract
        // rather than an implementation detail of the derive.
        let names: Vec<_> = VersionControl::value_variants()
            .iter()
            .map(|vcs| vcs.name())
            .collect();
        assert_eq!(names, ["git", "hg", "pijul", "fossil", "none"]);

        for &vcs in VersionControl::value_variants() {
            let parsed = VersionControl::from_str(vcs.name(), true)
                .unwrap_or_else(|_| panic!("{} should parse", vcs.name()));
            assert_eq!(parsed, vcs);
        }

        assert!(VersionControl::from_str("svn", true).is_err());
    }

    #[test]
    fn each_system_ignores_the_target_directory() {
        for &vcs in VersionControl::value_variants() {
            let files = vcs.ignore_files();
            if vcs == VersionControl::None {
                assert!(files.is_empty(), "none writes no ignore file");
                continue;
            }
            assert!(
                files
                    .iter()
                    .all(|(_, contents)| contents.contains("target")),
                "{} ignores target",
                vcs.name()
            );
        }

        assert_eq!(
            VersionControl::Git.ignore_files(),
            [(".gitignore", "/target\n")]
        );
        assert_eq!(
            VersionControl::Hg.ignore_files(),
            [(".hgignore", "^target$\n")]
        );
        assert_eq!(
            VersionControl::Pijul.ignore_files(),
            [(".ignore", "/target\n")]
        );
        assert_eq!(
            VersionControl::Fossil.ignore_files(),
            [
                (".fossil-settings/ignore-glob", "target\n"),
                (".fossil-settings/clean-glob", "target\n"),
            ]
        );
    }

    #[test]
    fn detects_an_enclosing_repository() {
        // This crate lives in the topcoat git repository, so a project created
        // beside this file would be nested inside it.
        let inside = Path::new(env!("CARGO_MANIFEST_DIR")).join("new-project");
        assert!(existing_repo(&inside));

        // The system temp directory is not under version control.
        let outside = std::env::temp_dir().join("topcoat-new-project");
        assert!(!existing_repo(&outside));
    }

    #[test]
    fn initializing_no_vcs_runs_nothing() {
        let root = std::env::temp_dir().join(format!("topcoat-vcs-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();

        assert!(VersionControl::None.init(&root).is_ok());
        assert_eq!(std::fs::read_dir(&root).unwrap().count(), 0);

        std::fs::remove_dir_all(&root).ok();
    }
}
