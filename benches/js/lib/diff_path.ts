/**
 * Resolving a deep diff's concrete paths (`fragment.nodes[3].expression`) back to the node
 * they name — shared by the documented-divergence matchers (`parse_divergences.ts`) and
 * the `loc` tolerance rows (`loc_tolerance.ts`), which both read the canonical tree at a
 * difference's path.
 *
 * @module
 */

/** Resolve a node by concrete diff path (`fragment.nodes[3].expression`); `''` is the root. */
export function get_at_path(root: unknown, path: string): unknown {
	if (path === '') return root;
	let node = root;
	for (const seg of path.split('.')) {
		if (node == null) return null;
		const m = seg.match(/^([^[]+)((?:\[\d+\])*)$/);
		if (!m) return null;
		node = (node as Record<string, unknown>)[m[1]];
		for (const idx of m[2].matchAll(/\[(\d+)\]/g)) {
			if (!Array.isArray(node)) return null;
			node = node[Number(idx[1])];
		}
	}
	return node;
}
