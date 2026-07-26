<script lang="ts">
	// an own-line directive between two declarators freezes only the following one
	const a = 1,
		// prettier-ignore
		b   =   2,
		c = 3;

	// an own-line block comment behaves identically
	let d = 1,
		/* prettier-ignore */
		e   =   2;

	// a `var` host
	var f = 1,
		// prettier-ignore
		g   =   2;

	// the annotation rides inside the frozen slice
	const h: number = 1,
		// prettier-ignore
		i:   string   =   'text';

	// a destructuring declarator freezes whole
	const j = 1,
		// prettier-ignore
		{ k,   l } = obj;

	// a multi-line frozen declarator keeps its verbatim layout
	const m = 1,
		// prettier-ignore
		n = {
			o:   2,
				p: 3
		},
		q = 4;

	// an uninitialized declarator
	let r = 1,
		// prettier-ignore
		s   :   number;

	// a `for` header's init clause is the same declarator list
	for (
		let t = 0,
			// prettier-ignore
			u   =   1;
		;
	) {
		break;
	}
</script>
