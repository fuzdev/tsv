/**
 * The branch report's parsers over tool output: `nm`, `objdump`, `size`, `heaptrack_print`, the
 * depth gate's shape file and the type-size boards. Pure functions of the text a tool printed,
 * so `parsers_test.ts` grades each over a captured sample.
 *
 * @module
 */

// -- symbol names -----------------------------------------------------------------------------

const LEGACY_ESCAPES: ReadonlyArray<[RegExp, string]> = [
	[/\$LT\$/g, '<'],
	[/\$GT\$/g, '>'],
	[/\$LP\$/g, '('],
	[/\$RP\$/g, ')'],
	[/\$C\$/g, ','],
	[/\$u20\$/g, ' '],
	[/\$u27\$/g, "'"],
	[/\$u5b\$/g, '['],
	[/\$u5d\$/g, ']'],
	[/\$u7b\$/g, '{'],
	[/\$u7d\$/g, '}'],
	[/\$RF\$/g, '&'],
	[/\$BP\$/g, '*'],
	[/\$LT\$/g, '<']
];

/**
 * The stable key of a mangled Rust symbol: the legacy path with its per-build hash segment and
 * any `.llvm.<n>` suffix removed, so one function's key is the same across two builds whose
 * hashes differ; non-legacy names pass through with the suffix removed.
 */
export function symbol_key(mangled: string): string {
	const stem = mangled.replace(/\.llvm\.\d+$/, '');
	if (!stem.startsWith('_ZN') || !stem.endsWith('E')) return stem;
	const segments: Array<string> = [];
	let i = 3;
	const end = stem.length - 1;
	while (i < end) {
		let j = i;
		while (j < end && stem.charCodeAt(j) >= 48 && stem.charCodeAt(j) <= 57) j++;
		if (j === i) return stem;
		const length = Number(stem.slice(i, j));
		const segment = stem.slice(j, j + length);
		i = j + length;
		// the legacy mangling's hash segment: `17h` + 16 hex digits, always last
		if (i >= end && /^h[0-9a-f]{16}$/.test(segment)) break;
		segments.push(segment.startsWith('_$') ? segment.slice(1) : segment);
	}
	let key = segments.join('::').replaceAll('..', '::');
	for (const [pattern, text] of LEGACY_ESCAPES) key = key.replace(pattern, text);
	return key;
}

export interface SymbolSizes {
	/** `.text` bytes under each key, summed across same-named copies (monomorphizations). */
	sizes: Map<string, number>;
	copies: Map<string, number>;
	total: number;
}

/**
 * `nm -S --radix=d` output's function symbols, keyed by `symbol_key`: types t/T/w/W with a size.
 */
export function parse_nm(text: string): SymbolSizes {
	const sizes = new Map<string, number>();
	const copies = new Map<string, number>();
	let total = 0;
	for (const line of text.split('\n')) {
		const match = /^\s*\d+\s+(\d+)\s+([a-zA-Z])\s+(\S+)/.exec(line);
		if (!match) continue;
		const [, size_text, type, name] = match;
		if (!'tTwW'.includes(type!)) continue;
		const size = Number(size_text);
		if (size === 0) continue;
		const key = symbol_key(name!);
		sizes.set(key, (sizes.get(key) ?? 0) + size);
		copies.set(key, (copies.get(key) ?? 0) + 1);
		total += size;
	}
	return { sizes, copies, total };
}

/** `parse_nm`'s result, refusing an empty table, which would diff as identical. */
export function assert_symbols(symbols: SymbolSizes, binary: string): SymbolSizes {
	if (symbols.sizes.size === 0) throw new Error(`nm found no function symbols in ${binary}`);
	return symbols;
}

export interface SymbolMover {
	key: string;
	base: number | null;
	cand: number | null;
	delta: number;
}

export interface SymbolDiff {
	base_total: number;
	cand_total: number;
	added: number;
	removed: number;
	changed: number;
	/** Every key that differs, widest absolute delta first (ties by key, so stable). */
	movers: Array<SymbolMover>;
}

/** Keys with different summed size. Moved or renamed functions land as one removed + one added. */
export function diff_symbols(base: SymbolSizes, cand: SymbolSizes): SymbolDiff {
	const movers: Array<SymbolMover> = [];
	let added = 0;
	let removed = 0;
	let changed = 0;
	for (const [key, size] of base.sizes) {
		const other = cand.sizes.get(key);
		if (other === undefined) {
			removed++;
			movers.push({ key, base: size, cand: null, delta: -size });
		} else if (other !== size) {
			changed++;
			movers.push({ key, base: size, cand: other, delta: other - size });
		}
	}
	for (const [key, size] of cand.sizes) {
		if (base.sizes.has(key)) continue;
		added++;
		movers.push({ key, base: null, cand: size, delta: size });
	}
	movers.sort((a, b) => Math.abs(b.delta) - Math.abs(a.delta) || a.key.localeCompare(b.key));
	return { base_total: base.total, cand_total: cand.total, added, removed, changed, movers };
}

// -- call sites ---------------------------------------------------------------------------------

/**
 * The three panic edges a checked index or slice compiles to, named by a substring of each
 * callee's mangled symbol — a stable function name in `core`, any of its versions' spellings.
 */
export const BOUNDS_CALLEES: ReadonlyArray<{ name: string; patterns: ReadonlyArray<string> }> = [
	{ name: 'panic_bounds_check', patterns: ['panic_bounds_check'] },
	{
		name: 'slice index fail',
		patterns: [
			'slice_index_fail',
			'slice_index_order_fail',
			'slice_start_index_len_fail',
			'slice_end_index_len_fail'
		]
	},
	{ name: 'str slice_error_fail', patterns: ['slice_error_fail'] }
];

/**
 * Count `call` and `jmp` instructions in `objdump -d --no-show-raw-insn` output per callee group
 * (each edge is one call site; a tail call counts).
 */
export function count_bounds_sites(objdump_text: string): Map<string, number> {
	const counts = new Map<string, number>(BOUNDS_CALLEES.map(({ name }) => [name, 0]));
	for (const line of objdump_text.split('\n')) {
		if (!/\s(?:call|jmp)q?\s/.test(line)) continue;
		// `>` right after the name: a `<sym+0x12>` jump lands inside a function, it does not call one
		const target = /<([^>+]+)>/.exec(line)?.[1];
		if (target === undefined) continue;
		for (const { name, patterns } of BOUNDS_CALLEES) {
			if (patterns.some((p) => target.includes(p))) {
				counts.set(name, counts.get(name)! + 1);
				break;
			}
		}
	}
	return counts;
}

/** `count_bounds_sites`'s result, refusing an all-zero count: a real binary always holds some,
 * so all-zero means the disassembly or the callee names no longer read, not a clean binary. */
export function assert_bounds_sites(
	counts: Map<string, number>,
	binary: string
): Map<string, number> {
	if ([...counts.values()].every((n) => n === 0)) {
		throw new Error(`objdump of ${binary} shows no bounds-miss call site of any kind`);
	}
	return counts;
}

// -- sections -----------------------------------------------------------------------------------

/** `size -A -d <file>`'s section table as a name → bytes map. */
export function parse_size_sections(text: string): Map<string, number> {
	const sections = new Map<string, number>();
	for (const line of text.split('\n')) {
		const match = /^(\S+)\s+(\d+)\s+\d+\s*$/.exec(line.trim());
		if (match) sections.set(match[1]!, Number(match[2]));
	}
	return sections;
}

// -- runs -----------------------------------------------------------------------------------------

/** Refuse an exit that is not a finished `tsv format --check` (0 clean, 1 would-change). Exit 2 is
 * a parse error or a malformed command line, which a ref lacking a flag would produce. */
export function assert_format_check_finished(code: number, stderr: string, label: string): void {
	if (code !== 0 && code !== 1) throw new Error(`${label} exited ${code}: ${stderr.trim()}`);
}

/** The KiB `/usr/bin/time -f %M` wrote as its last stderr line, or null if it wrote none. */
export function parse_rss_kib(stderr: string): number | null {
	const last = stderr.trim().split('\n').pop() ?? '';
	if (!/^\d+$/.test(last)) return null;
	const kib = Number(last);
	return kib > 0 ? kib : null;
}

/** Whether a libtest run actually ran a test: a filter matching nothing also exits 0. */
export function test_run_matched(stdout: string): boolean {
	return /test result: ok\. [1-9]\d* passed/.test(stdout);
}

/** A wasm sweep's summary, refusing no files or any throw: the child's per-call `try` is the only
 * thing that sees a missing export, and an empty list measures nothing. */
export function assert_wasm_sweep(
	sweep: { files: number; errors: number; first_error: string | null },
	label: string
): void {
	if (sweep.files === 0 || sweep.errors > 0) {
		throw new Error(
			`${label}: ${sweep.files} files, ${sweep.errors} threw (first: ${sweep.first_error ?? 'none'})`
		);
	}
}

/** `rows`, refusing an empty parse, which would compare as identical. */
export function assert_parsed_rows<T>(rows: Array<T>, what: string): Array<T> {
	if (rows.length === 0) throw new Error(`${what} parsed to no rows`);
	return rows;
}

// -- heaptrack -----------------------------------------------------------------------------------

export interface HeaptrackSummary {
	allocation_calls: number;
	temporary_allocations: number;
	peak_heap_bytes: number;
}

const SIZE_SUFFIX: Readonly<Record<string, number>> = {
	'': 1,
	B: 1,
	K: 1000,
	M: 1_000_000,
	G: 1_000_000_000
};

/** A heaptrack-printed size (`682.35K`, `1.20G`, `512B`) as bytes; heaptrack's units are 1000s. */
export function parse_heaptrack_size(text: string): number | null {
	const match = /^([0-9.]+)\s*([KMGB]?)/.exec(text.trim());
	if (!match) return null;
	return Math.round(Number(match[1]) * SIZE_SUFFIX[match[2]!]!);
}

/** The three figures `heaptrack_print`'s closing summary states, or null without them. */
export function parse_heaptrack_summary(text: string): HeaptrackSummary | null {
	const calls = /calls to allocation functions:\s+(\d+)/.exec(text);
	const temporary = /temporary memory allocations:\s+(\d+)/.exec(text);
	const peak = /peak heap memory consumption:\s+(\S+)/.exec(text);
	if (!calls || !temporary || !peak) return null;
	const peak_bytes = parse_heaptrack_size(peak[1]!);
	if (peak_bytes === null) return null;
	return {
		allocation_calls: Number(calls[1]),
		temporary_allocations: Number(temporary[1]),
		peak_heap_bytes: peak_bytes
	};
}

// -- depth shapes -------------------------------------------------------------------------------

export interface ShapeLimit {
	name: string;
	lang: string;
	n: number;
}

/** The depth gate's `write_shape_limits` file: name, language and accepted `n` per line. */
export function parse_shape_limits(text: string): Array<ShapeLimit> {
	const shapes: Array<ShapeLimit> = [];
	for (const line of text.split('\n')) {
		if (line === '') continue;
		const fields = line.split('\t');
		if (fields.length < 3) throw new Error(`malformed shape-limit line: ${line}`);
		const n = Number(fields[2]);
		if (!Number.isInteger(n)) throw new Error(`malformed shape-limit n: ${line}`);
		shapes.push({
			name: fields[0]!.replaceAll('\\n', '\n').replaceAll('\\t', '\t').replaceAll('\\\\', '\\'),
			lang: fields[1]!,
			n
		});
	}
	return shapes;
}

// -- type boards --------------------------------------------------------------------------------

export interface TypeRowSize {
	group: string;
	name: string;
	size: number;
	align: number;
	result_size: number;
}

/** `tsv_debug type_sizes --json` output. */
export function parse_type_sizes_json(text: string): Array<TypeRowSize> {
	const parsed = JSON.parse(text) as { types: Array<TypeRowSize> };
	return parsed.types.map(({ group, name, size, align, result_size }) => ({
		group,
		name,
		size,
		align,
		result_size
	}));
}

/** The wasm32 board's text: `group\tname\tsize\talign\tresult_size` per line. */
export function parse_board_text(text: string): Array<TypeRowSize> {
	const rows: Array<TypeRowSize> = [];
	for (const line of text.split('\n')) {
		if (line === '') continue;
		const [group, name, size, align, result_size] = line.split('\t');
		rows.push({
			group: group!,
			name: name!,
			size: Number(size),
			align: Number(align),
			result_size: Number(result_size)
		});
	}
	return rows;
}
