<script>
	let props = $state({});
	let x = $state(1);
	let v = $state('');
	let w = $state('');
</script>

<input class:a={x} style:color={v} bind:value={w} {...props} />
