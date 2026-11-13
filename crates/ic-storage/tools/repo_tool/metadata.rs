//! Release metadata rendering through the selected shared owners.
#[cfg_attr(
    not(test),
    expect(
        clippy::wildcard_imports,
        reason = "Private release submodules share the parent helper vocabulary."
    )
)]
use super::*;
use std::io::Write as _;

impl<E: Execute> Repository<E> {
    pub(super) fn validate_date(&self, date: &str) -> Result<()> {
        require(
            self.output("perl", &[RELEASE_INPUT_ADMISSION, "date", date])? == format!("{date}\n"),
            "release date admission returned unexpected output",
        )
    }

    pub(super) fn version_parts(&self, version: &str) -> Result<[u64; 3]> {
        let output = self.output("perl", &[RELEASE_INPUT_ADMISSION, "version", version])?;
        let mut parts = output.trim_end_matches('\n').split('\t');
        let mut numbers = [0; 3];
        for number in &mut numbers {
            *number = parts
                .next()
                .ok_or("release version admission omitted a component")?
                .parse()?;
        }
        require(
            parts.next().is_none(),
            "release version admission returned extra components",
        )?;
        Ok(numbers)
    }

    // Keep exact helper inputs for retries and diagnosis. Changelog finalization uses
    // Host capture instead of writing to one pipe before draining the others.
    pub(super) fn transform_text(
        &self,
        program: &str,
        prefix: &[&str],
        text: &str,
        suffix: &[&str],
    ) -> Result<String> {
        let digest = Sha256Digest::compute(text.as_bytes());
        let input = self.artifact_paths()?.transform_input(digest);
        match fs::symlink_metadata(&input) {
            Ok(metadata) => require(metadata.is_file(), "transform input is not a regular file")?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                ic_host_fs::durable::write_with(
                    &input,
                    WriteOptions {
                        mode: PublicationMode::CreateNew,
                        ..REPLACE_FILE_OPTIONS
                    },
                    |file| file.write_all(text.as_bytes()),
                )?;
            }
            Err(error) => return Err(error.into()),
        }
        let limit = u64::try_from(text.len())?;
        require(
            hash_file(&input, limit)?.sha256 == digest,
            "transform input changed",
        )?;
        let arguments = prefix
            .iter()
            .copied()
            .chain(std::iter::once(
                input.to_str().ok_or("non-UTF-8 transform input")?,
            ))
            .chain(suffix.iter().copied())
            .collect::<Vec<_>>();
        let output = self.output(program, &arguments)?;
        require(
            hash_file(&input, limit)?.sha256 == digest,
            "transform input changed",
        )?;
        Ok(output)
    }

    pub(super) fn project_group_catalog(
        &self,
        group: &groups::ReleaseGroup,
        text: &str,
        previous: &str,
        version: &str,
    ) -> Result<String> {
        self.version_parts(previous)?;
        self.version_parts(version)?;
        let manifest: toml::Value = toml::from_str(text)?;
        let aliases = manifest["workspace"]["dependencies"]
            .as_table()
            .ok_or("missing dependency catalog")?
            .iter()
            .filter(|(alias, dependency)| {
                let package = dependency
                    .get("package")
                    .and_then(toml::Value::as_str)
                    .unwrap_or(alias);
                group.members.iter().any(|member| member == package)
            })
            .map(|(alias, dependency)| {
                require(
                    dependency
                        .get("path")
                        .and_then(toml::Value::as_str)
                        .is_some(),
                    "local requirement must select a workspace path",
                )?;
                let requirement = dependency
                    .get("version")
                    .and_then(toml::Value::as_str)
                    .ok_or("missing local requirement")?;
                let operator = usize::from(requirement.starts_with(['=', '^', '~']));
                let literal = requirement[operator..].trim_start_matches([' ', '\t']);
                require(
                    literal == previous,
                    "local requirement differs from its group version",
                )?;
                let prefix = &requirement[..requirement.len() - literal.len()];
                Ok((alias, requirement, format!("{prefix}{version}")))
            })
            .collect::<Result<Vec<_>>>()?;
        let dependencies = aliases
            .iter()
            .map(|(alias, requirement, next)| DependencyVersionProjection {
                alias,
                previous: requirement,
                next,
            })
            .collect::<Vec<_>>();
        Ok(render_group_manifest(
            text,
            previous,
            version,
            ManifestVersion::Unchanged,
            &dependencies,
        )?)
    }

    pub(super) fn replace_group_lock_version(
        group: &groups::ReleaseGroup,
        text: &str,
        previous: &str,
        version: &str,
    ) -> Result<String> {
        let updated = render_workspace_lock(
            text,
            &group
                .members
                .iter()
                .map(|package| WorkspaceVersionProjection {
                    package,
                    previous,
                    next: version,
                })
                .collect::<Vec<_>>(),
        )?;
        group.check_lock_update(text.as_bytes(), &updated, previous, version)?;
        Ok(updated)
    }

    pub(super) fn release_changelog(
        &self,
        finalizer: &Path,
        text: &str,
        previous: &str,
        version: &str,
        date: &str,
    ) -> Result<String> {
        self.version_parts(previous)?;
        self.version_parts(version)?;
        self.validate_date(date)?;
        // Published history includes bare version headings. Keep that refusal;
        // the shared helper owns candidate classification and finalization.
        require(
            !text.lines().any(|line| {
                let mut words = line.split_whitespace();
                words.next() == Some("##") && words.next() == Some(version)
            }),
            "selected version already exists",
        )?;
        self.transform_text(
            "awk",
            &[
                "-v",
                &format!("version={version}"),
                "-v",
                &format!("previous={previous}"),
                "-v",
                &format!("date={date}"),
                "-f",
                finalizer.to_str().ok_or("non-UTF-8 changelog finalizer")?,
            ],
            text,
            &[],
        )
    }
}
