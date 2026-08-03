<script lang="ts">
	// Leading line comment before `=` with type params: kept before `=`; the union
	// breaks (interleaved member comments) with members at the `=` level, not deeper.
	type A<E, EM, D>
		// c0
		=
		| { type: 'a' }
		// c1
		| { type: 'b'; error: E; msg: EM }
		// c2
		| { type: 'c'; data: D };

	// Same without type params.
	type B
		// c0
		=
		| { type: 'a' }
		// c1
		| { type: 'b' }
		// c2
		| { type: 'c' };
</script>
