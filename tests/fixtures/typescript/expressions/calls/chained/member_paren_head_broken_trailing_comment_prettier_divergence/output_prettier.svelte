<script lang="ts">
	// A paren head whose interior BREAKS, with a comment written after its kept `)`:
	// the comment trails the `)` on the line the pair closes on, exactly where it
	// stays when the head fits on one line, and the member breaks below it.

	// a cast whose operand holds a `//`
	(
		x() as T // c1
	) // c2
	.h;

	// `typeof`, `await` and binary heads
	(typeof (
		x // c4
	)) // c5
	.h;
	async function fn() {
		(
			await x // c6
		) // c7
		.h;
	}
	(
		a +
		b + // c8
		c
	) // c9
	.h;

	// a ternary head broken by width (the authored variant writes its comment before the `)`)
	(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
		? bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
		: cccccccccccccccccccccccc
	) // c10
	.d;

	// a block comment before a called member, a computed and an optional link, a `//`
	// before a called member and a longer chain
	(
		x as T // c11
	) /* c12 */
		.h();
	(
		x as T // c13
	) // c14
	[k];
	(
		x as T // c15
	) // c16
	?.h;
	(
		x as T // c30
	) // c31
		.h();
	(
		x as T // c32
	).h // c33
		.i()
		.j();

	// a chain head whose own chain breaks, at a unary and a binary operand
	v = !(!a /* c17 */
		.b()
		.c()) /* c18 */
		.b()
		.c();
	a ==
		(
			a ==
			a /* c19 */
				.b()
				.c()
		) /* c20 */
			.b()
			.c();

	// at `return`, as an assignment value, a call argument, a declarator and an `if` test
	function f() {
		return (
			x as T // c21
		) // c22
		.h;
	}
	w = (
		x as T // c23
	) // c24
	.h;
	g(
		(
			x as T // c25
		) // c26
		.h
	);
	const u = (
		x as T // c34
	) // c35
	.h;
	if (
		(
			x as T // c36
		) // c37
		.h
	) {
	}

	// at `yield` and `throw`: a stripped shell's `)` behind a multi-line block, and a
	// block glued ahead of a `//` after the `)`
	function* gen() {
		yield [p.q] /** c45
		 */.h // c46
			.i()
			.j();
	}
	function thr() {
		throw (() => p.q) /* c47 */ // c48
			.h();
	}

	// a cast broken by width alone, as a declarator's value
	const t = (
		aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa as Tttttttttttttttttttttttttttttttttttttt
	) // c38
	.h;

	// a stripped shell (the authored variant puts its `)` on a line of its own): the
	// comment after it trails the head, in both spellings
	aaa // c27
	.h;
	aaa /* c39 */
		.h();

	// a stripped shell over a call takes the bare call's trailing-member collapse
	yy().h; // c44

	// a second comment after the `)` never joins a trailing `//` before it (no weld), and
	// never prints ahead of an own-line comment before it (no reorder)
	// c41
	(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
		? bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
		: cccccccccccccccccccccccc
	) // c40
	.d;
	// c42
	// c43
	(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
		? bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
		: cccccccccccccccccccccccc
	).d;
</script>
