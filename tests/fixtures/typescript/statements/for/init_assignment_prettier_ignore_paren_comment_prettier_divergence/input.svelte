<script lang="ts">
	// A frozen assignment RHS under a `for` header's init: the `[~In]` pair is the
	// printer's, and the author's grouping shell is RETAINED because a `//` is written
	// inside it. Only one pair prints — the shell already parenthesizes the `in`.
	for (
		ccc =
			// prettier-ignore
			(
				'aaa'  in  bbb // t
			);
		;
	) {}

	for (
		ccc +=
			// prettier-ignore
			(
				'aaa'  in  bbb // t
			);
		;
	) {}

	// The declarator twin, which reaches the same shell through the header's own builder.
	for (
		let ddd =
			// prettier-ignore
			(
				'aaa'  in  bbb // t
			);
		;
	) {}

	// The UNFROZEN forms answer the pair identically — the retained shell is the whole
	// reason either of them prints just one.
	for (
		ccc = (
			'aaa' in bbb // t
		);
		;
	) {}

	for (
		let eee = (
			'aaa' in bbb // t
		);
		;
	) {}

	// The `/* … */` spelling in the same gap is not a divergence at all: the clause `;`
	// terminates no statement, so the block strips inline and lands behind the `[~In]` pair,
	// at the assignment host, its compound form and the declarator alike — prettier's own
	// answer on every one of them.
	for (
		ccc =
			// prettier-ignore
			('aaa'  in  bbb) /* t */;
		;
	) {}

	for (
		ccc +=
			// prettier-ignore
			('aaa'  in  bbb) /* t */;
		;
	) {}

	for (
		let fff =
			// prettier-ignore
			('aaa'  in  bbb) /* t */;
		;
	) {}

	for (ccc = ('aaa' in bbb) /* t */; ;) {}
</script>
