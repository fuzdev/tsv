<script lang="ts">
	// 100 visual chars (98 content + 2 tab) - stays inline
	const aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa = fn(() => bbbb.cccccc(ddd, eee, fff));

	// 101 visual chars (99 content + 2 tab) - wraps, `fn(() =>` stays together
	const aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa = fn(() =>
		bbbb.cccccc(ddd, eee, fff),
	);

	// 100 visual chars with type annotation - stays inline
	const b: Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa = fn(() => bbbb.cccc(ddd, eee, fff));

	// 101 visual chars with type annotation - wraps, `fn(() =>` stays together
	const c: Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa = fn(() =>
		bbbb.cccc(ddd, eee, fff),
	);
</script>
