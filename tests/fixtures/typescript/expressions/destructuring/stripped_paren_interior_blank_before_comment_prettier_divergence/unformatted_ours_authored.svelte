<script>
	// a blank the author wrote ahead of a shell-interior comment is authorship: it
	// survives the shell's erasure and separates the comment from the item above,
	// wherever the list is open for a reason of its own
	const {
		a1 = (1

		/* c */),
		b1,
		c1 // holds the pattern open
	} = x;

	function fn(
		a2 = (1

		/* c */),
		b2,
		c2 // holds the list open
	) {}
</script>
