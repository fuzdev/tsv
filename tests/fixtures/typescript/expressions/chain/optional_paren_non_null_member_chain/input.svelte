<script lang="ts">
	// A non-null assertion on a parenthesized optional-chain base, followed by a
	// member chain that contains a call, keeps the parens. `(a?.b)!.c()` calls `.c`
	// on the asserted result (throws if `a` is null); `a?.b!.c()` folds `.c()` into
	// the chain (short-circuits to undefined).
	(a?.b)!.c();
	(a?.b)!.c.d();
	(a?.b)!.c().d;
</script>
