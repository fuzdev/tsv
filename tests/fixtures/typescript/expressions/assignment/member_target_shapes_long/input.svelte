<script lang="ts">
	// The target rule holds whatever the target is built from — the base, and any computed
	// or call link along it, keep their own break points and become the only ones left. It
	// stops at the two spellings that are targets in the grammar but not in the clause.

	// a non-null in the target — the `!` glues to its lookup and adds no break point
	aaaa!.bbbbbbbbbbbb.cccccccccccc.dddddddddddd.eeeeeeeeeeee.ffffffffffff.gggggggggggg.hhhhhhhhhhhh.iii =
		v;

	// a `this` base takes the same rule as an identifier base
	this.aaaaaaaaaaaa.bbbbbbbbbbbb.cccccccccccc.dddddddddddd.eeeeeeeeeeee.ffffffffffff.gggggggggggg.hhhhh =
		v;

	// a computed lookup at the head of the target — its brackets are the only break point
	aaa[
		'kkk'
	].bbbbbbbbbbbb.cccccccccccc.dddddddddddd.eeeeeeeeeeee.ffffffffffff.gggggggggggg.hhhhhhhhhhhh = v;

	// a computed lookup mid-target — the dot lookups around it still stay put
	aaaaaaaaaaaa.bbbbbbbbbbb[
		'kkk'
	].cccccccccccc.dddddddddddd.eeeeeeeeeeee.ffffffffffff.gggggggggggg.h = v;

	// a parenthesized base in the target — the base's own parens are the only break point
	(
		aaa as Tttt
	).bbbbbbbbbbbb.cccccccccccc.dddddddddddd.eeeeeeeeeeee.ffffffffffff.gggggggggggg.hhhhhhhhh = v;

	// a call in the target — the clause reaches the lookups above it, the call keeps its args
	fnnn().aaaaaaaaaaaa.bbbbbbbbbbbb.cccccccccccc.dddddddddddd.eeeeeeeeeeee.ffffffffffff.gggggggggggg.hhh =
		v;

	// NOT a target in `shouldInline`'s sense: a `for…of` head keeps its break points
	for (aaaaaaaaaaaa.bbbbbbbbbbbb.cccccccccccc.dddddddddddd.eeeeeeeeeeee.ffffffffffff
		.gggggggggggg of xs)
		fn();

	// NOT a target either: a destructuring pattern sits between the chain and the assignment
	[
		aaaaaaaaaaaa.bbbbbbbbbbbb.cccccccccccc.dddddddddddd.eeeeeeeeeeee.ffffffffffff.gggggggggggg
			.hhhhhh
	] = v;
</script>
