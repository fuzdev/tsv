<script lang="ts">
	// Member expression call with multiple args, last is arrow returning multiline object
	// Prettier hugs: a.b(c, (x) => ({...}))
	// Bug: we expand all args to separate lines
	a.b(c, (x) => ({
		y: x,
		z: x
	}));

	// With destructuring parameter
	a.b(c, ([d, e]) => ({
		f: d,
		g: e.h,
		i: e.j
	}));

	// String first arg
	a.b('./c', () => ({
		d: fn()
	}));

	// With inline block comment - still hugs
	something.method(firstArg /* comment */, (x) => ({
		veryLongPropertyName: x,
		anotherLongProperty: x
	}));
</script>
