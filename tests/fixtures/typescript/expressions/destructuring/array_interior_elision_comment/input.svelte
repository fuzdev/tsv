<script>
	// A line comment past an elision belongs to the binding it follows, once
	const [
		a,
		,
		b // c1
	] = arr;

	// Two elisions between the bindings
	const [
		c,
		,
		,
		d // c2
	] = arr;

	// The commented binding is not the last slot
	const [
		e,
		,
		f, // c3
		g
	] = arr;

	// A block comment there trails the same binding
	const [h, , i /* c4 */] = arr;

	// In a parameter list
	function fn([
		j,
		,
		k // c5
	]) {
		return j;
	}

	// As an assignment target
	let l, m;
	[
		l,
		,
		m // c6
	] = arr;
</script>
