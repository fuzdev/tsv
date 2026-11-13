/**
 * `facade_probe.ts`'s byte-identity gate: before any arm is timed, every arm must return what
 * the bench row's own call returns, file by file, and the files the row accepts are the set
 * every arm is timed over. A pure function over the arms' calls, so a suite drives it over
 * fake ones.
 *
 * A crash (`is_tsv_crash`: a caught native panic or a WASM trap) in any arm is the probe's
 * failure, never a file the gate leaves out: the row's other throws are rejections, and a
 * file the row rejects is dropped from the timed set, so without the classifier a crash in the
 * row would drop its file from the gate and the timing without a word.
 *
 * @module
 */

import { crash_text, is_tsv_crash, type PanicEntry } from '../lib/divergence/panic_errors.ts';
import type { SourceFile } from '../lib/types.ts';

/** One timed call: a source in, the row's product out. */
export type Call = (source: string) => unknown;

/** One operation's calls: the row's, and each other arm's by name. */
export interface IdentityCalls {
	operation: string;
	row: Call;
	others: ReadonlyArray<{ arm: string; call: Call }>;
}

/** What the gate found over one binding and language. */
export interface IdentityGrade {
	/** The files the row accepts for every operation: the set every arm is timed over. */
	kept: SourceFile[];
	/** Files an arm returned other bytes for than the row, or threw on where the row didn't. */
	mismatches: number;
	/** Products the serializer couldn't render, so nothing was compared. */
	ungraded: number;
	/** tsv's crashes, each labeled with the cell, the operation, the arm, and the file. */
	crashes: PanicEntry[];
}

/**
 * What two arms must agree on: a format's string, or a parse's serialized tree; `null` for a
 * tree too deep for the recursive serializer, which is then ungraded.
 */
export const product_text = (product: unknown): string | null => {
	if (typeof product === 'string') return product;
	try {
		return JSON.stringify(product);
	} catch {
		return null;
	}
};

/**
 * Grade every arm against the row over `files`.
 *
 * A file the row rejects (any throw but a crash) is left out of `kept`, and its later
 * operations aren't called. A file the row crashes on is left out and listed. Where the row
 * accepted, an arm's crash is listed and any other throw is a mismatch, as a different product
 * is.
 *
 * @param cell - the binding and language, as the crash labels name them (`napi/css`)
 * @param calls - each operation's calls, in the order they run on a file
 * @param files - the language's corpus
 */
export function grade_identity(
	cell: string,
	calls: readonly IdentityCalls[],
	files: readonly SourceFile[]
): IdentityGrade {
	const grade: IdentityGrade = { kept: [], mismatches: 0, ungraded: 0, crashes: [] };
	const crash = (operation: string, arm: string, file: SourceFile, e: unknown): void => {
		grade.crashes.push({
			label: `${cell} ${operation} ${arm}: ${file.path}`,
			error: crash_text(e)
		});
	};
	for (const file of files) {
		let ok = true;
		for (const { operation, row, others } of calls) {
			let expected: string | null;
			try {
				expected = product_text(row(file.content));
			} catch (e) {
				if (is_tsv_crash(e)) crash(operation, 'row', file, e);
				ok = false;
				break;
			}
			if (expected === null) {
				grade.ungraded++;
				continue;
			}
			for (const { arm, call } of others) {
				let actual: string | null;
				try {
					actual = product_text(call(file.content));
				} catch (e) {
					if (is_tsv_crash(e)) crash(operation, arm, file, e);
					else grade.mismatches++;
					continue;
				}
				if (actual !== expected) grade.mismatches++;
			}
		}
		if (ok) grade.kept.push(file);
	}
	return grade;
}
