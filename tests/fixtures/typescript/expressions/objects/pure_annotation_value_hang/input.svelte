<script>
	// An own-line annotation on an object value hangs onto an indented continuation line
	// after the `:`, exactly as any same-width ordinary comment does.
	const a = {
		b:
			/**
			 * @__PURE__
			 */ fn(x, y, z)
	};

	const b = {
		b:
			/**
			 * hiiiiiiii
			 */ fn(x, y, z)
	};

	// A trivial (no-argument) call is a "poorly breakable chain" — a static claim that the
	// chain has no internal break points. The annotation's own hardlines are in the printed
	// value regardless, so the value still hangs.
	const c = {
		b:
			/**
			 * @__PURE__
			 */ fn()
	};

	const d = {
		b:
			/**
			 * hiiiiiiii
			 */ fn()
	};
</script>
