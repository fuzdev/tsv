<script lang="ts">
	// Simple qualified - parens stripped correctly
	type T = ((ns.X));

	// Top-level union/intersection - prettier strips parens
	type U = ((ns.X & { a: number }));
	type V = ((ns.X) | null);

	// Indexed access - parens semantically required
	type W = ((ns.X & { a: number })[keyof ns.X]);
	type X = (((A | B))[K]);

	// typeof before indexed access - parens required
	type Y = (((typeof obj))[K]);
	type Z = (((typeof arr))[number]);

	// keyof with union/intersection - parens required
	type AA = (keyof ((A | B)));
	type AB = (keyof ((A & B)));

	// Nested conditional - parens on single line only
	type BB<T> = (T extends A ? (T extends B ? C : D) : E);

	// Function type in indexed access
	type CC = (((()=>T))[K]);

	// Conditional in indexed access
	type DD = (((T extends U ? V : W))[K]);

	// Deeply nested - inner parens required
	type EE = ((((A | (B))) & (C))[K]);

	// typeof with qualified name
	type FF = (((typeof ns.obj))[K]);

	// Chained indexed access with parens
	type GG = ((((A) | (B)))[K][J]);
</script>
