<script lang="ts">
	// A block comment leading the comparison stays inside the pair
	fn((/* k */ a < b), c > (d, e));
	const a1 = [(/* k */ a < b), c > (d, e)];
	((/* k */ a < b), c > (d, e));
	// A block comment ahead of the pair stays ahead of it
	fn(/* k */ (a < b), c > (d, e));
	const a2 = [/* k */ (a < b), c > (d, e)];
	x = (y, /* k */ (a < b), c > (d, e));
	// A block comment behind the pair stays ahead of the comma
	fn((a < b /* k */), c > (d, e));
	const a3 = [(a < b /* k */), c > (d, e)];
	((a < b /* k */), c > (d, e));
	// A line comment behind the pair ends the element's line
	fn(
		(a < b // k
		),
		c > (d, e)
	);
	const a4 = [
		(a < b // k
		),
		c > (d, e)
	];
	((a < b // k
	), c > (d, e));
	// A line comment glued to the list's opening delimiter keeps that line
	fn( // k
		(a < b),
		c > (d, e)
	);
	const a5 = [ // k
		(a < b),
		c > (d, e)
	];
</script>
