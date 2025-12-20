<script lang="ts">
	const obj = {
		async empty() {},
		async withBody() {
			return await fetch();
		},
		async withParams(a, b) {
			return await process(a, b);
		},
		async *generator() {
			yield 1;
		},
		async *genWithBody() {
			yield await fetch();
		},
		async typed(): Promise<void> {},
		async typedParams(a: string): Promise<number> {
			return 1;
		},
		async *typedGen(): AsyncGenerator<number> {
			yield 1;
		},
	};
</script>
