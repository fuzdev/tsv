<script>
	// parens terminate the optional chain so the trailing access is NOT absorbed into it
	// (a?.b).c reads .c on the chain's result; a?.b.c would short-circuit .c — different semantics
	(a?.b).c;
	(a?.b)();
	(a?.b)[c];
	// chain ending in an optional call, then a non-optional access
	(a?.()).b;
	(a?.())();
</script>
