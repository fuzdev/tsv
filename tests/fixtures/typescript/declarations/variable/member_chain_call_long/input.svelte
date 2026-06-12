<script>
	// Member chain call with arrow returning object - exactly 100 chars inline (both match)
	const {a1, b1, c1aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa} = a.fn(() => ({a1: x()}));

	// 101 chars with `() => ({` - arrow can't hug, call args expand (NOT break after =)
	const {a2, b2, c2aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa} = a.fn(
		() => ({
			a2: x(),
			b2: y(),
		}),
	);

	// `a.fn(` line at exactly 100 chars - call args still expand
	const {a3, b3, c3aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa} = a.fn(
		() => ({
			a3: x(),
			b3: y(),
		}),
	);

	// `a.fn(` line at 101 chars - both break after = (match)
	const {a4, b4, c4aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa} =
		a.fn(() => ({
			a4: x(),
			b4: y(),
		}));

	// Contrast: plain call (no member chain) at 101 `fn(() => ({` - correctly expands
	const {a5, b5, c5aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa} = fn(
		() => ({
			a5: x(),
			b5: y(),
		}),
	);

	// Short - everything fits on one line
	const {a6, b6} = a.fn(() => ({a6: x(), b6: y()}));
</script>
