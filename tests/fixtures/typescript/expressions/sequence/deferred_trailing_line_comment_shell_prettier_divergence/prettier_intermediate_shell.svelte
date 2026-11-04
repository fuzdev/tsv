<script lang="ts">
	// A line comment inside the last operand's own redundant pair is the same deferred
	// run: the sequence's parens close before it, so the operand run stays on one line.
	(a,
		b); // c1

	x =
		(a,
		(b,
		c)); // c2

	(a,
		b <
			c); // c3

	f(
		(a,
		b) // c4
	);

	o = {
		k:
			(a,
			b) // c5
	};

	const y =
		(a,
		b); // c6

	// In a statement header's test and a ternary's test the comment ends its line inside the
	// construct, before its closer or the `?`.
	if (
		(a,
		b) // c7
	) {
	}

	if (
		(a,
		b) // c8
	) {
	}

	while (
		(a,
		b) // c9
	) {}

	switch (
		(a,
		b) // c10
	) {
	}

	x = (a,
	b) // c11
		? d
		: e;
</script>
