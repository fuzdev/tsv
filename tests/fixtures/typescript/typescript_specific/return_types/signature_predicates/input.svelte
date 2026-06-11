<script lang="ts">
	// Type predicate in interface method signature
	interface Guard {
		isA(x: unknown): x is A;
		isThis(): this is A;
		isOptional?(x: unknown): x is A;
		isGeneric<T>(x: unknown): x is T;
		isUnion(x: unknown): x is A | B;
	}

	// asserts predicate in interface method signature
	interface Assert {
		assertA(x: unknown): asserts x is A;
		assertVal(x: unknown): asserts x;
		assertThis(): asserts this;
		assertThisIs(): asserts this is A;
	}

	// Type predicate in interface call signature
	interface Callable {
		(x: unknown): x is A;
	}

	// Predicates in type literal signatures
	type Lit = {
		isA(x: unknown): x is A;
		assertA(x: unknown): asserts x is A;
	};
</script>
