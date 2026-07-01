<script lang="ts">
	// Function types (pattern or type-keyword params) nested at full-type positions
	// inside an arrow's return-type annotation parse greedily — their own `=>` binds
	// inside; the enclosing arrow's `=>` follows the whole annotation.
	const f1 = (): B<([a]) => C> => x;
	const f2 = (): [([a]) => C] => y;
	const f3 = (): { m: ([a]) => C } => z;
	const f4 = (): B<(number) => C> => w;

	// A function type directly in the return type consumes the first `=>`;
	// the enclosing arrow's `=>` comes after its return type.
	const f5 = (): (([b]) => X) => v;

	// Conditional-type branches are full-type positions too.
	const f6 = (): A extends B ? ([c]) => D : E => u;
	const f7 = (): A extends B ? C : ([d]) => E => t;
</script>
