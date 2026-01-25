<script>
	// Line comment between } and while - we preserve, prettier moves into condition
	do {
		fn();
	} while (
		// comment before while
		a
	);

	// With blank line before comment
	do {
		fn();
	} while (
		// blank before
		a
	);
</script>
