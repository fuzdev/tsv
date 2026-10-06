<script lang="ts">
	// A `<` in one comma sibling and a `>` ahead of a paren or a template in a later one
	// are two comparisons wherever the tokens between them spell no type-argument list.

	// object properties: a key's `:` stands between the siblings
	const a1 = { p: a < b, q: c > (d - e) * 2 };
	const a2 = { p: a < b, 'q-r': c > (d, e) };
	const a3 = { p: a < b, [q]: c > `t` };
	const a4 = { p: a < b, q: c => c > (d - e) * 2 };

	// declarators, parameter defaults, destructuring defaults and enum members: an `=`
	const b1 = a < b,
		b2 = c > (d, e);
	let b3: boolean = a < b,
		b4: boolean = c > (d - e) * 2;
	function fn1(p = a < b, q = c > (d - e) * 2) {}
	const fn2 = (p = a < b, q = c > (d, e)) => p;
	const { p1 = a < b, q1 = c > (d, e) } = obj;
	const [p2 = a < b, q2 = c > (d - e) * 2] = arr;
	enum E {
		A = a < b,
		B = c > (d - e) * 2
	}
	for (let i = 0, p3 = a < b, q3 = c > (d - e) * 2; i < n; i++) {}

	// call arguments and array elements: an operator, a call or a member no type takes
	fn(a < b, c + d > (e, f));
	fn(a < b, c() > (d - e) * 2);
	fn(a < b, c?.d > (e, f));
	fn(a < b, c ? d : e > (f, g));
	fn(a < b, c as T > (d, e));
	fn(a < b, await c > (d, e));
	fn(a < b, -c > (d, e));
	fn(a < b, c in d > (e, f));
	fn(a < b, c && d > (e, f));
	fn(a < b, c + d > `t`);
	const c1 = [a < b, c * d > (e, f)];
	new Cls(a < b, c + d > (e, f));

	// a paren pair the printer keeps, ahead of a token no type continues with
	fn(a < b, (c & d) !== 0 && e > (f, g));

	// the `<` side may hold any operand a type could spell, and a sibling may stand between
	fn(a.b < c, d + e > (f, g));
	fn(a < 1, b + c > (d, e));
	fn(a < b, c, d + e > (f, g));
	fn(new Cls < a, b + c > (d, e));
</script>
