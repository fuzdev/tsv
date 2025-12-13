<script lang="ts">
	// Basic typed object destructuring
	const {a1}: {a1: number} = obj;
	let {x1, y1}: {x1: number; y1: string} = obj;
	const {a2, b2, c2}: {a2: string; b2: number; c2: boolean} = obj;

	// With rename and defaults
	const {a3: x2 = 1}: {a3?: number} = obj;
	let {x3: a4 = 0, y3: b3 = 0}: {x3?: number; y3?: number} = obj;

	// With rest
	const {a5, ...rest}: {a5: number; b5: string; c5: number} = obj;

	// Nested type
	const {a6}: {a6: {b6: number; c6: string}} = obj;

	// Complex type annotation (multiline)
	const {
		a7,
	}: {
		a7: {
			b7: string;
			c7: number;
		};
	} = obj;

	// Long type that wraps
	const {a8}: {a8: VeryLongTypeName<AnotherType, ThirdType>} = obj;
</script>
