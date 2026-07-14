<script>
	// An own-line annotation on a class property value hangs onto an indented continuation
	// line after the `=`, exactly as any same-width ordinary comment does.
	class C {
		a =
			/**
			 * @__PURE__
			 */ fn(x, y, z);

		b =
			/**
			 * hiiiiiiii
			 */ fn(x, y, z);

		// A trivial (no-argument) call is a "poorly breakable chain" — a static claim that the
		// chain has no internal break points. The annotation's own hardlines are in the printed
		// value regardless, so the value still hangs.
		c =
			/**
			 * @__PURE__
			 */ fn();

		d =
			/**
			 * hiiiiiiii
			 */ fn();
	}
</script>
