<script lang="ts">
	// A `//` in the same gap cannot trail inline before the `)` — it would swallow it —
	// so the pair takes its expanded shell and the comment stays inside, at every
	// position that prints the pair: a member access, a call, a computed lookup and a
	// template tag.
	const m =
		(a
			?.b) // c1
		.ccc;
	(a?.b) // c2
	();
	const n =
		(a
			?.b) // c3
		[0];
	(a?.b) // c4
	`tpl`;

	// With BOTH gaps commented the pair still takes ONE expanded shell — the leading run
	// above the chain, the trailing run beside it, the `)` back out.
	const r =
		(// c7
		a?.b /* c8 */).ccc;
	const s =
		(// c9
		a
			?.b) // c10
		.ccc;

	// A comment the author wrote AFTER the `)` is outside the pair and stays there —
	// the `)` is the boundary, and each side keeps what was written on it.
	const o = (a?.b /* c5 */).ccc;
	const p =
		(a
			?.b) // c6
		.ccc;
</script>
