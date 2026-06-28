<script lang="ts">
	// A non-null `!` authored INSIDE the sealing parens (`(a?.b!).c`) is kept where
	// written. `(a?.b!).c` and the `!`-outside form `(a?.b)!.c` are semantically
	// identical (the `!` is type-only) but parse to different ASTs, so the inner
	// placement is preserved.
	(a?.b!).c;
	(a?.b!)();
	(a?.b!)[c];
	// `!` inside, after an optional call
	(a?.()!).c;
	// `!` inside, then a further member chain
	(a?.b!).c.d;
</script>
