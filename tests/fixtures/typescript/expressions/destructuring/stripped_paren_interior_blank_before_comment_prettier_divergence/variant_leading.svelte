<script>
	// a blank the author wrote ahead of a shell-interior comment is authorship: it
	// survives the shell's erasure and separates the comment from the item above
	const { /* c */ a1 = 1, b1 } = x;

	function fn(/* c */ a2 = 1, b2) {}
</script>
