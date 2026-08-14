<script lang="ts">
	// An author blank between arguments forces every argument onto its own line,
	// defeating each specialized argument layout below.

	// Two callbacks
	fn(

		() => a,

		() => b

	);

	// Object-literal arguments - would otherwise stay flat
	fn(
		{ a: 1 },

		{ b: 2 }
	);

	// Hug-last - the trailing function would otherwise hug the `(`
	fn(
		a,

		function () {
			return b;
		}
	);

	// Expand-first - the leading block arrow would otherwise hug
	fn(
		() => {
			a();
		},

		1
	);

	// Function composition
	fn(
		items.map((item) => item),

		b
	);

	// Multiline string argument
	fn(
		'a\
b',

		c
	);

	// `new` takes the same path
	new Cls(
		() => a,

		() => b
	);

	// Dynamic import
	import(
		'./a',

		{ with: { type: 'json' } }
	);

	// A member chain's arguments print through their own builder
	a.b().c(
		() => a,

		() => b
	);

	// So does an optional call's
	a?.(
		() => a,

		() => b
	);

	// Contrast - with no blank, each layout still applies
	fn({ a: 1 }, { b: 2 });
	fn(a, function () {
		return b;
	});
	fn(() => {
		a();
	}, 1);

	// A test call bypasses the argument printer entirely, so a blank between its
	// arguments is dropped - as are the two edge gaps, which are not BETWEEN
	// arguments. See unformatted_dropped_blanks.
	it(
		'name',

		() => {
			fn();
		}
	);
</script>
