<script>
	let promise = Promise.resolve(1);
</script>

{#await promise}
	<p>loading</p>
{:then v}
	<p>{v}</p>
{:catch e}
	<p class="err">{e}</p>
{/await}

<style>
	.err {
		color: red;
	}
</style>
