<script>
	let a = $state(1);
	let b = $state.snapshot(a);
</script>

{#if b}<p>yes</p>{:else}<p>no</p>{/if}
