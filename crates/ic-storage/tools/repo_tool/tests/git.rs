//! Git observations and their interruption points own one fixture history.
use super::*;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum GitFault {
    History,
    Parent,
    PublishBeforeTag,
    PublishTagCommit,
    PublishTagMissing,
    PublishTagObject,
    RemoteAhead,
    RemoteLock,
    Status,
    TagAdmissionRefused,
    TagAdmissionMissing,
    TagAdmissionObject,
    TagCommit,
    TagObject,
}

#[derive(Default, Deserialize, Serialize)]
pub(super) struct GitState {
    pub group: String,
    pub source: BTreeMap<String, String>,
    pub release: Option<BTreeMap<String, String>>,
    pub staged: bool,
    pub tag: bool,
    pub descendant: Option<BTreeMap<String, String>>,
    pub validation_baseline: Option<BTreeMap<String, String>>,
    pub fail: Option<GitFault>,
}

impl GitState {
    fn head(&self) -> &str {
        if self.descendant.is_some() {
            "fix"
        } else if self.release.is_some() {
            "release"
        } else {
            "source"
        }
    }

    fn revision_files(&self, revision: &str) -> Result<&BTreeMap<String, String>> {
        match revision {
            "source" => Ok(&self.source),
            "baseline" => self
                .validation_baseline
                .as_ref()
                .ok_or_else(|| "missing baseline".into()),
            "release" => self
                .release
                .as_ref()
                .ok_or_else(|| "missing fixture release".into()),
            "fix" => self
                .descendant
                .as_ref()
                .ok_or_else(|| "missing fixture descendant".into()),
            _ => Err("unexpected fixture revision".into()),
        }
    }

    pub(super) fn check_tag(&mut self, args: &[&str]) -> Result<String> {
        require(
            args.len() == 4 && args[..3] == [TAG_CHECKER, "release", "0.13.0"],
            "unexpected fixture tag selection",
        )?;
        match self.fail.as_ref() {
            Some(GitFault::TagCommit | GitFault::TagAdmissionRefused) => {
                Err("fixture shared tag checker refused".into())
            }
            Some(GitFault::TagAdmissionObject) => {
                self.fail = Some(GitFault::TagObject);
                Ok(String::new())
            }
            Some(GitFault::TagAdmissionMissing) => {
                self.tag = false;
                Ok(String::new())
            }
            _ => Ok(String::new()),
        }
    }

    pub(super) fn after_publish(&mut self) {
        match self.fail.as_ref() {
            Some(GitFault::PublishTagMissing) => self.tag = false,
            Some(GitFault::PublishTagCommit) => self.fail = Some(GitFault::TagCommit),
            Some(GitFault::PublishTagObject) => self.fail = Some(GitFault::TagObject),
            _ => {}
        }
    }
    pub(super) fn capture(
        &mut self,
        root: &Path,
        args: &[&str],
        calls: &[Vec<String>],
    ) -> Result<String> {
        let changed = |reference: &BTreeMap<String, String>| -> Result<String> {
            Ok(reference
                .iter()
                .filter_map(|(path, text)| match fs::read_to_string(root.join(path)) {
                    Ok(current) if current == *text => None,
                    _ => Some(path.as_str()),
                })
                .collect::<Vec<_>>()
                .join("\n"))
        };
        match args {
            ["--no-optional-locks", "status", ..]
                if self.fail.as_ref() == Some(&GitFault::Status) =>
            {
                Err("fixture Git status observation failure".into())
            }
            ["--no-optional-locks", "status", ..] => changed(self.revision_files(self.head())?),
            ["ls-tree", "-r", "--name-only", "-z", revision] => Ok(self
                .revision_files(revision)?
                .keys()
                .fold(String::new(), |mut text, path| {
                    text.push_str(path);
                    text.push('\0');
                    text
                })),
            ["rev-parse", "--verify", _] if self.validation_baseline.is_some() => {
                Ok("baseline".to_owned())
            }
            ["rev-parse", "--verify", _] => Err("fixture has no previous tag".into()),
            ["cat-file", "-t", _] => Ok("tag".to_owned()),
            [
                "diff",
                "--name-only",
                "--no-renames",
                "-z",
                "baseline",
                "source",
                "--",
            ] => {
                let baseline = self.validation_baseline.as_ref().unwrap();
                Ok(self
                    .source
                    .iter()
                    .filter(|(path, text)| baseline.get(*path) != Some(*text))
                    .fold(String::new(), |mut text, (path, _)| {
                        text.push_str(path);
                        text.push('\0');
                        text
                    }))
            }
            ["rev-parse", "HEAD"] => {
                if self.fail.as_ref() == Some(&GitFault::PublishBeforeTag)
                    && calls
                        .iter()
                        .any(|call| call.get(1).is_some_and(|arg| arg == TAG_CHECKER))
                {
                    self.tag = false;
                }
                Ok(self.head().to_owned())
            }
            ["rev-parse", "HEAD^"] => Ok("source".to_owned()),
            ["show", object] => {
                let (revision, path) = object.split_once(':').ok_or("invalid fixture object")?;
                match revision {
                    "" if self.staged => Ok(fs::read_to_string(root.join(path))?),
                    revision => self
                        .revision_files(revision)?
                        .get(path)
                        .cloned()
                        .ok_or_else(|| "missing fixture path".into()),
                }
            }
            ["for-each-ref", ..] => Ok(if self.tag {
                format!(
                    "{} refs/tags/{}-v0.13.0",
                    if self.fail.as_ref() == Some(&GitFault::TagObject) {
                        "changed-tag-object"
                    } else {
                        "tag-object"
                    },
                    self.group
                )
            } else {
                String::new()
            }),
            ["symbolic-ref", ..] => Ok("main".to_owned()),
            ["merge-base", ..] if self.fail.as_ref() == Some(&GitFault::RemoteAhead) => {
                Err("fixture remote is ahead".into())
            }
            ["fetch", ..] if self.fail.as_ref() == Some(&GitFault::RemoteLock) => {
                if calls
                    .iter()
                    .filter(|call| call.get(1).is_some_and(|arg| arg == "fetch"))
                    .count()
                    == 2
                {
                    fs::write(
                        root.join("Cargo.lock"),
                        fixture_lock().replace(
                            "name = \"other\"\nversion = \"0.12.3\"",
                            "name = \"other\"\nversion = \"0.12.4\"",
                        ),
                    )?;
                }
                Ok(String::new())
            }
            ["merge-base", "--is-ancestor", _, _]
                if self.fail.as_ref() == Some(&GitFault::History) =>
            {
                Err("fixture unrelated history".into())
            }
            ["fetch" | "merge-base" | "ls-files", ..]
            | ["diff", "--cached", "--summary", "source", "--", _] => Ok(String::new()),
            ["diff", "--cached", "--name-only", "source", "--"] => {
                if self.staged && self.release.is_none() {
                    changed(&self.source)
                } else {
                    Ok(String::new())
                }
            }
            ["diff", "--name-only", "source", "release", "--"] => {
                let release = self.revision_files("release")?;
                Ok(self
                    .source
                    .iter()
                    .filter_map(|(path, original)| {
                        (release.get(path) != Some(original)).then_some(path.as_str())
                    })
                    .collect::<Vec<_>>()
                    .join("\n"))
            }
            ["diff", "--name-only", "source", ..] => changed(&self.source),
            ["diff", "--name-only", "fix", ..] => changed(self.revision_files("fix")?),
            ["diff", "--cached", "--name-only" | "--summary", "fix", ..] => {
                if self.staged {
                    changed(self.revision_files("fix")?)
                } else {
                    Ok(String::new())
                }
            }
            ["diff", "--name-only"] => {
                if self.staged {
                    Ok(String::new())
                } else {
                    changed(&self.source)
                }
            }
            ["diff", "--cached", "--name-only"] => {
                if self.staged {
                    changed(&self.source)
                } else {
                    Ok(String::new())
                }
            }
            ["log", "-1", "--format=%P", "release"] => {
                Ok(if self.fail.as_ref() == Some(&GitFault::Parent) {
                    "wrong-source"
                } else {
                    "source"
                }
                .to_owned())
            }
            ["log", "-1", "--format=%s", "fix"] => Ok("Fix callback".to_owned()),
            ["log", "-1", "--format=%s", "release"] => Ok(if self.release.is_some() {
                format!("Release {} 0.13.0", self.group)
            } else {
                "Fixture source".to_owned()
            }),
            _ => Err(format!("unexpected fixture git command: {args:?}").into()),
        }
    }
}
