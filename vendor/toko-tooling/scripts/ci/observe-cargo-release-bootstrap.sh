#!/usr/bin/env bash
# Shared companions: scripts/ci/cargo-release-groups.jq scripts/ci/release-metadata-transaction.sh scripts/ci/validation-input-checkpoint.pl
# Source-only observation; no repair, rendering, fetching or release authority.
set -euo pipefail
[[ $# == 1 && ( "$1" == policy || "$1" == observe ) ]] || {
    echo 'usage: observe-cargo-release-bootstrap.sh policy|observe' >&2; exit 2;
}
case "$PWD:${BASH_SOURCE[0]}" in *$'\r'*|*$'\n'*|*$'\t'*) echo 'invalid bootstrap directory' >&2; exit 1 ;; esac
script_dir="$(cd -P "${BASH_SOURCE[0]%/*}" && printf '%s/.' "$PWD")"
script_dir="${script_dir%/.}"
root="$(pwd -P && printf '.')"
root="${root%$'\n.'}"
case "$root:$script_dir" in *$'\r'*|*$'\n'*|*$'\t'*) echo 'invalid bootstrap directory' >&2; exit 1 ;; esac
# shellcheck source=scripts/ci/release-metadata-transaction.sh
source "$script_dir/release-metadata-transaction.sh"
temporary="$(mktemp -d "${TMPDIR:-/tmp}/cargo-bootstrap.XXXXXX")"
complete=false
finish() {
    local status=$?
    [[ "$complete" == true || "$status" != 0 ]] || status=1
    if [[ "$complete" == true && "$status" == 0 ]]; then rm -rf -- "$temporary"
    else printf 'Bootstrap observation evidence retained: %s\n' "$temporary" >&2; fi
    exit "$status"
}
trap finish EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
files=()
snapshot_input() {
    local path="$1"
    release_metadata_regular "$root" "$path" || return 1
    if [[ "$path" == */* ]]; then mkdir -p "$temporary/inputs/${path%/*}" || return; fi
    cp -p -- "$root/$path" "$temporary/inputs/$path" || return
    files+=("$path")
}
snapshot_input ci/cargo-release-groups.json
jq -e -L "$script_dir" 'include "cargo-release-groups"; release_policy' \
    "$temporary/inputs/ci/cargo-release-groups.json" > "$temporary/policy.json"
if [[ "$1" == policy ]]; then
    release_metadata_regular "$root" ci/cargo-release-groups.json
    cmp -s -- ci/cargo-release-groups.json "$temporary/inputs/ci/cargo-release-groups.json"
    cat "$temporary/policy.json"
    complete=true
    exit 0
fi
snapshot_input Cargo.toml
snapshot_input Cargo.lock
# Cargo owns workspace discovery without resolving the incoherent local lock.
# Both interruption orders may be observed. The caller owns exact input custody
# and must re-observe after helpers/effects; this output is never a cached proof.
RUSTUP_AUTO_INSTALL=0 CARGO_NET_OFFLINE=true cargo metadata --locked --no-deps --offline --format-version 1 > "$temporary/metadata.json"
"${YQ:-yq}" -p toml -o json '.' "$temporary/inputs/Cargo.toml" > "$temporary/catalog.json"
"${YQ:-yq}" -p toml -o json '.' "$temporary/inputs/Cargo.lock" > "$temporary/lock.json"
jq -er '. as $m | .packages[] | select(.id as $id | $m.workspace_members | index($id)) |
    [.name,.manifest_path] | @tsv' "$temporary/metadata.json" > "$temporary/members"
printf '{}\n' > "$temporary/manifests.json"
while IFS=$'\t' read -r package manifest; do
    case "$manifest" in "$root"/*) path="${manifest#"$root"/}" ;; *) echo 'bootstrap member outside workspace' >&2; exit 1 ;; esac
    snapshot_input "$path"
    "${YQ:-yq}" -p toml -o json '.' "$temporary/inputs/$path" > "$temporary/member.json"
    jq --arg package "$package" --slurpfile member "$temporary/member.json" '. + {($package):$member[0]}' \
        "$temporary/manifests.json" > "$temporary/next.json"
    mv "$temporary/next.json" "$temporary/manifests.json"
done < "$temporary/members"
jq -n -e -L "$script_dir" --slurpfile policies "$temporary/policy.json" --slurpfile metadatas "$temporary/metadata.json" \
    --slurpfile catalogs "$temporary/catalog.json" --slurpfile locks "$temporary/lock.json" --slurpfile manifests "$temporary/manifests.json" '
  include "cargo-release-groups";
  $policies[0] as $policy | $metadatas[0] as $metadata | $catalogs[0] as $catalog | $locks[0] as $lock |
  $metadata as $m |
  [$m.packages[] | select(.id as $id | $m.workspace_members | index($id))] as $owned |
  if ($catalog | has("package")) or ($catalog.workspace | type) != "object" or
     $catalog.workspace.package.version != null or
     any($policy.groups[]; .version != "package") or
     ($owned | length) == 0 or
     ($owned | map(.id) | sort) != ($m.workspace_members | sort) or
     ($owned | map(.name) | sort) != ([$policy.groups[].members[]] | sort) or
     any($owned[]; .source != null or (.version | stable | not) or $manifests[0][.name].package.version != .version)
  then error("bootstrap observation requires a virtual package-version workspace") else . end |
  [$lock.package[] | select(.source == null)] as $local |
  if ($lock.version != 3 and $lock.version != 4) or
     ($local | map(.name) | sort) != ($owned | map(.name) | sort) or
     any($local[]; (.version | stable | not))
  then error("invalid bootstrap local lock identities") else . end |
  member_catalog($catalog; $owned) as $dependencies |
  if any($dependencies[]; (.value.path | type) != "string" or (.value.path | length) == 0 or
       (.value.path | test("[\\r\\n\\t]")) or (.value.version | type) != "string" or
       (.value.version | sub("^[~^=]?[ \t]*"; "") | stable | not)) or
     any($policy.groups[]; . as $g |
       ([$dependencies[] | select(.package as $n | $g.members | index($n)) | .key] | sort) != ($g.dependencies | sort))
  then error("invalid bootstrap member dependency catalog") else . end |
  {version:"unchanged", dependencies:[$dependencies[] as $d |
      $local[] | select(.name == $d.package) |
      .version as $locked |
      {alias:$d.key, previous:$d.value.version,
       next:($d.value.version | capture("^(?<prefix>[~^=]?[ \\t]*).*$") | .prefix + $locked)}]} as $catalog_edit |
  [$policy.groups[] as $g |
    [$g.members[] as $name |
      ($owned | map(select(.name == $name))[0]) as $p |
      ($local | map(select(.name == $name))[0]) as $l |
      if ($p.manifest_path | startswith($m.workspace_root + "/") | not)
      then error("bootstrap member outside workspace") else
      {name:$name, manifest:$p.manifest_path, declared:$p.version, locked:$l.version} end] as $members |
    if ($members | map(.locked) | unique | length) != 1
    then error("bootstrap group lock versions differ")
    else {name:$g.name, primary:$g.primary, members:$members} end] as $groups |
  {schema:1, workspace_root:$m.workspace_root, target_directory:$m.target_directory,
   coherent:(all($groups[].members[]; .declared == .locked) and all($catalog_edit.dependencies[];
     .previous == .next)), groups:$groups, catalog_edit:$catalog_edit}' > "$temporary/result.json"
# The complete catalog includes unused aliases; Cargo does not validate them.
jq -r --slurpfile result "$temporary/result.json" '
    [$result[0].groups[].members[].name] as $owned |
    .workspace.dependencies | to_entries[] | select((.value | type) == "object") |
    select((.value.package // .key) as $name | $owned | index($name)) |
    [.key, (.value.package // .key), .value.path] | @tsv' "$temporary/catalog.json" > "$temporary/catalog-paths"
while IFS=$'\t' read -r alias package path; do
    release_metadata_regular "$root" "$path/Cargo.toml"
    manifest="$(jq -er --arg name "$package" '.groups[].members[] | select(.name == $name) | .manifest' "$temporary/result.json")"
    canonical="$(cd -P "$root/$path" && printf '%s/Cargo.toml.' "$PWD")"
    canonical="${canonical%.}"
    [[ "$canonical" == "$manifest" ]] || { echo "bootstrap catalog path does not own $alias" >&2; exit 1; }
done < "$temporary/catalog-paths"
checksum="$script_dir/verify-file-checksum.sh"
perl "$script_dir/validation-input-checkpoint.pl" "$temporary/inputs" '' "$temporary/inputs.json" "$checksum" "${files[@]}"
perl "$script_dir/validation-input-checkpoint.pl" "$root" "$temporary/inputs.json" "$temporary/final-inputs.json" "$checksum"
jq --slurpfile inputs "$temporary/inputs.json" '. + {inputs:$inputs[0]}' "$temporary/result.json"
complete=true
