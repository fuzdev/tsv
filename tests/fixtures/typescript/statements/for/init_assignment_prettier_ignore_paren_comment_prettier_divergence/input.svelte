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
		ccc =
			(
				'aaa' in bbb // t
			);
		;
	) {}

	for (
		let eee =
			(
				'aaa' in bbb // t
			);
		;
	) {}
</script>
