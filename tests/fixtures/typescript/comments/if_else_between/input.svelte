<script lang="ts">
	// Comment between } and else (#17998)
	if (cond) {
		fn1();
	} // comment
	else {
		fn2();
	}

	// Inline comment after opening brace (#17998)
	if (a) {
		// comment
		fn3();
	}
</script>
