<script lang="ts">
	// Nested object types with index signatures
	type Nested = {
		[key: string]: {
			[inner: string]: number;
		};
	};

	// Multiple index signatures (string + number)
	type Multi = {
		[key: string]: unknown;
		[idx: number]: string;
	};

	// Index signature with complex value types
	type ComplexValue = {
		[key: string]: string | number | null;
	};

	// Index signature with function type value
	type FnMap = {
		[key: string]: (arg: string) => void;
	};

	// Index signature in generic constraint
	function lookup<T extends { [key: string]: unknown }>(obj: T, key: string): unknown {
		return obj[key];
	}

	// Interface with index signature and methods
	interface WithMethods {
		[key: string]: unknown;
		get(key: string): unknown;
		set(key: string, value: unknown): void;
	}
</script>
