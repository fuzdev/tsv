<script lang="ts">
	// a decorated class heads a `new` callee bare where a member access on it is the callee
	new @dec
	class {}.k();
	a = new @dec()
	class A extends B {}[0](x, y);
	b = new @dec.d
	@e
	class {}.k<T>().x;
	c = new @dec
	class {}!();
	fn(
		new @dec
		class {}.k()
	);
</script>
