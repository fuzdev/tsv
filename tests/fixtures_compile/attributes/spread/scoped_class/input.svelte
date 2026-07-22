<script>
	let props = $state({});
	let x = $state(1);
</script>

<div class:foo={x} {...props}></div>

<style>
	.foo {
		color: red;
	}
</style>
