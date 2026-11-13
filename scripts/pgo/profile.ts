/**
 * The pure readers behind the PGO pipeline's gates (`scripts/pgo/pgo.ts`): the
 * LLVM version a tool reports, the per-function counts `llvm-profdata show`
 * prints, the profile diagnostics a `-Cprofile-use` build logs, and the
 * instrumentation sections `llvm-readobj --sections` lists. Text in, verdicts
 * out — no process, no file — so `scripts/pgo/profile_test.ts` grades each one
 * against captured tool output.
 *
 * Node builtins only (none, in fact), so the pipeline runs under Node as well as
 * Deno: the musl row trains and compares inside a Node-only container.
 *
 * @module
 */

/** What one world's training ran, by input class — the floors `grade_profile` holds the hot
 * functions to. */
export interface TrainedCounts {
	/** Every file trained (each is parsed and formatted). */
	files: number;
	typescript: number;
	svelte: number;
	css: number;
	/** TypeScript files the type strip ran on. */
	strip: number;
	/** Files the import records ran on, per language (the addon's alone: the CLI has none). */
	imports_typescript: number;
	imports_svelte: number;
}

/** A function a trained profile must carry real counts on. */
export interface HotFunction {
	/** Its demangled path. */
	path: string;
	/** The trained count its floor is read from (`floor_of`). */
	per: keyof TrainedCounts;
	/** The worlds that compile it (the CLI has no import records; the FFI library neither
	 * those nor the type strip). */
	worlds: readonly ProfileWorld[];
}

/** A training world, as `layout.ts` names them (restated: this module imports nothing). */
export type ProfileWorld = 'cli' | 'napi' | 'ffi';

const EVERY = ['cli', 'napi', 'ffi'] as const;

/**
 * The functions a trained profile must carry real counts on, by demangled path: the doc
 * renderer every format walks, each language's two parse entries — `parse_folded_into`, or
 * `parse_folded` where a language has no stores (the parse a format runs) and the parse the wire-JSON path runs — the type strip, and the
 * import records' entries. A profile that misses any of them trained nothing that
 * matters, however many records it holds: "zero hash mismatches" passes VACUOUSLY on an
 * empty profile, since every function then reads as missing rather than mismatched. Each
 * is held to a floor read from the inputs of its class it trained on (`floor_of`). A
 * rename here is a rename in the gate (the gate fails, naming the path).
 */
export const HOT_FUNCTIONS: readonly HotFunction[] = [
	{ path: 'tsv_lang::doc::arena_render::render_doc_iterative', per: 'files', worlds: EVERY },
	{ path: 'tsv_ts::__private::parse_with_goal', per: 'typescript', worlds: ['cli'] },
	{ path: 'tsv_ts::__private::parse_with_goal_into', per: 'typescript', worlds: ['napi', 'ffi'] },
	{ path: 'tsv_ts::__private::parse_folded_into', per: 'typescript', worlds: EVERY },
	{ path: 'tsv_svelte::__private::parse', per: 'svelte', worlds: ['cli'] },
	{ path: 'tsv_svelte::__private::parse_into', per: 'svelte', worlds: ['napi', 'ffi'] },
	{ path: 'tsv_svelte::__private::parse_folded_into', per: 'svelte', worlds: EVERY },
	{ path: 'tsv_css::__private::parse', per: 'css', worlds: EVERY },
	{ path: 'tsv_css::__private::parse_folded', per: 'css', worlds: EVERY },
	{ path: 'tsv_ts::strip::strip_into', per: 'strip', worlds: ['cli', 'napi'] },
	{ path: 'tsv_ts::imports::parse_imports_into', per: 'imports_typescript', worlds: ['napi'] },
	{ path: 'tsv_svelte::__private::parse_imports_into', per: 'imports_svelte', worlds: ['napi'] }
];

/**
 * A hot function's floor: every input of its class the training ran. The count graded is
 * the function's ENTRY count — how many times it was called — which every input of its
 * class adds at least one to (each is parsed, formatted, stripped or recorded through it),
 * however it leaves. A block counter could not be held to it: IR instrumentation counts
 * edges, so a counter past a loop or a pre-inlined callee reads far more than the calls,
 * and one past an early exit fewer. A function graded by the entry floor refuses a training
 * that missed any input of its class, which a non-zero count would pass.
 */
export const floor_of = (h: HotFunction, trained: TrainedCounts): number =>
	Math.max(1, trained[h.per]);

/** The `x.y.z` LLVM version in a tool's `--version` text (`LLVM version 21.1.8-rust-…`)
 * or in `rustc -vV`'s `LLVM version: 21.1.8` line, or `null` when it names none. */
export function llvm_version(text: string): string | null {
	const m = /LLVM version:?\s+(\d+\.\d+\.\d+)/.exec(text);
	return m ? m[1]! : null;
}

/**
 * The `::`-joined path of a legacy-mangled Rust symbol (`_ZN6tsv_ts9__private5parse17h…E`
 * → `tsv_ts::__private::parse`), its trailing `h<hash>` component dropped, or `null`
 * when the name holds no legacy symbol. A local symbol's `file;` prefix and Mach-O's
 * extra leading underscore are skipped (the symbol starts at its first `_ZN`).
 * Generic arguments stay in their escaped spelling — only the plain paths
 * `HOT_FUNCTIONS` names are compared.
 */
export function demangle_legacy_path(name: string): string | null {
	const start = name.indexOf('_ZN');
	if (start === -1) return null;
	const parts: string[] = [];
	let i = start + 3;
	while (i < name.length && name[i] !== 'E') {
		const digits = /^\d+/.exec(name.slice(i));
		if (!digits) return null;
		const len = Number(digits[0]);
		i += digits[0].length;
		if (len === 0 || i + len > name.length) return null;
		parts.push(name.slice(i, i + len));
		i += len;
	}
	if (name[i] !== 'E' || parts.length === 0) return null;
	if (/^h[0-9a-f]{16}$/.test(parts[parts.length - 1]!)) parts.pop();
	return parts.join('::');
}

/** One function record of `llvm-profdata show --all-functions --counts`. */
export interface ProfileRecord {
	/** The record's name as the profile stores it (mangled, maybe `file;`-prefixed). */
	name: string;
	/** The largest of its function count and block counts — zero when it never ran. */
	max_count: number;
	/**
	 * How many times the function was entered, or `null` when the profile cannot say: a
	 * front-end profile prints it (`Function count`); an IR profile counts it in its first
	 * counter only when it instruments every function's entry (`entry_first = 1`, the
	 * `-pgo-instrument-entry` the instrumented build passes) — otherwise the first counter is
	 * whichever edge the spanning tree left uncounted first, not the entry.
	 */
	entry_count: number | null;
}

/** What `llvm-profdata show` says about how the profile was instrumented: `ir` and
 * `entry_first` from its `Instrumentation level: IR  entry_first = 1` line, `null` when the
 * line is absent. */
export function profile_instrumentation(
	text: string
): { ir: boolean; entry_first: boolean } | null {
	const m = /^Instrumentation level: (\S+)(?:\s+entry_first = (\d))?/m.exec(text);
	return m ? { ir: m[1] === 'IR', entry_first: m[2] === '1' } : null;
}

/** Every function record in `llvm-profdata show --all-functions --counts` output. */
export function parse_profile_show(text: string): ProfileRecord[] {
	const entry_first = profile_instrumentation(text)?.entry_first ?? false;
	const records: ProfileRecord[] = [];
	let current: ProfileRecord | null = null;
	for (const line of text.split(/\r?\n/)) {
		const name = /^ {2}(\S.*):$/.exec(line);
		if (name) {
			current = { name: name[1]!, max_count: 0, entry_count: null };
			records.push(current);
			continue;
		}
		if (!current) continue;
		const fn_count = /^ {4}Function count: (\d+)/.exec(line);
		if (fn_count) {
			current.entry_count = Number(fn_count[1]);
			current.max_count = Math.max(current.max_count, current.entry_count);
		}
		const blocks = /^ {4}Block counts: \[(.*)\]/.exec(line);
		if (blocks) {
			const counts = blocks[1]!
				.split(',')
				.map((v) => v.trim())
				.filter((v) => v !== '')
				.map(Number);
			for (const n of counts) if (n > current.max_count) current.max_count = n;
			if (entry_first && current.entry_count === null && counts.length > 0) {
				current.entry_count = counts[0]!;
			}
		}
	}
	return records;
}

/** What the non-empty-profile gate found in one profile. */
export interface ProfileVerdict {
	records: number;
	/** Records with any non-zero count. */
	executed: number;
	/** Each of the world's hot functions with its entry count summed over its records and its
	 * largest count over them (0 when absent), and the floor the entry count is held to. */
	hot: {
		path: string;
		per: keyof TrainedCounts;
		entry_count: number;
		max_count: number;
		floor: number;
		found: boolean;
	}[];
	/** The gate's failures, empty when it passes. */
	failures: string[];
}

/** Grade one world's profile `show` output: a profile that counts function entries, and
 * every hot function the world ships entered at least its floor's times (`floor_of`). */
export function grade_profile(
	text: string,
	world: ProfileWorld,
	trained: TrainedCounts
): ProfileVerdict {
	const records = parse_profile_show(text);
	const by_path = new Map<string, { entry: number; max: number }>();
	for (const r of records) {
		const path = demangle_legacy_path(r.name);
		if (path === null) continue;
		const prev = by_path.get(path) ?? { entry: 0, max: 0 };
		by_path.set(path, {
			entry: prev.entry + (r.entry_count ?? 0),
			max: Math.max(prev.max, r.max_count)
		});
	}
	const hot = HOT_FUNCTIONS.filter((h) => h.worlds.includes(world)).map((h) => ({
		path: h.path,
		per: h.per,
		entry_count: by_path.get(h.path)?.entry ?? 0,
		max_count: by_path.get(h.path)?.max ?? 0,
		floor: floor_of(h, trained),
		found: by_path.has(h.path)
	}));
	const failures: string[] = [];
	if (records.length === 0) failures.push('the profile holds no function records');
	const instrumentation = profile_instrumentation(text);
	if (instrumentation?.ir && !instrumentation.entry_first) {
		failures.push(
			'the profile does not count function entries (entry_first = 0) — the instrumented ' +
				'build must pass -Cllvm-args=-pgo-instrument-entry (scripts/pgo/build.sh)'
		);
	} else if (records.length > 0 && records.every((r) => r.entry_count === null)) {
		failures.push('no record carries an entry count — the `show` output is not one this reads');
	}
	for (const h of hot) {
		if (!h.found) failures.push(`no record for ${h.path}`);
		else if (h.max_count === 0) failures.push(`${h.path} never ran in training (all counts 0)`);
		else if (h.entry_count < h.floor) {
			failures.push(
				`${h.path} was entered ${h.entry_count} times, under its floor of ${h.floor} (the ${trained[h.per]} inputs of its class trained)`
			);
		}
	}
	return {
		records: records.length,
		executed: records.filter((r) => r.max_count > 0).length,
		hot,
		failures
	};
}

/** One PGO diagnostic a `-Cprofile-use` build logged. */
export interface PgoDiagnostic {
	/** The LLVM module the warning names (`build_script_build.<hash>-cgu.0`, `tsv_ts.<hash>-cgu.0`). */
	module: string;
	/** The warning text after the module. */
	message: string;
	/** Whether it is a build script's — the build scripts never run in training, so they are always missing. */
	build_script: boolean;
}

/**
 * Every profile diagnostic in a `-Cprofile-use -Cllvm-args=-pgo-warn-missing-function`
 * build log: a function with no profile data, or one whose control flow changed
 * since training (a hash mismatch — the profile is stale and that function falls
 * back to no PGO, silently but for this line).
 */
export function parse_pgo_diagnostics(log: string): PgoDiagnostic[] {
	const out: PgoDiagnostic[] = [];
	for (const line of strip_ansi(log).split(/\r?\n/)) {
		const m =
			/^warning: ([^:\s]+): (.*(?:no profile data available|hash mismatch|profile).*)$/.exec(line);
		if (!m) continue;
		const module = m[1]!;
		out.push({ module, message: m[2]!, build_script: module.startsWith('build_script_build') });
	}
	return out;
}

/** The object format of a binary, from the `Format:` line `llvm-readobj` prints first. */
export type ObjectFormat = 'elf' | 'macho' | 'coff';

/** The object format `llvm-readobj` output names (`Format: elf64-x86-64`, `Format: Mach-O
 * 64-bit x86-64`, `Format: COFF-x86-64`), or `null` when it names none this reads. */
export function object_format(readobj: string): ObjectFormat | null {
	const name = /^Format: (\S+)/m.exec(readobj)?.[1]?.toLowerCase();
	if (!name) return null;
	if (name.startsWith('elf')) return 'elf';
	if (name.startsWith('mach-o')) return 'macho';
	if (name.startsWith('coff')) return 'coff';
	return null;
}

/**
 * Whether a format's linker drops a dead function's profile records with its code. An
 * instrumented function's counters and data record are tied to it: Mach-O's `-dead_strip`
 * keeps a record only while something live references its counters, and COFF's `/OPT:REF`
 * discards the comdats associated with a discarded function. ELF links keep each profile
 * section whole, so every function compiled into the instrumented build has a record there.
 */
export const DROPS_DEAD_RECORDS: Readonly<Record<ObjectFormat, boolean>> = {
	elf: false,
	macho: true,
	coff: true
};

/** The mangled function a `no profile data available for function …` diagnostic names, or
 * `null` when the message is another one. */
export function missing_profile_function(message: string): string | null {
	return /no profile data available for function (\S+)/.exec(message)?.[1] ?? null;
}

/** A rustc codegen unit's LLVM module name, `<crate>.<hash>-cgu.<n>`, read apart. */
export interface CguModule {
	/** The crate the module belongs to (`tsv_lang`). */
	crate: string;
	/** The hash rustc derives from the crate's identity (its name and `-C metadata`), the
	 * same in every module of one compilation of the crate and different in any other. */
	hash: string;
}

/** The crate and hash of a codegen unit's module name (`tsv_lang.1189d0178b66a26f-cgu.0`),
 * or `null` when `module` has another shape. The hash is lowercase hex of no fixed width
 * (rustc drops leading zeros). */
export function parse_cgu_module(module: string): CguModule | null {
	const m = /^([A-Za-z0-9_]+)\.([0-9a-f]+)-cgu\.\d+$/.exec(module);
	return m ? { crate: m[1]!, hash: m[2]! } : null;
}

/**
 * Every crate the instrumented binary's records name a codegen unit of, with the CGU hashes
 * they carry: read from each record's `<module>;` prefix, so a crate whose records are all
 * bare (every function of it external) or all dropped is absent. A prefix of another shape names no crate
 * and is skipped.
 *
 * @param records - every full record name the instrumented binary wrote, prefixes kept
 * @returns crate name → the CGU hashes its records carry
 */
export function crate_cgu_hashes(records: Iterable<string>): Map<string, Set<string>> {
	const out = new Map<string, Set<string>>();
	for (const name of records) {
		const semi = name.indexOf(';');
		if (semi === -1) continue;
		const cgu = parse_cgu_module(name.slice(0, semi));
		if (cgu === null) continue;
		let hashes = out.get(cgu.crate);
		if (!hashes) out.set(cgu.crate, (hashes = new Set()));
		hashes.add(cgu.hash);
	}
	return out;
}

/** The use build's profile diagnostics, by what each means (`classify_diagnostics`). */
export interface DiagnosticClasses {
	/** A build script's: never run in training, so never profiled. */
	build_script: PgoDiagnostic[];
	/**
	 * A diagnostic whose module is not one the instrumented build compiled: its crate is
	 * among those the instrumented records name, under a CGU hash none of them carries — the
	 * use build compiled another crate world (a feature or `-C metadata` drift), so no
	 * `<module>;` record can match and every missing function would read as dead code — or
	 * its module name has no `<crate>.<hash>-cgu.<n>` shape to grade at all.
	 */
	foreign_module: PgoDiagnostic[];
	/** A function with no profile data that the instrumented binary holds no record of in the
	 * diagnostic's module (`binary_holds`), on a format whose linker drops dead functions'
	 * records: dead code, which never runs. Another module's copy of the same local function
	 * may stay live beside it. */
	dead_stripped: PgoDiagnostic[];
	/** A function with no profile data that the instrumented binary DOES record in the
	 * diagnostic's module: the profile is not that binary's — stale, or trained on other
	 * source. */
	stale: PgoDiagnostic[];
	/** Everything else: a hash mismatch, a missing function on a format that keeps every
	 * record (the use build compiled code the instrumented one did not), any other message. */
	unexpected: PgoDiagnostic[];
}

/**
 * Whether the instrumented binary holds the profile record a use-build diagnostic names: a
 * record named `module;symbol` (the local copy compiled into that module) or bare `symbol`
 * (an external function's). A local function's PGO name is `<source file>;<symbol>`, and
 * rustc's source file is the codegen unit, the module a diagnostic names — so a function
 * inlinable across crates is a distinct record per module that compiled a copy, and the
 * bare symbol alone cannot tell the copy diagnosed from a live copy elsewhere.
 *
 * @param records - every full record name the instrumented binary wrote, prefixes kept
 * @param module - the module the diagnostic names
 * @param symbol - the mangled function the diagnostic names
 */
export const binary_holds = (
	records: ReadonlySet<string>,
	module: string,
	symbol: string
): boolean => records.has(`${module};${symbol}`) || records.has(symbol);

/**
 * Sort the use build's diagnostics (`parse_pgo_diagnostics`) against the full record names
 * of the instrumented binary they were trained on (every record in a profile it wrote, the
 * `<module>;` prefix a local function's record carries kept) and its object format. A
 * missing function is held to its own module's record (`binary_holds`): it is benign only
 * when the instrumented binary has no record of it there AND its format drops dead
 * functions' records — a stale profile leaves functions the binary records unmatched
 * (`stale`), and on ELF no missing function is benign. A hash mismatch is never benign.
 *
 * Per-module matching holds only while both builds compile the same crate world, so every
 * other diagnostic is first held to the instrumented build's modules (`crate_cgu_hashes`):
 * a crate the records name, diagnosed under a CGU hash they do not carry, is a use build
 * from another crate world (`foreign_module`), where no `<module>;` record could match and
 * a dead-code reading would pass a build no profile reached. A crate the records do not
 * name (every function of it external, or every record of it dropped with dead code) is not
 * held to this. A module name without the
 * codegen-unit shape fails closed into the same class: rustc names every module it
 * compiles that way, so another shape is a log this reader does not understand.
 */
export function classify_diagnostics(
	diagnostics: readonly PgoDiagnostic[],
	binary_records: ReadonlySet<string>,
	format: ObjectFormat
): DiagnosticClasses {
	const out: DiagnosticClasses = {
		build_script: [],
		foreign_module: [],
		dead_stripped: [],
		stale: [],
		unexpected: []
	};
	const crate_hashes = crate_cgu_hashes(binary_records);
	for (const d of diagnostics) {
		if (d.build_script) {
			out.build_script.push(d);
			continue;
		}
		const cgu = parse_cgu_module(d.module);
		if (cgu === null || crate_hashes.get(cgu.crate)?.has(cgu.hash) === false) {
			out.foreign_module.push(d);
			continue;
		}
		const missing = missing_profile_function(d.message);
		if (missing === null) out.unexpected.push(d);
		else if (binary_holds(binary_records, d.module, missing)) out.stale.push(d);
		else if (DROPS_DEAD_RECORDS[format]) out.dead_stripped.push(d);
		else out.unexpected.push(d);
	}
	return out;
}

/** `text` without its ANSI escape sequences — a log cargo wrote with colour on (the builds
 * turn it off; this keeps a reader from depending on that). */
export function strip_ansi(text: string): string {
	// deno-lint-ignore no-control-regex
	return text.replace(/\x1b\[[0-9;?]*[A-Za-z]/g, '');
}

/**
 * The crates whose rustc invocation in a `cargo build -v` log carries `-Cprofile-use=` naming
 * a file called `profdata_name` — the spelling-level proof that the profile reached each
 * compile, which a zero diagnostic count cannot give alone (a flag that never reached the
 * build also reports nothing). cargo prints each invocation as ``Running `…rustc --crate-name
 * <name> …` ``.
 */
export function profile_use_crates(log: string, profdata_name: string): Set<string> {
	const out = new Set<string>();
	for (const line of strip_ansi(log).split(/\r?\n/)) {
		if (!/^\s*Running `/.test(line)) continue;
		const name = /--crate-name (\S+)/.exec(line)?.[1];
		if (!name) continue;
		const uses = [...line.matchAll(/-Cprofile-use=(\S+?)(?=[\s`'"]|$)/g)].some(
			(m) => /[\\/]([^\\/]+)$/.exec(m[1]!)?.[1] === profdata_name
		);
		if (uses) out.add(name);
	}
	return out;
}

/** Whether `llvm-readobj --sections` output lists an instrumentation section — ELF and
 * Mach-O spell them `__llvm_prf_*`, COFF `.lprf*`. */
export function has_instrumentation_sections(readobj: string): boolean {
	return /^\s*Name: (?:__llvm_prf|\.lprf)/m.test(readobj);
}
