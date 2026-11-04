<script lang="ts">
	// an authored pair makes a decorated class an expression, not a declaration: the pair stays
	export default (
		@dec
		class A {}
	);
	export default (
		@dec
		class {}
	);

	// two decorators and a body, one per line inside the pair
	export default (
		@dec
		@dec2()
		class B {
			m() {}
		}
	);

	// a line comment inside the pair, after the class, stays inside it
	export default (
		@dec
		class C {} // c1
	);
	export default (
		@dec
		class D {}
		// c2
	);
	export default (
		@dec
		class G {} // c5
	); // c6

	// a block comment there prints past `);`, as for an undecorated class
	export default (
		@dec
		class E {}
	); /* c3 */
	export default (
		@dec
		class H {}
	);
	/* c7 */

	// a comment glued to the first decorator stays inside the pair
	export default (
		/* c4 */ @dec
		class F {}
	);
</script>
