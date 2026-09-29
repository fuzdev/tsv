<script>
	// a comment written behind a `//` in the interpolation's trailing run starts its own line
	const a = `${
		x || y // c1
		 // d1
	}`;

	const b = `${
		x // c2
		 /* d2 */
	}`;

	const c = `${x /* c3 */ /* d3 */}`;
</script>

<p>
	{`${
		x || y // c4
		 // d4
	}`}
</p>
