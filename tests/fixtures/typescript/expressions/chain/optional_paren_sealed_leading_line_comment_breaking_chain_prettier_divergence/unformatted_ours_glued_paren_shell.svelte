<script lang="ts">
	// A `//` glued to a sealed base's `(` keeps the `(` line and expands the pair; a
	// chain that breaks inside it keeps its own continuation — the operand one indent
	// in, its links two — the depths the comment-free base already takes.
	(// c1
	a?.m().m().c(() => d)).z;
	(// c2
	a?.m().m().c(() => d))!.z;
	const v = (// c3
	a?.m().m().c(() => d)).z;
	(// c4
	a?.m().m().c(() => d)).f();

	// Nested positions: a comparison operand and an `if` test.
	a<(// c5
	p/* m */.c?.()).s;
	if ((// c6
	x?.().a(function(){x})).f()){}

	// A mid-chain `//` breaks the chain inside the shell.
	(// c7
	x// m
	?.()).n;

	// An own-line comment keeps its own line above the operand.
	(/* c8 */
	a?.m().m().c(() => d)).z;

	// Both gaps commented (fitting, then breaking): one shell, the trailing block beside
	// the operand.
	(((// c9
	a)?.x/* t */).h);
	(
	// c11
	a?.m().m().c(() => d)/* t */).h;

	// The bare callee takes the same shell.
	(// c10
	a?.m().m().c(() => d))();

	// A restricted production's argument keeps the shell on the keyword line.
	function f() {
		return (// c12
		a?.m().m().c(() => d)).z;
	}
</script>
