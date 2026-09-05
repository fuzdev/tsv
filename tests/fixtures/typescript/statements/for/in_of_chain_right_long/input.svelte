<script>
	// A for-in/for-of iterable takes a continuation indent: prettier's shouldNotIndent
	// names ForStatement, and a ForOfStatement / ForInStatement is not one
	for (const a of bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb ??
		cccccccccccccccccccccccccccccccccc ??
		ddd) {
	}

	for (const b in bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb ??
		cccccccccccccccccccccccccccccccccc ??
		ddd) {
	}

	// A C-style for header IS a ForStatement, so its clause chain stays flush
	for (
		let c = 0;
		bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb ??
		cccccccccccccccccccccccccccccccccccccccccccccc ??
		ddd;
		c++
	) {}

	// 100 chars - stays inline
	for (const d of bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb ?? cccccccccccccccccccccccccccc ?? ddd) {
	}

	// 101 chars - the chain breaks
	for (const e of bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb ??
		ccccccccccccccccccccccccccccc ??
		ddd) {
	}
</script>
