<script>
	// a property NAMED like a method keyword is an ordinary assignment property
	({ get, set, async, static: s } = x);
	({ get: a1, set: a2, async: a3 } = x);

	// a function value is a default or the source, never the target
	({ f = function () {} } = x);
	[g = () => {}] = x;
	({ h } = { h() {} });
</script>
