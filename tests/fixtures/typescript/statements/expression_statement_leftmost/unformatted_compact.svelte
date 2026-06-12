<script>
	({}).foo;
	(class {}).foo;
	(function () {}).foo;
	(class A extends Base {}).foo;
	(class {}) + 1;
	({} = a);

	// Multi-segment chains — the leftmost still needs parens even when the chain
	// rebuilds its base across conditional-group layout variants
	({a:1}).b().c();
	(class {}).foo().bar();
	(function () {}).call().toString();
</script>
