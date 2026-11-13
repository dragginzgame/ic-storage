#!/usr/bin/env bash
# Caller-owned Makefiles, target rosters and effect stubs; no real release effects.
make_probe_target_admission() {
    [[ $# -ge 5 ]] || return 2
    local executable="$1" makefile="$2" directory="$3" events="$4"
    shift 4
    local target mode status
    [[ -f "$makefile" && -d "$directory" ]] || return 2
    # Consumer targets exercise each unsafe behavior once. The producer owns
    # aliases, inherited/replaced flags and hidden-MFLAGS combinations.
    for target in "$@"; do
        for mode in -i -n -t -q; do
            : > "$events" || return
            status=0
            "$executable" --no-print-directory -f "$makefile" "$mode" "$target" \
                > "$directory/output" 2>&1 || status=$?
            if [[ "$status" != 2 || -s "$events" ]]; then
                printf 'Make target admission failed: %s %s (status %s)\n' "$target" "$mode" "$status" >&2
                return 1
            fi
        done
    done
}

make_probe_parallel_dispatch() {
    [[ $# -ge 5 ]] || return 2
    local executable="$1" makefile="$2" directory="$3" events="$4" target
    shift 4
    local parallel=(-j4)
    if "$executable" --help | grep -q -- --jobserver-style; then parallel+=(--jobserver-style=pipe); fi
    for target in "$@"; do
        : > "$events" || return
        "$executable" "${parallel[@]}" --no-print-directory -f "$makefile" "$target" \
            > "$directory/output" 2>&1 || return
        [[ -s "$events" ]] || return 1
    done
}

# Caller stubs the actual lint/effect leaves, recording ordered lines through
# MAKE_PROBE_EVENTS and returning MAKE_PROBE_LINT_STATUS from the lint leaf.
# Expected files describe the caller's full prefix and successful pipeline.
make_probe_lint_order() {
    [[ $# == 7 ]] || return 2
    local executable="$1" makefile="$2" directory="$3" events="$4"
    local target="$5" failure_events="$6" success_events="$7" lint_status status expected
    [[ -f "$makefile" && -d "$directory" && -f "$failure_events" && -f "$success_events" ]] || return 2
    for lint_status in 23 0; do
        : > "$events" || return
        status=0
        MAKE_PROBE_EVENTS="$events" MAKE_PROBE_LINT_STATUS="$lint_status" \
            "$executable" -j4 --no-print-directory -f "$makefile" "$target" \
            > "$directory/lint-$lint_status.log" 2>&1 || status=$?
        expected="$failure_events"
        if [[ "$lint_status" == 0 ]]; then
            [[ "$status" == 0 ]] || return 1
            expected="$success_events"
        else
            [[ "$status" != 0 ]] || return 1
        fi
        cmp "$expected" "$events" || return 1
    done
}
