<script>
	let x = $state(true);
	let w = $state(1);
</script>

<div class={w} class:foo={x}>text</div>

<style>
	.foo {
		color: red;
	}
</style>
