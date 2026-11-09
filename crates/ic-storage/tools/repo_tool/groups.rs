//! Consumer selection over the canonical shared release-group policy.
#[cfg_attr(
    not(test),
    expect(
        clippy::wildcard_imports,
        reason = "Private release submodules share the parent helper vocabulary."
    )
)]
use super::*;

pub(super) const GROUP_POLICY: &str = "ci/cargo-release-groups.json";

///
/// `ReleaseGroup`
///
/// Consumer metadata selected from the canonical Shared group policy.
///

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ReleaseGroup {
    pub name: String,
    pub primary: String,
    pub version: String,
    pub members: Vec<String>,
    pub changelog: String,
    pub dependencies: Vec<String>,
    #[serde(default)]
    pub metadata_files: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReleaseGroups {
    schema: u32,
    groups: Vec<ReleaseGroup>,
    #[serde(default)]
    shared_metadata_files: Vec<String>,
}

impl<E: Execute> Repository<E> {
    pub(super) fn release_groups(&self, revision: Option<&str>) -> Result<Vec<ReleaseGroup>> {
        let mut policy: ReleaseGroups = serde_json::from_str(&self.text(GROUP_POLICY, revision)?)?;
        require(policy.schema == 1, "unsupported release-group policy")?;
        for group in &mut policy.groups {
            group
                .metadata_files
                .splice(0..0, policy.shared_metadata_files.iter().cloned());
            let identifier = |s: &str| {
                !s.is_empty()
                    && s.bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
            };
            let relative = |s: &str| {
                !s.is_empty()
                    && !s.bytes().any(|b| b.is_ascii_control())
                    && Path::new(s)
                        .components()
                        .all(|c| matches!(c, std::path::Component::Normal(_)))
            };
            require(
                identifier(&group.name)
                    && identifier(&group.primary)
                    && group.members.iter().all(|name| identifier(name))
                    && group.members.contains(&group.primary)
                    && group.version == "package"
                    && relative(&group.changelog)
                    && group.metadata_files.iter().all(|path| relative(path)),
                "invalid consumer release group",
            )?;
        }
        Ok(policy.groups)
    }

    pub(super) fn release_group(&self, revision: Option<&str>) -> Result<ReleaseGroup> {
        require(!self.group.is_empty(), "select RELEASE_GROUP explicitly")?;
        self.release_groups(revision)?
            .into_iter()
            .find(|group| group.name == self.group)
            .ok_or_else(|| "unknown selected release group".into())
    }

    pub(super) fn release_files(&self, revision: Option<&str>) -> Result<Vec<String>> {
        Ok(self.release_group(revision)?.files())
    }

    pub(super) fn selected_packages(&self) -> Result<Vec<String>> {
        Ok(self.release_group(None)?.members)
    }

    pub(super) fn check_lock_update(
        &self,
        before: &[u8],
        after: &str,
        previous: &str,
        version: &str,
    ) -> Result<()> {
        let group = self.release_group(None)?;
        group.check_lock_update(before, after, previous, version)
    }
}

#[derive(Clone, Copy)]
pub(super) enum SurfaceRole {
    Catalog,
    Lock,
    Member,
    Notes,
    Preserved,
}

impl ReleaseGroup {
    pub(super) fn files(&self) -> Vec<String> {
        self.surfaces().into_iter().map(|(path, _)| path).collect()
    }

    pub(super) fn surfaces(&self) -> Vec<(String, SurfaceRole)> {
        let mut paths = vec![
            ("Cargo.toml".to_owned(), SurfaceRole::Catalog),
            ("Cargo.lock".to_owned(), SurfaceRole::Lock),
            (self.changelog.clone(), SurfaceRole::Notes),
        ];
        paths.extend(
            self.members
                .iter()
                .map(|name| (package_manifest(name), SurfaceRole::Member)),
        );
        paths.extend(self.metadata_files.iter().map(|path| {
            // Independent fixtures consume this checkout's Storage and Logging versions.
            // Only explicitly declared catalog/lock inputs share group projection;
            // unrelated metadata, including other Cargo manifests, stays opaque.
            let role = if matches!(self.name.as_str(), "storage" | "logging-storage")
                && path == "qualification/Cargo.toml"
            {
                SurfaceRole::Catalog
            } else if matches!(self.name.as_str(), "storage" | "logging-storage")
                && path == "qualification/Cargo.lock"
            {
                SurfaceRole::Lock
            } else if Path::new(path).parent() == Path::new(&self.changelog).parent()
                && Path::new(path)
                    .extension()
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("md"))
            {
                SurfaceRole::Notes
            } else {
                SurfaceRole::Preserved
            };
            (path.clone(), role)
        }));
        paths
    }

    pub(super) fn check_lock_update(
        &self,
        before: &[u8],
        after: &str,
        previous: &str,
        version: &str,
    ) -> Result<()> {
        verify_workspace_release_projection(
            std::str::from_utf8(before)?,
            after,
            &self
                .members
                .iter()
                .map(|package| WorkspaceVersionProjection {
                    package,
                    previous,
                    next: version,
                })
                .collect::<Vec<_>>(),
        )?;
        Ok(())
    }
}
