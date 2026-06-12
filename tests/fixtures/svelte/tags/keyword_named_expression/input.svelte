<script>
	let html = '';
	let attach = (el) => el;
	let render = '';
	let key = 1;
	let promise = Promise.resolve(1);
</script>

<!-- expression identifier shares the tag keyword's name; spans point at the identifier -->
<span>{@html html}</span>
<div {@attach attach}></div>
<span>{@html render + html}</span>
{#key key}
	<p>block1</p>
{/key}
{#await promise}
	<p>block2</p>
{:then then}
	<p>{then}</p>
{/await}
{#snippet snippet()}
	<p>block3</p>
{/snippet}
{@render snippet()}
