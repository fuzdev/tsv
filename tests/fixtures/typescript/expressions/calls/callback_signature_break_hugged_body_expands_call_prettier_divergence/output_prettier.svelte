<script lang="ts">
	// a sole parameter's authored-expanded object type stays expanded; every argument breaks
	// out around an object body
	fn((a: { b: C }) => ({ a }));

	// new
	new Xy((a: { b: C }) => ({ a }));

	// member chain
	obj.a.fn((a: { b: C }) => ({ a }));

	// a preceding argument
	fn(lead, (a: { b: C }) => ({ a }));

	// a preceding argument, member chain
	obj.a.fn(lead, (a: { b: C }) => ({ a }));

	// a sole parameter's authored-expanded object type stays expanded; every argument breaks
	// out around an array body
	fn((a: { b: C }) => [a]);

	// new
	new Xy((a: { b: C }) => [a]);

	// member chain
	obj.a.fn((a: { b: C }) => [a]);

	// a preceding argument
	fn(lead, (a: { b: C }) => [a]);

	// a preceding argument, member chain
	obj.a.fn(lead, (a: { b: C }) => [a]);

	// a sole parameter's authored-expanded object type stays expanded; every argument breaks
	// out around a conditional body
	fn((a: { b: C }) => (a ? b : c));

	// new
	new Xy((a: { b: C }) => (a ? b : c));

	// member chain
	obj.a.fn((a: { b: C }) => (a ? b : c));

	// a preceding argument
	fn(lead, (a: { b: C }) => (a ? b : c));

	// a preceding argument, member chain
	obj.a.fn(lead, (a: { b: C }) => (a ? b : c));

	// the object type annotating a sole destructured parameter stays expanded too, around an
	// object body
	fn(({ typography }: { typography: T }) => ({ a: typography }));

	// a curried callee
	memoize(1)(({ typography }: { typography: T }) => ({ a: typography }));
</script>
