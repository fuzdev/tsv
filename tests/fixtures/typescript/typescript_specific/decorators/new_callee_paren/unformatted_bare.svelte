<script lang="ts">
	// a decorated class as the whole `new` callee breaks its pair open
	new @dec class {}();
	a = new @dec() class A extends B {}(x, y);
	b = new @dec.d @e class {}<T>().x;
	c = new new @dec class {}()();
	fn(new @dec class {}());
</script>
