<script lang="ts">
	// The callee rule holds whatever the callee is built from — a `!` link, a parenthesized
	// base, a call link — each keeps its own break points and becomes the only ones left. It
	// stops where the chain is no longer the callee itself.

	// a non-null in the callee — the `!` glues to its lookup and adds no break point
	new aaaa!.bbbbbbbbbbbb.cccccccccccc.dddddddddddd.eeeeeeeeeeee.ffffffffffff.gggggggggggg.hhhhhhhh();

	// type arguments ride the callee, past the last lookup
	new aaaaaaaaaaaa.bbbbbbbbbbbb.cccccccccccc.dddddddddddd.eeeeeeeeeeee.ffffffffffff.gggggggggg<Tt>();

	// a parenthesized base in the callee — the base's own parens are the only break point
	new (
		aaa as Tttt
	).bbbbbbbbbbbb.cccccccccccc.dddddddddddd.eeeeeeeeeeee.ffffffffffff.ggggggggggggg();

	// a call in the callee — the clause reaches the lookups above it, the call keeps its args
	new (fnnn(
		kkk
	).aaaaaaaaaaaa.bbbbbbbbbbbb.cccccccccccc.dddddddddddd.eeeeeeeeeeee.fffffffffffffff)();

	// a computed index holding its own assignment — the callee is marked before any node doc
	// is built, so the nested target's mark cannot clear it and the lookups after `]` stay glued
	new aaaaaaaaaaaa.bbbbbbbbbbbb[
		(cccccccccccc = dddddddddddd)
	].eeeeeeeeeeee.ffffffffffff.gggggggggggggg();

	// NOT the callee: an optional chain is a `ChainExpression`, which the walk stops at
	new (aaaaaaaaaaaa?.bbbbbbbbbbbb.cccccccccccc.dddddddddddd.eeeeeeeeeeee.ffffffffffff
		.gggggggggggggg)();

	// NOT the callee: a chain inside a computed index keeps its break points
	new aaa[
		bbbbbbbbbbbb.cccccccccccc.dddddddddddd.eeeeeeeeeeee.ffffffffffff.gggggggggggg
			.hhhhhhhhhhhhhhhhhhhhhhh
	]();

	// NOT the callee: members trailing the `new` expression keep their break points
	new aaaa.bbbb().cccccccccccc.dddddddddddd.eeeeeeeeeeee.ffffffffffff.gggggggggggg
		.hhhhhhhhhhhhhhhhh;
</script>
