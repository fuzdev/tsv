<script lang="ts">
	// Line comment trailing first type parameter
	type T<A,// trailing
		B> = A|B;

	// Line comment trailing multiple type parameters
	type U<A,// trailing A
		B,// trailing B
		C> = A|B|C;

	// Trailing on last type parameter
	type V<A,B// trailing
	> = A|B;
</script>
