<script>
	let { active = false, label } = $props();
	let count = $state(0);
	let doubled = $derived(count * 2);
	const options = { active: active, doubled: doubled, label: label };
	function read(source) {
		const { count: count = 1 } = source;
		return { count: count };
	}
	function describe(o) {
		return `${o.label}`;
	}
</script>

<p title={describe({ label: label })}>{options.doubled}{read({}).count}</p>
