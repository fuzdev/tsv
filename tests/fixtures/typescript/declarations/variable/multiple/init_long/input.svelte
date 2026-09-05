<script>
	// A non-first declarator's chain breaks after `=`, its operands one level past the declarator
	const a = fn(),
		b =
			aaaaaaaaaaaaaaaaaaaaaaaaaa &&
			bbbbbbbbbbbbbbbbbbbbbbbbbbbbbb &&
			ccccccccccccccccccccccccccccc &&
			ddddddddddd;

	// The first declarator's chain hangs the same way: two levels past the keyword
	const c =
			aaaaaaaaaaaaaaaaaaaaaaaaaa &&
			bbbbbbbbbbbbbbbbbbbbbbbbbbbbbb &&
			ccccccccccccccccccccccccccccc &&
			ddddddddddd,
		d = 1;

	// A call's arguments expand one level past the declarator, the `)` at the declarator's column
	const e = 1,
		f = fn(
			aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,
			bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb,
			ccccccccccc
		);

	// A ternary's branches one level past the declarator
	const g = 1,
		h = cccccccccccccccccccccccccccccccc
			? dddddddddddddddddddddddddddddddddddd
			: eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee;

	// A string breaks after `=`
	const i = 1,
		j =
			'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa';

	// 100 chars - the chain stays on the declarator line
	const k = 1,
		l = aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa && bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb && cccccccc;

	// 101 chars - breaks after `=`
	const m = 1,
		n =
			aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa && bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb && ccccccccc;
</script>
