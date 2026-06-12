<script lang="ts">
	// Multiple type params with long constraints
	function multiple<

		T extends VeryLongConstraintType,
		U extends AnotherLongConstraintType,
		V extends YetAnotherLongConstraintType,

	>(a: T[]): U[] {}

	// Type param with constraint AND default, both long
	function withDefault<

		T extends VeryLongInputConstraintType = DefaultVeryLongInputType,
		U extends VeryLongOutputConstraintType = DefaultVeryLongOutputType,

	>(a: T): U {}

	// Generic function with very long name + type params
	function functionNameThatExceedsPrintWidthWithTypeParameters<T extends LongConstraintType>(
		a: T[],
	) {}

	// Nested generics with constraints
	function nested<
	T,
	 U extends 
	 Array<T>,
		 V extends (a: T) =>
		  boolean>(a: U, b: V): T[] {}

	// Class with complex type parameters
	class WithComplexParams<

		T extends {a: string; b: number},
		U extends {c: boolean},
		V extends Error = TypeError,

	> {
		
		fn(a: T): U | V {}
	}
</script>
