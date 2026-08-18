<script lang="ts">
	// A comment between a sealed parenthesized optional chain and its `)` stays inside
	// the parens, where the author wrote it. The pair is what seals the chain, so the
	// gap is the pair's at every position that prints it — a member access, a call, a
	// computed lookup and a template tag — with or without a `!` after the `)`.
	const m = (a?.b /* c1 */).ccc;
	(a?.b /* c2 */)();
	const n = (a?.b /* c3 */)[0];
	(a?.b /* c4 */)`tpl`;
	const o = (a?.() /* c5 */).ccc;

	// The `!` spelling of the same gap answers identically.
	const p = (a?.b /* c6 */)!.ccc;
</script>
