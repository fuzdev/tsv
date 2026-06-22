<script lang="ts">
	// Type argument, trailing comment
	type T = Generic<
		A & // c
			B
	>;

	// Type argument, own-line comment
	type U = Generic<
		A &
			// c
			B
	>;

	// Tuple element, trailing comment
	type V = [
		A & // c
			B,
		C
	];

	// Tuple element, own-line comment
	type W = [
		A &
			// c
			B,
		C
	];

	// Mapped value, trailing comment
	type X = {
		[K in Keys]: A & // c
			B;
	};

	// Mapped value, own-line comment
	type Y = {
		[K in Keys]: A &
			// c
			B;
	};
</script>
