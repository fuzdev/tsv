<script>
	// the pair's `//` flushes where the value ends, ahead of the comment written after it
	const a = `${
		x || y // c1
		// d1
	}`;

	const b = `${
		f(() => x || y) // c2
		// d2
	}`;

	const c = `${
		x || y // c3
		/* d3 */
	}`;

	// no comment after the value: the closer's own break takes it
	const d = `${
		x || y // c4
	}`;

	// a hugged arrow the interpolation does not wrap: the call breaks and takes it
	const e = `${f(
		() => x || y // c6
	)}`;
</script>

<p>
	{`${
		x || y // c5
		// d5
	}`}
</p>

<p>
	{`${f(
		() => x || y // c7
	)}`}
</p>
