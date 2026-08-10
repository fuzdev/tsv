<script>
	// a blank the author wrote ahead of a shell-interior comment is authorship: it
	// survives the shell's erasure and separates the comment from the item above
	const {
		a1 = 1,

		/* c */ b1
	} = x;

	function fn(
		a2 = 1,

		/* c */
		b2
	) {}
</script>
