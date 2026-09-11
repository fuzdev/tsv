<script lang="ts">
	// variable annotation: a multi-line block on its own line keeps the break after it, and
	// the type drops below it
	let a1: /* c
d */
	Ooo;

	// the indentable flavor
	let a2: /* c
	 * d */
	Ooo;

	// an author blank after the comment survives with the break
	let a3: /* c
d */

	Ooo;

	// function and arrow return types
	function f1(): /* c
d */
	Ooo {}
	const f2 = (): /* c
d */
	Ooo => b;

	// index signature, class property, parameter
	interface I {
		[k: string]: /* c
d */
		Ooo;
	}
	class A {
		p: /* c
d */
		Ooo;
	}
	function f3(
		a: /* c
d */
		Ooo
	) {}
</script>
