<script>
	// Expand-first with after-comma JSDoc cast on second arg
	a.b((x) => {
		return x;
	}, /** @type {T} */ c);

	// Expand-first with regular block comment after comma
	a.b((x) => {
		return x;
	}, /* cast */ c);
</script>
