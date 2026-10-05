<script lang="ts">
	// A comma is a type-argument separator, so a `>` in a later sibling closes the region an
	// earlier sibling's `<` opened, and a `(` past that `>` commits the list on any line:
	// `fn(a < b, c > (await d))` is the generic call `a<b, c>(await d)`. The clarity pair an
	// operand takes behind any other operator would be the printer's own invention there, so
	// the operand prints bare.

	// Call arguments, `new` arguments, array elements and an optional call.
	fn(a < b, c > await d);
	new Fn(a < b, c > await d);
	const a1 = [a < b, c > await d];
	fn?.(a < b, c > await d);

	// A sequence, a `for` head and a template.
	(a < b, c > await d);
	for (a < b, c > await d; ;) {}
	const a2 = `${(a < b, c > await d)}`;

	// The `<` opens the region wherever it stands in its sibling, and a sibling between the two
	// is one more type argument.
	fn(x && a < b, c > await d);
	fn(a < b, y, c > await d);

	// The `await` may lead a longer operand or a chain.
	fn(a < b, c > await d + e);
	fn(a < b, c > await d.e());
	fn(a < b, c > await d > e);

	// A mixed-arithmetic left operand takes the same clarity pair at the same place.
	fn(a < b, c > d % e + f);
	fn(a < b, c > d * e / f);
	fn(a < b, c > d << e << f);
	fn(a < b, c > d + e << f);
	fn(a < b, c > await d % e + f);

	// So does a function expression that is called or used as a tag, alone or leading a longer
	// operand.
	fn(a < b, c > function () {}());
	fn(a < b, c > function () {}`t`);
	fn(a < b, c > function () {}().e);
	fn(a < b, c > function () {}() % e + f);

	// A `>>` closes two nested lists and a `>>>` three.
	fn(a < b < c, d >> await e);
	fn(a < b < c, d >> e % f + g);
	fn(a < b < c < d, e >>> await f);

	// A second chain's own pair ends its own region and no other: the first sibling's is still
	// open behind the `>`.
	fn(a < b, (c < d) > await e);

	// A pair the AUTHOR wrote stays. Behind a region that reads as a list it is the argument
	// list of a generic call, to every parser.
	fn(a<b, c>(await d));

	// Where no list can be read — an operator or a call inside the region — the comparison
	// reads the same with the pair or without it, and each authoring keeps its own.
	const b1 = a < b && c > (await d);
	const b2 = a < b && c > await d;
	fn(a < b(), c > (await d));
	fn(a < b + 1, c > (d % e) + f);
	fn(a < b + 1, c > (function () {})().e);

	// An operand that takes no pair is untouched.
	fn(a < b, c > -d);
	fn(a < b, c > !d);
	fn(a < b, c > typeof d);
	fn(a < b, c > new D());

	// So is a `>` no region reaches: the chain's own pair ends its region on a `)`, the
	// closer of the delimiter the `<` was written in ends it too, and a statement starts none.
	// Nor does a shift close more regions than are open, or a `<=` open one.
	const b3 = (a < b) > (await c);
	fn(g(a < b), c > (await d));
	fn([a < b], c > (d % e) + f);
	fn(c > (await d));
	fn(c > (function () {})());
	fn(a < b, c >> (await d));
	fn(a << b, c >>> (await d));
	fn(a <= b, c > (await d));

	// A module item is a statement boundary like any other.
	export const b4 = a < b;
	export const b5 = c > (await d);
</script>

<!-- A template expression takes the same rule. -->
{fn(a < b, c > d % e + f)}
