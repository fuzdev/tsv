<script lang="ts">
	// Type predicate in declare class method
	declare class Guard {
		isA(x: unknown): x is A;
		isUnion(x: unknown): x is A | B;

		// asserts predicate in declare class method
		assertA(x: unknown): asserts x is A;
		assertVal(x: unknown): asserts x;
		assertThis(): asserts this;

		// type predicate in declare class accessor
		get a(): this is A;
	}
</script>
