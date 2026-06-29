<script lang="ts">
	// call args: blank line before a trailing comment (before `)`) is preserved
	fn(
		a,
		b

		/* c */
	);

	// new args: blank before a trailing comment (before `)`) is preserved
	new C(
		a,
		b

		/* n */
	);

	// array: blank before a trailing comment (before `]`) is preserved
	const arr = [
		x,
		y

		/* a */
	];

	// tuple: blank before a trailing comment (before `]`) is preserved
	type Tup = [
		A,
		B

		/* d */
	];

	// function params: blank before a trailing comment (before `)`) is preserved
	function f(
		a,
		b

		/* e */
	) {}

	// function-type params: blank before a trailing comment (before `)`) is preserved
	type F = (
		a: number,
		b: number

		/* g */
	) => void;

	// method signature params: blank before a trailing comment (before `)`) is preserved
	interface I {
		m(
			a: number,
			b: number

			/* h */
		): void;
	}
</script>
