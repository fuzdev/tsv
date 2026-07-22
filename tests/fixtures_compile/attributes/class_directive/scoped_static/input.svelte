<script>
	let x = $state(true);
</script>

<div class="foo" class:active={x}>text</div>

<style>
	.foo {
		color: red;
	}
</style>
