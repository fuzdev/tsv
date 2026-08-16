<script lang="ts">
	// The test and update clauses keep an assignment's clarity parens.
	for (; (x = fn());) {}
	for (let i = 0; (x = fn()); i++) {}

	// A for-header DECLARATOR's initializer keeps them too — the assignment is the
	// declarator's value, not the header's own init clause.
	for (let a = (b = c); ;) {}
	for (let d = 1, e = (f = g); ;) {}

	// A trailing block comment strips the redundant shell and stays inline inside
	// the pair the declarator supplies.
	for (let h = (i = j /* c1 */); ;) {}

	// The init clause's own expression is the exception: no parens there.
	for (k = l = m; ;) {}
	for (; ; n = o = p) {}

	// A for-in / for-of iterable keeps them.
	for (const q of (r = s)) {
		fn();
	}
	for (const t in (u = v)) {
		fn();
	}
</script>
