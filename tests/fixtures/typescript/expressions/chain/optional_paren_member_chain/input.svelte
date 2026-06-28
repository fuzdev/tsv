<script>
	// Parens around an optional-chain base in a member chain preserve semantics:
	// `(a?.b).c.d()` reads `.c` on the chain's result (throws if `a` is null);
	// `a?.b.c.d()` folds `.c.d()` into the chain (short-circuits to undefined).
	(a?.b).c.d();
	(a?.b).c.d().e();
	(a?.b?.c).d.e().f();
	(a?.b).c[d]();
</script>
