<script lang="ts">
	// Call body - the call expands, matching prettier
	fn(
		(
			a /* multi
	line */,
			b
		) => call(a)
	);

	// A member chain's call-body argument too
	arr.map(
		(
			a /* multi
	line */,
			b
		) => call(a)
	);

	// Object body - the signature breaks, the call keeps hugging it
	fn((
		a /* multi
	line */,
		b
	) => ({ x: a }));

	// Block body - same
	fn((
		a /* multi
	line */,
		b
	) => {
		call(a);
	});

	// A member chain's block-body argument too
	arr.map((
		a /* multi
	line */,
		b
	) => {
		call(a);
	});

	// A function expression argument too
	fn(function (
		a /* multi
	line */,
		b
	) {
		call(a);
	});
</script>
