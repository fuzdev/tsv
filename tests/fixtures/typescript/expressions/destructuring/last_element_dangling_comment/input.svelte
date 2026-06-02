<script>
	// object pattern: own-line comments after last property
	const {
		a,
		// c1
		// c2
	} = obj1;

	// array pattern: own-line comments after last element
	const [
		b,
		// c1
		// c2
	] = arr1;

	// object pattern: interior comment then trailing comment after last property
	const {
		c,
		// between
		d,
		// after
	} = obj2;

	// array pattern: interior comment then trailing comment after last element
	const [
		e,
		// between
		f,
		// after
	] = arr2;
</script>
