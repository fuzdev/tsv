<script lang="ts">
	// A non-null assertion between a parenthesized optional chain and a non-optional
	// access keeps the parens: they seal the chain so the trailing access is not
	// absorbed. `(a?.b)!.c` reads `.c` on the asserted result; `a?.b!.c` would
	// short-circuit `.c` when `a` is null — a different expression.
	(a?.b)!.c;
	(a?.b)!();
	(a?.b)![c];
	// chain ending in an optional call, then `!` and a non-optional access
	(a?.())!.c;
	// `!` then a member chain after a sealed optional chain
	(a?.b)!.c.d;
</script>
