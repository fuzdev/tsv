<script>
	// own-line block comment after `=` breaks after `=`
	for (
		let i =
			/* c1 */
			0;
		i < 10;
		i++
	) {}

	// own-line line comment after `=` breaks after `=`
	for (
		let j =
			// c2
			0;
		j < 10;
		j++
	) {}

	// inline block comment after `=` stays glued, header inline
	for (let k = /* c3 */ 0; k < 10; k++) {}
</script>
