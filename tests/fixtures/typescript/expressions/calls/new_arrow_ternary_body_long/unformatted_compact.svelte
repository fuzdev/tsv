<script lang="ts">
	// 100 chars - fits on one line with parens
	const a = new Xy((x) => (x ? aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa : bbbbbbbbbbbbbbbbbbbbbbb));

	// 101 chars - breaks after =>
	const b = new Xy((x) => (x ? aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa : bbbbbbbbbbbbbbbbbbbbbbb));

	// with await - 101 chars
	const c = await new Xy((x) => (x ? aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa : bbbbbbbbbbbbbbbbbbbb));
</script>
