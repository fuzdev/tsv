<script>
	let y = $state(0);
	let w = $state(0);
	function h() {}
</script>

<svelte:window bind:scrollY={y} onkeydown={h} bind:innerWidth={w} />
