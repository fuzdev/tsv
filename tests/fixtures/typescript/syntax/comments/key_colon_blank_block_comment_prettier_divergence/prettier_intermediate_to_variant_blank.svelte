<script lang="ts">
	// function declaration parameter
	function f1(
		a /* c */

		: number
	) {}

	// value arrow parameter
	const f2 = (
		a /* c */

		: string
	) => a;

	// declare function
	declare function f3(
		a /* c */

		: number
	): void;

	// interface method
	interface I {
		m(
			a /* c */

			: number
		): void;
	}

	// function type
	type F = (
		a /* c */

		: number
	) => void;

	// optional marker: the `?`→`:` gap
	function f4(
		a? /* c */

		: number
	) {}

	// variable binding
	let v /* c */

	: number;

	// class property
	class C {
		p /* c */: number;
	}

	// type-literal member
	type L = {
		p /* c */: number;
	};

	// a run: the blank authored after the last comment
	let r1 /* c1 */ /* c2 */

	: number;

	// a run: the blank authored between the comments
	let r2 /* c1 */

	/* c2 */ : number;
</script>
