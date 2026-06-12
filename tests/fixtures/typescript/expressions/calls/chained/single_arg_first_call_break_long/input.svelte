<script>
	// 100 chars: fits on one line (79 a's in template)
	fn(a.b()).call(`aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa`);

	// 101 chars: break at .call( keeping first call args inline (80 a's)
	fn(a.b()).call(
		`aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa`,
	);

	// Longer first arg: still breaks at .call( (not at fn)
	fn(longer_name.longer_method()).call(
		`aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa`,
	);
</script>
