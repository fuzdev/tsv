<script>
	let v = $state('');
	let c = $state(false);
	let g = $state('a');
	let obj = $state({ x: 1 });
	let x = $state(1);
	let el = $state();
</script>

<input bind:value={v} class:x={c} />
