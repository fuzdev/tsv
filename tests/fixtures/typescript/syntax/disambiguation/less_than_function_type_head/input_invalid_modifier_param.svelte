<script lang="ts">
	// tsc's `isUnambiguouslyStartOfFunctionType` runs its modifier step before the
	// parameter start, so the compiler and prettier read a type-argument list here and
	// reject the parameter property inside it. tsv claims the region for the same reason
	// and rejects it too, rather than printing the comparison chain the region is not.
	f<(readonly a: T) => U>(x);
</script>
