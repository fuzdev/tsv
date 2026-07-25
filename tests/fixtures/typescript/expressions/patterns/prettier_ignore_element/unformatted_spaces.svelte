<script lang="ts">
	// an own-line directive in an array-pattern element gap freezes only that element
		const [
			{ a:   a1 }  ,
		// prettier-ignore
		{ b:   b1 },
			c1  
	] = arr;

	// before the first element
		const [
		// prettier-ignore
		{ a:   a2 },
			b2
	] = arr;

	// an own-line block comment behaves identically
		const [
		/* prettier-ignore */
		{ a:   a3 },
			b3
	] = arr;

	// a rest element rides inside the frozen slice
		const [
			a4,
		// prettier-ignore
		...  rest
	] = arr;

	// a destructuring parameter host
		function fn([
		// prettier-ignore
		{ a:   a5 },
			b5
		]) {}

	// a block comment glued before a frozen element is owned by it and stays put
		const [
		// prettier-ignore
		/* c */ { a:   a6 },
			b6
	] = arr;
</script>
