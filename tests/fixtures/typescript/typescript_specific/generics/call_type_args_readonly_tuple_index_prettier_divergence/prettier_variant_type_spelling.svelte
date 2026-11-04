<script lang="ts">
	// A `readonly` tuple with an index behind it is a member access on the name `readonly`,
	// read in a comparison chain as acorn-typescript reads it; tsc's type reading of the same
	// list is its checker's TS1354, `readonly` over an indexed access
	a<readonly [0][1]>(b);
	fn<readonly [A]['k']>(x);
	fn<readonly [A][0]>(x, y);
	new Cls<readonly [A]['k']>(x);
	fn<readonly [A]['k'][]>(x);
</script>

<p>{fn<readonly [A]['k']>(x)}</p>
