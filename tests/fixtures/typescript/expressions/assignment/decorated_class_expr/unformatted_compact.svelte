<script lang="ts">
	let C, D, E, M, obj;
	C = @dec class {};
	D = @dec.foo class extends Base {};
	E = @dec() class {
		a = 1;
	};
	M = @a @b class {};
	// property-value position routes through the same assignment-layout engine
	obj = { k: @dec class {} };
</script>
