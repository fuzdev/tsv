/**
 * Per-row corpus count pins — the exact counts `corpus:compare:format --all` and
 * `corpus:compare:parse --all` grade, one row per corpus SOURCE (a snapshot collection,
 * a prettier suite, the styles harvest) and language, kept in a generated, committed
 * text file per gate rather than as hand-written totals in `gate_counts.ts`. Adding a
 * collection adds rows and moves none; a regression names the collection it is in; and
 * a deliberate move is re-pinned by the gate's own `--update`, so the diff of the file
 * IS the record of what moved (its attribution goes in the commit message —
 * docs/gate_counts.md §Update ritual).
 *
 * The grammar, one line per row:
 *
 * ```text
 * # comment
 * <row> <language> <bucket>=<count> <bucket>=<count> …
 * ```
 *
 * Every bucket the file's gate declares appears on every line, in declaration order;
 * rows are sorted by row then language. A row is present iff the run saw a file of that
 * language from that source, so a zero count is a real reading, not an absence. The
 * buckets cover every file a passing run grades, so a row also pins its file count.
 *
 * Node builtins only, so it gates in `deno task test:deno`.
 *
 * @module
 */

import { readFile, writeFile } from 'node:fs/promises';

import { CORPORA_COLLECTIONS } from './corpora.ts';
import { LANGUAGES, type Language } from './types.ts';

/** One gate's pin table: `"<row> <language>"` → bucket counts. */
export type PinTable<B extends string> = Map<string, Record<B, number>>;

/**
 * The row a corpus entry's files count under (`SourceFile.source`): a snapshot
 * collection's name, whichever of its subpaths the file came from (one collection is
 * one row however many subpaths its manifest spells), else the entry's own path without
 * its leading `../` (`prettier/tests/format/typescript`, `benches/js/.cache/svelte_styles`).
 */
export function corpus_pin_row(source: string): string {
	const prefix = `${CORPORA_COLLECTIONS}/`;
	const row = source.startsWith(prefix)
		? source.slice(prefix.length).split('/')[0]!
		: source.replace(/^(\.\.\/)+/, '');
	// a row is one whitespace-free field of the file grammar
	if (row === '' || /\s/.test(row)) throw new Error(`no pin row for corpus source "${source}"`);
	return row;
}

/** The table key for a row and language. */
const pin_key = (row: string, language: Language): string => `${row} ${language}`;

/** Accumulates one run's per-row counts. */
export class CorpusPinTally<B extends string> {
	readonly buckets: readonly B[];
	readonly #table: PinTable<B> = new Map();

	/** @param buckets - the counts every row carries, in file order */
	constructor(buckets: readonly B[]) {
		this.buckets = buckets;
	}

	/**
	 * Count a file the run reached, creating its row — so a source whose every file
	 * lands in no pinned bucket still reads as a row of zeros.
	 */
	see(source: string, language: Language): Record<B, number> {
		const key = pin_key(corpus_pin_row(source), language);
		let counts = this.#table.get(key);
		if (!counts) {
			counts = Object.fromEntries(this.buckets.map((b) => [b, 0])) as Record<B, number>;
			this.#table.set(key, counts);
		}
		return counts;
	}

	/** Count a file into `bucket`. */
	add(source: string, language: Language, bucket: B): void {
		this.see(source, language)[bucket]++;
	}

	/** The table, rows sorted. */
	table(): PinTable<B> {
		return sort_table(this.#table);
	}
}

/** Order table keys by row name, then by `LANGUAGES` order — the file's order. */
function compare_keys(a: string, b: string): number {
	const split = (key: string): [string, number] => {
		const at = key.lastIndexOf(' ');
		return [key.slice(0, at), LANGUAGES.indexOf(key.slice(at + 1) as Language)];
	};
	const [ra, la] = split(a);
	const [rb, lb] = split(b);
	return ra < rb ? -1 : ra > rb ? 1 : la - lb;
}

/** A copy of the table in file order. */
function sort_table<B extends string>(table: PinTable<B>): PinTable<B> {
	return new Map([...table].sort(([a], [b]) => compare_keys(a, b)));
}

/** Serialize a table, `header` lines first (each written as a `#` comment). */
export function serialize_pins<B extends string>(
	table: PinTable<B>,
	buckets: readonly B[],
	header: readonly string[]
): string {
	const lines = header.map((h) => (h === '' ? '#' : `# ${h}`));
	for (const [key, counts] of [...table].sort(([a], [b]) => compare_keys(a, b))) {
		lines.push(`${key} ${buckets.map((b) => `${b}=${counts[b]}`).join(' ')}`);
	}
	return lines.join('\n') + '\n';
}

/**
 * Parse a pins file strictly: an unknown language, a missing, extra or out-of-order
 * bucket, a non-integer count or a duplicate row is an error naming its line, never a
 * silently partial table.
 */
export function parse_pins<B extends string>(
	text: string,
	buckets: readonly B[]
): { ok: true; table: PinTable<B> } | { ok: false; error: string } {
	const table: PinTable<B> = new Map();
	const lines = text.split('\n');
	for (let i = 0; i < lines.length; i++) {
		const line = lines[i]!.trim();
		if (line === '' || line.startsWith('#')) continue;
		const fail = (why: string) => ({ ok: false as const, error: `line ${i + 1}: ${why}` });
		const fields = line.split(/\s+/);
		if (fields.length !== 2 + buckets.length) {
			return fail(`expected <row> <language> and ${buckets.length} bucket(s), got "${line}"`);
		}
		const [row, language, ...rest] = fields as [string, string, ...string[]];
		if (!LANGUAGES.includes(language as Language)) return fail(`unknown language "${language}"`);
		const counts = {} as Record<B, number>;
		for (let b = 0; b < buckets.length; b++) {
			const m = /^([a-z_]+)=(\d+)$/.exec(rest[b]!);
			if (!m || m[1] !== buckets[b])
				return fail(`expected ${buckets[b]}=<count>, got "${rest[b]}"`);
			counts[buckets[b]!] = Number(m[2]);
		}
		const key = pin_key(row, language as Language);
		if (table.has(key)) return fail(`duplicate row "${key}"`);
		table.set(key, counts);
	}
	return { ok: true, table };
}

/**
 * Every difference between the pinned and the measured table, one line each:
 * `<row> <language> <bucket> <pinned> → <measured>`, or a row present on one side only.
 */
export function diff_pins<B extends string>(
	pinned: PinTable<B>,
	measured: PinTable<B>,
	buckets: readonly B[]
): string[] {
	const out: string[] = [];
	const keys = [...new Set([...pinned.keys(), ...measured.keys()])].sort(compare_keys);
	for (const key of keys) {
		const p = pinned.get(key);
		const m = measured.get(key);
		if (!p) out.push(`${key}: new row (${buckets.map((b) => `${b}=${m![b]}`).join(' ')})`);
		else if (!m) out.push(`${key}: pinned row not measured (a source absent from this run?)`);
		else {
			for (const b of buckets) if (p[b] !== m[b]) out.push(`${key} ${b} ${p[b]} → ${m[b]}`);
		}
	}
	return out;
}

/** The per-language sums of a table — what `pins_disagree_with_stats` grades. */
export function sum_pins<B extends string>(
	table: PinTable<B>,
	buckets: readonly B[]
): Record<Language, Record<B, number>> {
	const zero = () => Object.fromEntries(buckets.map((b) => [b, 0])) as Record<B, number>;
	const sums = Object.fromEntries(LANGUAGES.map((l) => [l, zero()])) as Record<
		Language,
		Record<B, number>
	>;
	for (const [key, counts] of table) {
		const language = key.slice(key.lastIndexOf(' ') + 1) as Language;
		for (const b of buckets) sums[language][b] += counts[b];
	}
	return sums;
}

/**
 * Every language whose tally sums differ from the run's own per-language stats, one
 * line each — the guard that the per-file tally saw what the gate's tables counted, so
 * a path that counts a file without tallying it cannot slip a pin past the totals.
 */
export function pins_disagree_with_stats<B extends string>(
	table: PinTable<B>,
	buckets: readonly B[],
	stats: (language: Language) => Record<B, number>
): string[] {
	const sums = sum_pins(table, buckets);
	return LANGUAGES.flatMap((language) => {
		const expected = stats(language);
		const off = buckets.filter((b) => sums[language][b] !== expected[b]);
		return off.length === 0
			? []
			: [
					`${language}: the per-source tally disagrees with the run's stats (` +
						off.map((b) => `${b} ${sums[language][b]} vs ${expected[b]}`).join(', ') +
						') — a harness bug, not a corpus move'
				];
	});
}

/** How a gate's run treats its pins file (`grade_corpus_pins`). */
export interface CorpusPinsOptions<B extends string> {
	/** The pins file, project-root-relative. */
	file: string;
	/** The gate that owns it, for the header and the remedy (`corpus:compare:format`). */
	gate: string;
	tally: CorpusPinTally<B>;
	/** Rewrite the file from this run instead of grading against it. */
	update: boolean;
	/**
	 * Why this run may not rewrite the file (an absent optional source, a red verdict, an
	 * environment error) — any entry refuses an `update`; a grade ignores them.
	 */
	update_blockers?: readonly string[];
	/** What `pins_disagree_with_stats` grades the tally against, per language. */
	stats: (language: Language) => Record<B, number>;
}

/**
 * Grade a full run's tally against its pins file, or with `update` rewrite the file from
 * it. Returns the failure lines (empty = pass); an update prints what it moved and
 * always passes. A missing or malformed file fails the grade with the reason.
 */
export async function grade_corpus_pins<B extends string>(
	options: CorpusPinsOptions<B>
): Promise<string[]> {
	const { file, gate, tally, update, update_blockers = [], stats } = options;
	const measured = tally.table();
	const drift = pins_disagree_with_stats(measured, tally.buckets, stats);
	if (drift.length > 0) return drift;
	let text: string | null = null;
	try {
		text = await readFile(file, 'utf8');
	} catch (e) {
		// absent: an update writes it, a grade fails below
		if ((e as { code?: unknown }).code !== 'ENOENT') throw e;
	}
	const read = text === null ? null : parse_pins(text, tally.buckets);
	if (update) {
		if (update_blockers.length > 0) return update_blockers.map((b) => `refusing --update: ${b}`);
		if (read && !read.ok) console.log(`\nPins: replacing a malformed ${file} (${read.error})`);
		const before = read?.ok ? read.table : new Map<string, Record<B, number>>();
		const moved = diff_pins(before, measured, tally.buckets);
		const header = [
			`Generated by \`deno task ${gate} --all --update\` — never hand-edited.`,
			'Exact per-source counts over the `gates` view; see docs/gate_counts.md.',
			'',
			`<row> <language> ${tally.buckets.map((b) => `${b}=<n>`).join(' ')}`
		];
		await writeFile(file, serialize_pins(measured, tally.buckets, header));
		console.log(
			moved.length === 0
				? `\nPins: ${file} unchanged.`
				: `\nPins: rewrote ${file} (${moved.length} change(s)):\n  ${moved.join('\n  ')}`
		);
		return [];
	}
	if (read === null) return [`${file} is missing — run \`deno task ${gate} --all --update\``];
	if (!read.ok) return [`${file} ${read.error}`];
	return diff_pins(read.table, measured, tally.buckets);
}
