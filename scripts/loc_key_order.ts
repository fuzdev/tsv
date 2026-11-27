/**
 * Where a `{locations: true}` tree first departs from the `loc` wire's key order — the order
 * `tsv parse --locations` writes, and the one the tape decoder's loc mode builds: `loc` right
 * after `end`, a Svelte `name_loc` right after `name`. Shared by both package suites
 * (`scripts/test_npm.ts`, `scripts/test_napi_npm.ts`), which hold the shipped
 * `parse_<lang>(source, {locations: true})` to it.
 *
 * Imports nothing, so the suites load it under any runtime with no install.
 *
 * @module
 */

/**
 * The path of the first object whose `loc` does not follow its `end` (or `name_loc` its `name`),
 * or `null` when every one does; walks with a stack, so any depth the parsers accept.
 * @param tree - a parse's tree
 */
export function loc_key_order(tree: unknown): string | null {
	const stack: Array<[unknown, string]> = [[tree, '$']];
	while (stack.length > 0) {
		const [value, path] = stack.pop()!;
		if (value === null || typeof value !== 'object') continue;
		const keys = Object.keys(value);
		for (const [key, after] of [
			['loc', 'end'],
			['name_loc', 'name']
		] as const) {
			const at = keys.indexOf(key);
			if (at !== -1 && keys[at - 1] !== after) {
				return `${path}: \`${key}\` follows \`${keys[at - 1] ?? '(nothing)'}\`, not \`${after}\``;
			}
		}
		for (const key of keys) {
			if (key === 'loc' || key === 'name_loc') continue;
			stack.push([(value as Record<string, unknown>)[key], `${path}.${key}`]);
		}
	}
	return null;
}
