<script>
	// A declaration's initializer inherits the head's `[~In]`: a bare `in` would end the
	// init, so each nested one is parenthesized, and each position below restores `[+In]`
	for (let a = (b in c); d; e) {}
	for (const a = arr[b in c]; d; e) {}
	for (var a = cond ? b in c : 0; d; e) {}
	for (let a = function () { z = b in c; }; d; e) {}
	for (let a = f(b in c); d; e) {}
	for (let a = (x = b in c) => {}; d; e) {}

	// a later declarator inherits it too
	for (let a = 0, b = (c in d); e; f) {}
</script>
