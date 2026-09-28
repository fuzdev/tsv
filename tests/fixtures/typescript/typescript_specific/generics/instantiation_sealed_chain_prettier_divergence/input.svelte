<script lang="ts">
	// an optional chain the author sealed keeps its pair ahead of type arguments
	x = (a?.b)<T>;
	x = (a?.b())<T>;
	(a?.b)<T>?.();
	// the non-null spelling, which prettier keeps too
	x = (a?.b)!<T>;
	// a tag or a `new` callee, where the bare spelling does not parse
	new (a?.b())<T>`t`();
	new ((a?.b())!<T>)<T>`t`();
</script>

{(a?.b)<T>}
{(a?.b)<T>?.()}
{new ((a?.b())!<T>)<T>`t`()}
