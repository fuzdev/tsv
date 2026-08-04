<script lang="ts">
	// three arguments: the callback needs a block body and at most one parameter
	test('aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa', (a) => {}, 2500);
	test('aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa', function (a) {}, 2500);

	// two parameters - excluded, so every argument breaks out
	test(
		'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
		(a, b) => {},
		2500
	);

	// an expression body - excluded for the same reason
	test(
		'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
		(a) => fn(a),
		2500
	);

	// a third argument that is not a number - excluded
	test(
		'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
		() => {},
		opts
	);

	// the excluded forms are ordinary calls, so a short one still fits on one line
	test('text', (a, b) => {}, 2500);
</script>
