<script lang="ts">
	// short — arrow call fits inline
	const a = list.map(([m, n]) => [n, m]);

	// 100 — the hugged first line fits, so both formatters hug the arrow and break its array body
	const b = (objectExpr as Array<[ElementTypeAaaaaaaaaaaaaaaaaaaaaaaaaaaa, Bbbb]>).map(([m, n]) => [
		n as number,
		m
	]);

	// 101 — one char longer: the hugged first line would be 101, so the arrow drops to
	// its own line and the destructuring param must NOT break (all-args-broken-out)
	const c = (objectExpr as Array<[ElementTypeAaaaaaaaaaaaaaaaaaaaaaaaaaaa, Bbbbb]>).map(
		([m, n]) => [n as number, m]
	);
</script>
