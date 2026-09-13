<script lang="ts">
	// A multiline block comment trailing the function type inside the disambiguating
	// pair keeps the pair open, as a `//` there does.
	const a =
		(
			x: T
		): ((y: T) => T) /* m1
	m2 */ =>
		(y) =>
			y;

	// A leading `//` glued to the `(` keeps that line; the pair still holds both.
	const b =
		(
			x: T
		): // c1
		((y: T) => T) /* m3
	m4 */ =>
		(y) =>
			y;

	// An own-line leading comment keeps its line inside the pair.
	const c =
		(
			x: T
		): // c2
		((y: T) => T) /* m5
	m6 */ =>
		(y) =>
			y;
</script>
