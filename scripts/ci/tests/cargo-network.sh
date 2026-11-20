#!/usr/bin/env bash
# Exercise the product and qualification recipes without consulting a registry.
set -euo pipefail
cd "$(dirname "$0")/../../.."
fixture=$(mktemp -d)
trap 'rm -rf "$fixture"' EXIT

cat > "$fixture/cargo" <<'CARGO'
#!/usr/bin/env bash
set -euo pipefail
[[ ${CARGO_NET_OFFLINE-unset} == "$expected_mode" ]]
case " $* " in
  *' --offline '*|*' --frozen '*) exit 1 ;;
esac
[[ " $* " == *' --locked '* ]]
echo "$1" >> "$calls"
CARGO
chmod +x "$fixture/cargo"
export PATH="$fixture:$PATH" calls="$fixture/calls"
for expected_mode in unset true false; do
    export expected_mode
    unset CARGO_NET_OFFLINE
    if [[ $expected_mode != unset ]]; then export CARGO_NET_OFFLINE=$expected_mode; fi
    : > "$calls"
    make check storage-build
    [[ $(cat "$calls") == $'check\nbuild' ]]
done
