<script lang="ts">
	// A trailing `.prop` off a chain-rooted call keeps its break point: prettier inlines a
	// lookup only for the clauses `printMemberExpression`'s `shouldInline` names, and a bare
	// statement is none of them, so the width falls to the lookup rather than to the prefix.

	// 100 - the call and its lookup fit on one line
	aaaa.bbbb().ccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc;

	// 101 - one char longer: the lookup drops to its own line, the call keeps its `()`
	aaaa.bbbb()
		.dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd;

	// the call's arguments stay inline - the lookup is the break point
	aaaa.bbbb(cccc)
		.eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee;

	// two lookups: only the last one drops
	aaaa.bbbb().ffffffffffffffffffffffffffffffffffffffff
		.gggggggggggggggggggggggggggggggggggggggggggggggg;

	// a longer chain is the same - the chain stays flat, the lookup drops
	aaaa.bbbb().cccc()
		.hhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhhh;
</script>
