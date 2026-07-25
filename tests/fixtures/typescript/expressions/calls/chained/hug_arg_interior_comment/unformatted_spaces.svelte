<script>
	// object arg: own-line comment before the first property
	const a =   obj.fn( {
			// c1
			a:   1
	} ).b;

	// object arg: own-line comment between properties
	const b = obj.fn({
		a: 1  ,
				// c2
		b:    2
	}).b;

	// object arg: own-line comment after the last property
	const c = obj.fn({
			a: 1
		// c3
	}).b;

	// object arg: own-line block comment before the first property
	const d = obj.fn({
		/* c4 */
			a: 1
	}).b;

	// array arg: own-line comment before the first element
	const e = obj.fn( [
		// c5
			a
	] ).b;

	// array arg: own-line comment between elements
	const f = obj.fn([
			a  ,
		// c6
				b
	]).b;

	// empty object arg: the dangling comment is the whole body
	const g = obj.fn( {
			// c7
	} ).b;

	// empty array arg: the dangling comment is the whole body
	const h = obj.fn( [
		// c8
			] ).b;
</script>
