<script>
	// an own-line comment authored before the comma leads the next property
	const a = { property1: 1
// c1
, property2: 2 };

	// a same-line trailing comment keeps its line; the own-line one leads the next property
	const b = { property1: 1 // c1
// c2
, property2: 2 };

	// an own-line block comment before the comma leads the next property too
	const c = { property1: 1
/* c1 */
, property2: 2 };

	// a blank line the author left before the own-line comment is preserved
	const d = { property1: 1

// c1
, property2: 2 };
</script>
