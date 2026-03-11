<script lang="ts">
	// 100 boundary - fits on one line
	const a = await import(fn('aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa'));

	// 101 boundary - outer import breaks, inner call stays flat
	const b = await import(fn('aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa'));

	// short control - well under 100
	const c = await import(fn('./a'));

	// nested member call at 101
	const d = await import(a.b('aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa'));
</script>
