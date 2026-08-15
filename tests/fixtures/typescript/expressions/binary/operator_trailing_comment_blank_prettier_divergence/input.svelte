<script lang="ts">
	// arithmetic operator: the blank between the operator's trailing comment and the
	// own-line comment below it survives
	const a =
		x + // c1

		// c2
		y;

	// logical operator: the same
	const b =
		x && // c1

		// c2
		y;

	// a longer run keeps every blank the author wrote
	const c =
		x + // c1

		// c2

		// c3
		y;

	// both comments own-line: prettier keeps the blank too, so this is a control
	const d =
		x +
		// c1

		// c2
		y;
</script>
