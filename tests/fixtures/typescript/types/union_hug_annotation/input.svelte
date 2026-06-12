<script lang="ts">
	// Hugged union in interface property - null first
	interface A {
		a: null | {
			b: T;
			c: U;
		};
	}

	// Hugged union in interface property - object first
	interface B {
		a: {
			b: T;
			c: U;
		} | null;
	}

	// Hugged union in variable declaration
	let x: null | {
		a: T;
		b: U;
	};

	// Hugged union with void
	interface C {
		a: {
			b: T;
			c: U;
		} | void;
	}

	// Hugged union with type reference + null stays inline
	interface D {
		a: Map<string, number> | null;
	}
</script>
