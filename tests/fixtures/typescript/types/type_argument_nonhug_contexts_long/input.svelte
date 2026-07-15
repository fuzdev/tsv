<script lang="ts">
	// Every context that reaches the non-wrapping type-argument builder applies the same
	// hug rule, so a non-hugging single type arg breaks the `<...>` in each of them.

	// 100 chars - fits inline at the boundary, so the `<...>` stays put
	declare const at100: Ref<AAAAAAAAAAAAAAAAAAAAAA & BBBBBBBBBBBBBBBBBBBBB & CCCCCCCCCCCCCCCCCCCC>[];

	// 101 chars - array element type breaks
	declare const arr: Ref<
		AAAAAAAAAAAAAAAAAAAAAAAAA & BBBBBBBBBBBBBBBBBBBBB & CCCCCCCCCCCCCCCCCCCCCC
	>[];

	// `typeof` query:
	declare const query: typeof fn<
		AAAAAAAAAAAAAAAAAAAAAAAAA & BBBBBBBBBBBBBBBBBBBB & CCCCCCCCCCCCCCCCCCCC
	>;

	// `import(...)` type with a qualifier:
	declare const imp: import('./a').Q<
		AAAAAAAAAAAAAAAAAAAAAAA & BBBBBBBBBBBBBBBBBBBB & CCCCCCCCCCCCCCCCCCCC
	>;

	// `keyof` operand:
	declare const key: keyof Ref<
		AAAAAAAAAAAAAAAAAAAAAAAAAA & BBBBBBBBBBBBBBBBBBBB & CCCCCCCCCCCCCCCCCCCC
	>;

	// Indexed-access object:
	declare const idx: Ref<
		AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA & BBBBBBBBBBBBBBBBBBBB & CCCCCCCCCCCCCCCC
	>['k'];
</script>
