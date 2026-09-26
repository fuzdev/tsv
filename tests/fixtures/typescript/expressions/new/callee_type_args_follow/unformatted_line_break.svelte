<script lang="ts">
	// An optional chain cannot follow a `new` whose callee carries type arguments and no
	// argument list, so the argument list the author writes, or the pair around the whole
	// `new`, is what makes one legal.
	new f<T>()?.(x);
	new f<T>()?.[x];
	new a.b<T>()?.(x);
	new f<T>()!;

	// Past a line break the type arguments bind to the callee instead: `new f<T>⏎[x](y)`
	// constructs the computed member `f<T>[x]`, so on one line the instantiation keeps its
	// pair, which ends the list ahead of the `[` that would otherwise re-read it as a
	// comparison.
	new f<T>
		[x](y);
	new a.b<T>
		[x];
	new f<T>
		[x].y();
</script>
