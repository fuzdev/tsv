<script lang="ts">
	// Short - stays on one line (contrast case)
	type Short<T> = T extends string ? T : never;

	// Check type with type args in extends clause
	type LongGenericCheck<T> =
		T extends Promise<LongGenericTypeName | AnotherLongGenericTypeName>
			? LongGenericTypeName
			: never;

	// Mapped type in extends clause - stays inline, ternary wraps
	type LongMappedCheck<T> = T extends { [K in keyof LongObjectTypeName]: LongValueTypeName }
		? LongValueTypeName
		: never;

	// Function type with type params in extends - function wraps
	type LongFunctionCheck<T> =
		T extends <U extends LongConstraintTypeName>(param: U) => Promise<LongReturnTypeName>
			? LongReturnTypeName
			: never;

	// Long intersection in extends clause
	type LongIntersectionCheck<T> = T extends LongTypeNameOne & LongTypeNameTwo & LongTypeNameThree
		? LongTypeNameOne
		: never;

	// Long union in extends clause
	type LongUnionCheck<T> = T extends
		| LongTypeNameOne
		| LongTypeNameTwo
		| LongTypeNameThree
		| LongTypeNameFour
		? LongTypeNameOne
		: T;

	// Multiple conditionals with long extends clauses
	type LongChainedCheck<T> =
		T extends Promise<LongResultTypeName>
			? LongResultTypeName
			: T extends Array<LongElementTypeName>
				? LongElementTypeName
				: never;
</script>
