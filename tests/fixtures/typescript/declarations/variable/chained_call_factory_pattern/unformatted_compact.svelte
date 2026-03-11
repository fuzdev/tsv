<script lang="ts">
	// Factory pattern (capital start) + 2 calls + exceeds print width
	// → break-after-operator (chain fits after indent)
	export const Aaaaaaaaaaaaaaaaaaaaa = Bbbbbbbbbbbbbbbbbbbbb.pipe(Ccccccccccccc).brand('Aaaaaaaaaaaaaaaaaaaaa');

	// Non-factory (lowercase start) + 2 calls + exceeds print width
	// → member chain formatting
	export const aaaaaaaaaaaaaaaaaaaaa = bbbbbbbbbbbbbbbbbbbbb.pipe(Ccccccccccccc).brand('aaaaaaaaaaaaaaaaaaaaa');

	// Factory pattern + 3 calls + exceeds print width
	// → member chain formatting (cutoff is 3, but we have 4 groups)
	// Note: first call merges with factory identifier (shouldMerge behavior)
	export const Ccccccccccccccccccccc = Ddddddddddddddddddddd.method1(Eeeeeeeee).method2(Fffffffff).method3('Ccccccccccccccccccccc');

	// Factory pattern + 2 calls + fits on one line
	// → stays on one line
	const result1 = Factory.create(x).build();

	// Non-factory + 2 calls + fits on one line
	// → stays on one line
	const result2 = factory.create(x).build();

	// Underscore/dollar factory patterns (also match /^[$_]+$/)
	// → break-after-operator (2 calls = 3 groups, cutoff = 3)
	export const Xxxxxxxxxxxxxxxxxxxxxxx = $_.method1(Yyyyyyyyyyyy).method2('Xxxxxxxxxxxxxxxxxxxxxxx');

	export const Yyyyyyyyyyyyyyyyyyyyyyy = $__.method1(Zzzzzzzzzzzz).method2('Yyyyyyyyyyyyyyyyyyyyyyy');

	// Pure $__ with 3 calls: merge first call (factory, same as uppercase)
	export const Aaaaaaaaaaaaaaaaaaaaaaaa = $__.method1(Bbbbbbbbbbb).method2(Cccccccccc).method3('Aaaaaaaaaaaaaaaaaaaaaaaa');

	// $util with 3 calls: no merge (not factory, $ prefix with letters)
	export const Ddddddddddddddddddddddd = $util.method1(Eeeeeeeeeee).method2(Fffffffffff).method3('Ddddddddddddddddddddddd');
</script>
