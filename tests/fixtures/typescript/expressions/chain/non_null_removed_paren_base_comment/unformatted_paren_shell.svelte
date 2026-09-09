<script lang="ts">
	// A redundant paren shell around a chain's BASE is stripped, and the run the author
	// wrote inside it leads the chain. The answer is the same whether or not a `!` seals
	// the chain's tail — but the sealed spelling reaches the chain through its own entry
	// point, which has to claim the base's removed-paren run exactly as the member and
	// call entries do.
	const a = ( // c1
	e).m()!;

	// A block run takes the same claim — the rule is not `//`-only.
	const b = ( /* c2 */
	e).m()!;

	// Statement and call-argument positions answer the same way.
	( // c3
	e).m()!;
	fn(( // c4
	e).m()!);

	// A base whose pair the chain REQUIRES answers the same: the pair stays and the run
	// still leads the chain.
	( // c5
	e as E).m()!;

	// The three controls the sealed spelling has to agree with — the same shell with no
	// `!` at all, with the `!` continued past, and with the `!` called past. In each the
	// chain's outermost node is a member or a call, so a different entry point claims.
	const c = ( // c6
	e).m();
	const d = ( // c7
	e).m()!.n;
	const f = ( // c8
	e).m()!();
</script>
