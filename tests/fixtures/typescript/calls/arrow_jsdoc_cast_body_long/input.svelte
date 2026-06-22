<script>
	// Short: fits inline
	const a = b.map((x) => /** @type {A} */ (fn(x)));

	// Long: cast paren is opaque, so standard arg breaking (params on own line)
	const aaaa = b.cccccccccccccccccccccccccc.map(
		(x) => /** @type {AAAAAAAAAAAAAAAAAAAAAAAA} */ (fn(x))
	);
</script>
