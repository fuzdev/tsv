<script>
	// a blank line the author left before an elision is authorship and survives
	const [
		a1,

		,
		b1 // c1
	] = x1;

	// a blank AFTER an elision does not: a hole carries none, its line break is structure
	const [
		a2,
		,
		b2 // c2
	] = x2;

	// the blank still counts with the element's own trailing comment above it
	const [
		a3, // c3

		,
		b3 // c4
	] = x3;

	// ...and with a comment leading the next real binding below it
	const [
		a4,

		,
		// c5
		b4
	] = x4;

	// between two elisions there is nothing to measure either
	const [
		a5,
		,
		,
		b5 // c6
	] = x5;

	// ahead of the first of two elisions the blank survives, as with one
	const [
		a6,

		,
		,
		b6 // c7
	] = x6;
</script>
