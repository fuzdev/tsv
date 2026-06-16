<script lang="ts">
	// Outer has return type, inner does not - ALWAYS breaks
	const a =
		(x: T): H =>
		(y) =>
			x + y;

	// Inner has return type, outer does not - ALWAYS breaks
	const b =
		(x: T) =>
		(y): H =>
			x + y;

	// Neither has return type - stays inline
	const c = (x: T) => (y) => x + y;

	// Multiple arrows with return types - ALWAYS breaks
	const d =
		(x: T): U =>
		(y: V): W =>
		(z) =>
			expr;

	// Block body with outer return type
	const e =
		(x: T): H =>
		(y) => {
			return x + y;
		};

	// Block body with inner return type
	const f =
		(x: T) =>
		(y): H => {
			return x + y;
		};

	// Multiple params with outer return type
	const g =
		(a: A, b: B): Handler =>
		(c, d) => {
			return c;
		};

	// Multiple params with inner return type
	const h =
		(a: A, b: B) =>
		(c, d): Handler => {
			return c;
		};

	// Type parameters on outer arrow - ALWAYS breaks
	const i =
		<T extends A>(x: T) =>
		(y) =>
			x + y;

	// Type parameters on inner arrow - ALWAYS breaks
	const j =
		(x: T) =>
		<U extends B>(y: U) =>
			x + y;

	// Destructuring param - ALWAYS breaks
	const k =
		({a, b}) =>
		(y) =>
			a + b + y;

	// Array destructuring - ALWAYS breaks
	const l =
		([a, b]) =>
		(y) =>
			a + b + y;

	// Default param - ALWAYS breaks
	const m =
		(x = 1) =>
		(y) =>
			x + y;
</script>
