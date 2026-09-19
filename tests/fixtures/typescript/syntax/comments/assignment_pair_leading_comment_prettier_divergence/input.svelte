<script lang="ts">
	// A block comment glued to an assignment prints inside the clarity parens the position
	// adds, whether or not the author wrote them.

	// single-line block, at each position that parenthesizes an assignment
	const x1 = (/* c */ a = b);
	class C1 {
		p = (/* c */ a = b);
	}
	const o1 = { k: (/* c */ a = b) };
	function f1(p = (/* c */ a = b)) {}
	const { p1 = (/* c */ a = b) } = o;
	const g1 = () => (/* c */ a = b);
	for (const x of (/* c */ a = b)) {
		fn();
	}
	f((/* c */ a = b));
	const r1 = [(/* c */ a = b)];
	new F((/* c */ a = b));
	const t1 = `${(/* c */ a = b)}`;
	const o2 = { [(/* c */ a = b)]: 1 };
	o[(/* c */ a = b)];
	if ((/* c */ a = b)) {
		fn();
	}
	const x2 = c ? (/* c */ a = b) : d;
	function* g2() {
		yield (/* c */ a = b);
	}

	// multi-line block
	const x3 = (/* c
	d */ a = b);
	class C3 {
		p = (/* c
		d */ a = b);
	}
	const o3 = {
		k: (/* c
		d */ a = b)
	};
	const g3 = () =>
		(/* c
		d */ a = b);
	f(
		(/* c
		d */ a = b)
	);
	const r3 = [
		(/* c
		d */ a = b)
	];
	new F(
		(/* c
		d */ a = b)
	);
	const t3 = `${
		(/* c
		d */ a = b)
	}`;
	if (
		(/* c
		d */ a = b)
	) {
		fn();
	}
</script>
