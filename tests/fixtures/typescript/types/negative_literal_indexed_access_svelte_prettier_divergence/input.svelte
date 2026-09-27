<script lang="ts">
	// a bare negative literal type ahead of an index: an indexed access on the literal type
	type A = -1[K];
	type B = -1n['a'];
	type C = -1[K][K];

	// a pair the author wrote around the run
	type D = (-1[K])[];

	// in a type argument
	let a = f<-1[K]>(x);
</script>
