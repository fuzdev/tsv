<script lang="ts">
	// Block comment between `:` and the return type
	const a =
		(x: T): /* c1 */ ((y: T) => T) =>
		(y) =>
			y;

	// Block comment inside the disambiguating parens, leading the function type
	const b =
		(x: T): /* c2 */ ((y: T) => T) =>
		(y) =>
			y;

	// Block comment inside the parens, trailing the function type
	const c =
		(x: T): ((y: T) => T) /* c3 */ =>
		(y) =>
			y;

	// Line comment between `:` and the return type
	const d =
		(
			x: T
		): // c4
		((y: T) => T) =>
		(y) =>
			y;

	// Line comment inside the parens, trailing the function type
	const e =
		(
			x: T
		): ((y: T) => T) => // c5
		(y) =>
			y;
</script>
