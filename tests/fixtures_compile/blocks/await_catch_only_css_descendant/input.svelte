<script>
	let p = Promise.resolve(1);
</script>

{#await p}
	<p>l</p>
{:then v}
	<p>{v}</p>
{:catch e}
	<div><span class="in">{e}</span></div>
{/await}

<style>
	div .in {
		color: red;
	}
</style>
