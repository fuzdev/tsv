<script lang="ts">
	// a `//` in a stripped shell at the end of a paren head, then a comment after the `)`:
	// a block, a glued block, a `//`, and a block glued ahead of a `//`
	v = (a + (b // c1
	)
	) /* c2 */
	.h();
	v = (a + (b // c3
	)) /* c4 */.h();
	v = (a + (b // c5
	)
	) // c6
	.h;
	v = (a + (b // c7
	)
	) /* c8 */ // c9
	.h();

	// no comment after the `)`, across the links
	v = (a + (b // c10
	)).h;
	v = (a + (b // c11
	))[k];
	v = (a + (b // c12
	))?.h;
	v = (a + (b // c13
	)).h.i().j();

	// an own-line `//`, a block and a multi-line block in the shell
	v = (a + (b
	// c14
	)) /* c15 */.h();
	v = (a + (b /* c16 */)) /* c17 */
	.h();
	v = (a + (b /** c18
	 */)) // c19
	.h;

	// a logical chain, nested shells, and down a right spine
	v = (a && (b // c20
	)) /* c21 */
	.h();
	v = (a + ((b // c22
	))) /* c23 */
	.h();
	v = (a + (c * (b // c24
	))) /* c25 */
	.h();
	v = ((m) => (k) => y + (b // c26
	)) /* c27 */
	[k];
	v = (c ? d : e + (b // c28
	)) /* c29 */
	.h();
	v = (x = a + (b // c30
	)) /* c31 */.h();
	function* gen() {
		v = (yield a + (b // c32
		)) /* c33 */.h();
	}

	// a head whose binary breaks after a multi-line operand lays out as its comment-free twin
	v = (h(() => {
	throw x;
	}) + (b // c34
	)).h();

	// a kept pair holds its own comment: one the operator requires, and a unary's
	v = (a - (b - c // c35
	)) /* c36 */.h();
	v = (a + typeof (x // c37
	)) /* c38 */.h();

	// at `return`, `throw`, `yield`, a declarator, a call argument and an `if` test
	function f() {
		return (a + (b // c39
		)
		) // c40
		.h;
	}
	function g() {
		throw (a + (b // c41
		)
		) /* c42 */
		.h();
	}
	function* y() {
		yield (a + (b // c43
		)
		) // c44
		[k];
	}
	const u = (a + (b // c45
	)
	) /* c46 */
	.h;
	q((a + (b // c47
	)
	) // c48
	.h);
	if ((a + (b // c49
	)
	) /* c50 */
	.h) {
	}
	function k() {
		return (a + (b /** c51
		 */)) /* c52 */.h();
	}

	// in a callback
	foo(function () {
		return (a + (b // c53
		)
		) /* c54 */
		.h;
	});
</script>

<p class={(a || (b // c55
)).m}>t</p>
<p class={(a || (b
// c56
)).m}>t</p>
<p class={(a || (b // c57
)) /* c58 */.m}>t</p>
<p>{(a || (b // c59
)).m}</p>
{#if (a || (b // c60
)).m}
	<p>x</p>
{/if}
