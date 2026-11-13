//! Registry reconciliation and coordinated publication.
#[cfg_attr(
    not(test),
    expect(
        clippy::wildcard_imports,
        reason = "Private release submodules share the parent helper vocabulary."
    )
)]
use super::*;

// The registry may add fields. These required facts still have exact types and
// must match the independently qualified package before it counts as published.
#[derive(Deserialize)]
struct RegistryResponse {
    version: RegistryVersion,
}

#[derive(Deserialize)]
struct RegistryVersion {
    #[serde(rename = "crate")]
    name: String,
    num: String,
    yanked: bool,
    checksum: String,
}

fn registry_response(response: &str, name: &str, version: &str, digest: &str) -> Result<bool> {
    let (body, status) = response
        .rsplit_once('\n')
        .ok_or("missing registry HTTP status")?;
    if status == "404" {
        return Ok(false);
    }
    require(
        status == "200",
        &format!(
            "registry observation failed for {name} {version} (HTTP {status}); publication was not retried"
        ),
    )?;
    let record: RegistryResponse = serde_json::from_str(body)?;
    let record = record.version;
    require(
        record.name == name && record.num == version && !record.yanked && record.checksum == digest,
        "published package differs from the qualified archive or is yanked",
    )?;
    Ok(true)
}

impl<E: Execute> Repository<E> {
    // Re-observe both identities at each publication checkpoint; never cache
    // admission across the registry observation or upload effect.
    fn check_publication_identity(
        &self,
        release: &InspectedRelease,
        tag: &str,
        timing: &str,
    ) -> Result<()> {
        require(
            self.release_commit()?.0 == *release,
            &format!("release changed {timing} publication"),
        )?;
        require(
            self.check_tag(&release.selection.version, &release.commit)? == tag,
            &format!("release tag changed {timing} publication"),
        )
    }

    fn registry_contains(&self, name: &str, version: &str, digest: &str) -> Result<bool> {
        require(
            self.selected_packages()?.iter().any(|p| p == name),
            "unknown publication package",
        )?;
        self.version_parts(version)?;
        let response = self.output(
            "curl",
            &[
                "--silent",
                "--show-error",
                "--location",
                "--user-agent",
                "ic-storage-release (https://github.com/dragginzgame/ic-storage)",
                "--max-time",
                "60",
                "--write-out",
                "\n%{http_code}",
                &format!("https://crates.io/api/v1/crates/{name}/{version}"),
            ],
        )?;
        registry_response(&response, name, version, digest)
    }

    pub(super) fn publish(&self, dry_run: bool) -> Result<()> {
        self.release_group(None)?;
        let (release, evidence) = self.release_commit()?;
        let version = &release.selection.version;
        let head = &release.commit;
        let tag = self.check_tag(version, head)?;
        // Observe every package before any effect. A conflict in a later package
        // must not permit an earlier upload; dependencies publish first.
        let packages = evidence
            .archives()
            .map(|(name, digest)| {
                let published = !dry_run && self.registry_contains(name, version, digest)?;
                Ok((name, digest, published))
            })
            .collect::<Result<Vec<_>>>()?;
        let mut paths = None;
        for (name, digest, exists) in packages {
            self.check_publication_identity(&release, &tag, "before")?;
            if exists {
                println!("Already published and checksum verified: {name} {version}");
                continue;
            }
            if paths.is_none() {
                paths = Some(self.artifact_paths()?);
            }
            let parent = paths
                .as_ref()
                .expect("publication paths were admitted")
                .publication();
            fs::create_dir_all(&parent)?;
            let attempt = parent.join(format!(
                "{name}.{}.{}",
                std::process::id(),
                SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
            ));
            // Shared owns per-attempt locking, stdout/stderr and both Cargo and
            // logger statuses. This adapter owns package order and reconciliation.
            self.run(
                "env",
                &[
                    &format!("RUSTUP_TOOLCHAIN={}", self.toolchain()?),
                    "bash",
                    "vendor/toko-tooling/scripts/ci/run-cargo-publication.sh",
                    if dry_run { "check" } else { "publish" },
                    attempt.to_str().ok_or("non-UTF-8 publication path")?,
                    head,
                    "crates-io",
                    "--package",
                    &format!("{name}@{version}"),
                    "--no-verify",
                ],
            )?;
            self.check_publication_identity(&release, &tag, "during")?;
            if !dry_run {
                require(
                    self.registry_contains(name, version, digest)?,
                    "publication is not yet visible; rerun publish later to reconcile checksums before continuing",
                )?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
