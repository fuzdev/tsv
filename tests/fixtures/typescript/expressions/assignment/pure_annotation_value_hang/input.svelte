<script>
	// An own-line annotation on an assignment value hangs onto an indented continuation line
	// after the `=`, exactly as any same-width ordinary comment does — including on a trivial
	// (no-argument) call, whose chain has no internal break points of its own.
	a.b =
		/**
		 * @__PURE__
		 */ fn();

	a.c =
		/**
		 * hiiiiiiii
		 */ fn();

	const e =
		/**
		 * @__PURE__
		 */ fn();

	const f =
		/**
		 * hiiiiiiii
		 */ fn();
</script>
