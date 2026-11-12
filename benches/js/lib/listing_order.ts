/**
 * The order the comparison tools walk a directory in: Node's `readdir`, which libuv sorts by
 * `strcmp` over the UTF-8 names, so by code point. A handle's listing (`opendir`) comes in
 * the filesystem's order instead, and the tools' reports list files in walk order.
 *
 * @module
 */

/** Compares two names by code point, as `strcmp` compares their UTF-8 bytes. */
export function compare_code_points(a: string, b: string): number {
	const a_points = [...a];
	const b_points = [...b];
	const n = Math.min(a_points.length, b_points.length);
	for (let i = 0; i < n; i++) {
		const d = a_points[i]!.codePointAt(0)! - b_points[i]!.codePointAt(0)!;
		if (d !== 0) return d;
	}
	return a_points.length - b_points.length;
}

/** `entries` sorted by name in `readdir`'s order, in place. */
export function sort_listing<T extends { name: string }>(entries: T[]): T[] {
	return entries.sort((a, b) => compare_code_points(a.name, b.name));
}
