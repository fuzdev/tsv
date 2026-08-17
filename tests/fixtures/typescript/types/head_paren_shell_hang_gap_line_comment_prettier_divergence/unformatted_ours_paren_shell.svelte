<script lang="ts">
	// a function type's `=>` gap, over an array suffix
	type A = () => ( // c1
		B
	)[];

	// the same gap over an indexed access
	type C = () => ( // c2
		D
	)['k'];

	// the same gap over a conditional's check type
	type E = () => ( // c3
		F
	) extends G ? H : I;

	// a type-parameter `extends` constraint, whose value takes a REQUIRED pair
	type J<
		K extends (
			// c4
			L
		) extends M ? N : O
	> = K;
</script>
