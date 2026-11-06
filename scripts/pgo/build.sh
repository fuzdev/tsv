#!/bin/sh
# The cargo half of the PGO pipeline: the one place the profile-guided builds'
# RUSTFLAGS are spelled. POSIX sh, because it runs wherever the release matrix
# builds — the almalinux:8 and rust:alpine containers (no Node, no Deno), the
# native mac and Windows runners — and locally, called by `scripts/pgo/pgo.ts all`.
# Everything that is not a cargo build (training, the merge, the gates) is
# scripts/pgo/pgo.ts.
#
# usage: sh scripts/pgo/build.sh <instrument|use> <pgo-dir> [<rust-target>]
#
# The worlds built are `PGO_WORLDS` (space-separated, default `cli napi` — the two shipped
# binaries); `pgo.ts --ffi` adds `ffi`, the bench's C-FFI library (`tsv_ffi`, `release`).
#
#   instrument  each world (tsv_cli `release`, tsv_napi `napi`, tsv_ffi `release`),
#               -Cprofile-generate, into
#               <pgo-dir>/target-instrument. The build scripts it runs are
#               instrumented too (RUSTFLAGS reaches host artifacts when host ==
#               target), so their profiles go to <pgo-dir>/build-profraw — a
#               default the training run overrides with LLVM_PROFILE_FILE, which
#               keeps them out of the merge. The instrumenting rustc's `-vV` goes
#               to <pgo-dir>/logs/rustc-instrument.txt: the merge must run in that
#               compiler's LLVM, and on a container row it is the container's.
#   use         the same worlds, -Cprofile-use with each world's own profile
#               (<pgo-dir>/<world>.profdata), into
#               <pgo-dir>/target-use, verbose (`-v`), so the log carries each
#               rustc command line for the gate to read the flag back from. The
#               dir is DELETED first: cargo fingerprints the RUSTFLAGS text, not
#               the profile's bytes, so a kept dir would serve the last profile's
#               codegen under this one's name.
#
# `-inline-enable-cost-benefit-analysis=false` turns off LLVM's profile-driven
# cost-benefit inliner, which otherwise inlines hot call sites past every
# threshold — most of the binary-size cost of a default PGO build for little of
# its speed. A silently ignored flag shows as a size jump at the size gate
# (`validate_napi_artifact.ts --pgo`). `-pgo-warn-missing-function` makes every
# function the profile does not cover a build warning, which the build-log gate
# reads (`pgo.ts gate-builds`); it does not change codegen. The instrumented builds
# pass `-pgo-instrument-entry`, which puts a counter on every function's entry so
# the profile's first count per function IS its call count — what the merge's
# non-empty gate grades (`pgo.ts merge`); without it the first counter is whichever
# edge the spanning tree left uncounted. The use build reads the choice from the
# profile's header, so it needs no matching flag. Cargo's colour is off
# (`CARGO_TERM_COLOR=never`), so the logs that gate reads are plain text.
#
# Any RUSTFLAGS already set are kept in front (the musl row's
# `-C target-feature=-crt-static`). Each build's output goes to
# <pgo-dir>/logs/build-<phase>-<world>.log; a failure prints its tail.

set -eu

phase=${1:?usage: build.sh <instrument|use> <pgo-dir> [<rust-target>]}
dir=${2:?usage: build.sh <instrument|use> <pgo-dir> [<rust-target>]}
target=${3:-}

mkdir -p "$dir/logs"
# An absolute path rustc can read: on Windows (Git Bash) `pwd -W` gives the
# native `D:/a/...` spelling, which rustc and LLVM take where `/d/a/...` fails.
if pwd -W >/dev/null 2>&1; then
	dir=$(cd "$dir" && pwd -W)
else
	dir=$(cd "$dir" && pwd)
fi
case $dir in
*' '*)
	echo "build.sh: the PGO dir '$dir' contains a space, which RUSTFLAGS cannot carry" >&2
	exit 2
	;;
esac

worlds=${PGO_WORLDS:-cli napi}
for world in $worlds; do
	case $world in
	cli | napi | ffi) ;;
	*)
		echo "build.sh: unknown world '$world' in PGO_WORLDS (cli, napi, ffi)" >&2
		exit 2
		;;
	esac
done

base=${RUSTFLAGS:-}
export CARGO_TERM_COLOR=never
target_args=
if [ -n "$target" ]; then target_args="--target $target"; fi

use_flags() {
	echo "-Cprofile-use=$dir/$1.profdata -Cllvm-args=-pgo-warn-missing-function -Cllvm-args=-inline-enable-cost-benefit-analysis=false"
}

# run <log-name> <flags> <cargo args...>
run() {
	name=$1
	log="$dir/logs/build-$name.log"
	flags=$2
	shift 2
	echo "build.sh: cargo $* (RUSTFLAGS: ${base:+$base }$flags)"
	started=$(date +%s)
	# shellcheck disable=SC2086 # target_args splits into its flag and value on purpose
	if RUSTFLAGS="${base:+$base }$flags" cargo "$@" $target_args >"$log" 2>&1; then
		grep -v '^ *Running `' "$log" | tail -n 2
		echo "build.sh: $name took $(($(date +%s) - started)) s"
	else
		tail -n 80 "$log" >&2
		echo "build.sh: FAILED — full log $log" >&2
		exit 1
	fi
}

# The cargo selection of one world: the package and its profile.
world_args() {
	case $1 in
	cli) echo "-p tsv_cli --release" ;;
	napi) echo "-p tsv_napi --profile napi" ;;
	ffi) echo "-p tsv_ffi --release" ;;
	esac
}

case $phase in
instrument)
	export CARGO_TARGET_DIR="$dir/target-instrument"
	gen="-Cprofile-generate=$dir/build-profraw -Cllvm-args=-pgo-instrument-entry"
	rustc -vV >"$dir/logs/rustc-instrument.txt"
	for world in $worlds; do
		# shellcheck disable=SC2046 # world_args splits into cargo's arguments on purpose
		run "instrument-$world" "$gen" build --locked $(world_args "$world")
	done
	;;
use)
	for world in $worlds; do
		if [ ! -s "$dir/$world.profdata" ]; then
			echo "build.sh: $dir/$world.profdata is missing or empty — train and merge first" >&2
			exit 1
		fi
	done
	rm -rf "$dir/target-use"
	export CARGO_TARGET_DIR="$dir/target-use"
	for world in $worlds; do
		# shellcheck disable=SC2046 # world_args splits into cargo's arguments on purpose
		run "use-$world" "$(use_flags "$world")" build -v --locked $(world_args "$world")
	done
	;;
*)
	echo "build.sh: unknown phase '$phase' (instrument, use)" >&2
	exit 2
	;;
esac
