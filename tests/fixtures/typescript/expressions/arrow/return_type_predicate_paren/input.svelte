<script lang="ts">
	// Indexed access on a parenthesized `typeof` type
	const f1 = (a: A): a is (typeof b)[number] => true;

	// Array suffix on a parenthesized union
	const f2 = (a: A): a is (B | C)[] => true;

	// Function type as the predicate's type
	const f3 = (a: A): a is (x: B) => C => true;

	// Generic arrow
	const f4 = <T extends A>(a: A): a is (typeof b)[] => true;

	// `asserts` prefix
	const f5 = (a: A): asserts a is (B | C)[] => {};

	// Arrow as a call argument
	g((a: A): a is (B | C)[] => true);

	// `is` as an ordinary call, after a parenthesized ternary consequent
	const h1 = cond ? (a, b) : is(a);

	// `is` as an ordinary type name in a return type
	const h2 = (a: A): is => a;
</script>
