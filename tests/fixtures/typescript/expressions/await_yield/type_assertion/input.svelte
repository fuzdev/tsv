<script lang="ts">
	// await expression in as/satisfies context
	const a = (await fn()) as T;
	const b = (await fn()) satisfies T;
	const c = (await fn()) as unknown as T;

	// as/satisfies expression as await argument
	const d = await (fn() as T);
	const e = await (fn() satisfies T);

	// method chains
	const f = (await fn().method()) as T;
	const g = await (fn().method() as T);

	// yield in generator
	function* gen() {
		const h = (yield fn()) as T;
		const i = (yield fn()) satisfies T;
	}
</script>
