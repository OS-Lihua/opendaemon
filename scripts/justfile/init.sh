#!/usr/bin/env sh
set -eu

usage() {
    cat <<'EOF'
usage:
  init.sh --component <tools...>
  init.sh --install <crates...>

examples:
  ./scripts/justfile/init.sh --component rust-analyzer clippy rustfmt
  ./scripts/justfile/init.sh --install prek cargo-nextest
EOF
}

need_cmd() {
    command -v "$1" >/dev/null 2>&1 || {
        printf "error: missing required command: %s\n" "$1" >&2
        exit 1
    }
}

tool_command_candidates() {
    case "$1" in
        clippy)
            printf '%s\n' cargo-clippy clippy-driver
            ;;
        rustfmt)
            printf '%s\n' rustfmt
            ;;
        rust-analyzer)
            printf '%s\n' rust-analyzer
            ;;
        wasm32-unknown-unknown)
            printf '%s\n' ''
            ;;
        *)
            printf '%s\n' "$1"
            ;;
    esac
}

brew_formula_for_tool() {
    case "$1" in
        clippy)
            printf '%s\n' clippy
            ;;
        rust-analyzer)
            printf '%s\n' rust-analyzer
            ;;
        rustfmt)
            printf '%s\n' rust
            ;;
        trunk)
            printf '%s\n' trunk
            ;;
        wasm-bindgen)
            printf '%s\n' wasm-bindgen
            ;;
        *)
            printf '%s\n' "$1"
            ;;
    esac
}

has_any_command() {
    for command_name in "$@"; do
        [ -n "$command_name" ] || continue
        command -v "$command_name" >/dev/null 2>&1 && return 0
    done
    return 1
}

target_is_available() {
    rustc --print target-list 2>/dev/null | grep -qx "$1"
}

ensure_tool() {
    tool="$1"

    if [ "$tool" = "wasm32-unknown-unknown" ]; then
        if target_is_available "$tool"; then
            return 0
        fi
        printf "error: missing Rust target: %s\n" "$tool" >&2
        printf "install it with: rustup target add %s\n" "$tool" >&2
        printf "note: Homebrew Rust does not manage additional Rust std targets reliably; rustup is the supported fallback for this target.\n" >&2
        exit 1
    fi

    # shellcheck disable=SC2046
    if has_any_command $(tool_command_candidates "$tool"); then
        return 0
    fi

    printf "error: missing required tool: %s\n" "$tool" >&2
    if command -v brew >/dev/null 2>&1; then
        printf "install it with: brew install %s\n" "$(brew_formula_for_tool "$tool")" >&2
    elif command -v rustup >/dev/null 2>&1; then
        printf "install it with: rustup component add %s\n" "$tool" >&2
    fi
    exit 1
}

ensure_cargo_tool() {
    crate="$1"
    binary="$crate"

    command -v "$binary" >/dev/null 2>&1 && return 0

    if command -v cargo-binstall >/dev/null 2>&1; then
        cargo binstall "$crate" 2>/dev/null || cargo install --locked "$crate"
    else
        cargo install --locked "$crate"
    fi
}

if [ $# -eq 0 ]; then
    usage
    exit 2
fi

while [ $# -gt 0 ]; do
    case "$1" in
        -h | --help)
            usage
            exit 0
            ;;
        --component)
            shift
            if [ $# -eq 0 ] || [ "${1#--}" != "$1" ]; then
                printf "error: --component requires at least 1 value\n" >&2
                exit 2
            fi

            tools=""
            while [ $# -gt 0 ] && [ "${1#--}" = "$1" ]; do
                tools="$tools $1"
                shift
            done

            for tool in $tools; do
                ensure_tool "$tool"
            done
            ;;
        --install)
            shift
            if [ $# -eq 0 ] || [ "${1#--}" != "$1" ]; then
                printf "error: --install requires at least 1 value\n" >&2
                exit 2
            fi

            need_cmd cargo
            crates=""
            while [ $# -gt 0 ] && [ "${1#--}" = "$1" ]; do
                crates="$crates $1"
                shift
            done

            for crate in $crates; do
                ensure_cargo_tool "$crate"
            done
            ;;
        *)
            printf "error: unknown argument: %s\n" "$1" >&2
            usage >&2
            exit 2
            ;;
    esac
done
