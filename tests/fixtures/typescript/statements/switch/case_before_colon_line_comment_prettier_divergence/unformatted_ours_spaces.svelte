<script>
	// A line comment before `:` stays after the head; `:` drops one level in.
	switch (a) {
		case x // c
		:
			b();
			break;
		default // c2
		:
			b();
	}

	// A run stays in place, in order, each comment distinct.
	switch (d) {
		case y // c3
		// c4
		:
			b();
	}
</script>
