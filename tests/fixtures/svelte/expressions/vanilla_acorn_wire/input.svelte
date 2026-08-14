<script>
	const expr = { with: { type: 'json' } };
	function fn(a) {
		return a;
	}

	// Script island: vanilla acorn's shape
	const a = import('./a', expr);
	const b = {
		get c() {
			return 1;
		}
	};
</script>

<!-- Template expression island -->
{import('./d')}
{import('./e', expr)}
{fn({
	get g() {
		return 1;
	}
})}

<!-- {@const} island -->
{#if expr}
	{@const h = {
		get i() {
			return 1;
		}
	}}
	{fn(h)}
{/if}
