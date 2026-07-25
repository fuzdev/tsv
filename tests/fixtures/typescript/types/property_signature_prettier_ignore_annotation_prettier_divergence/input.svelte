<script lang="ts">
	// an own-line directive before a property signature's `:` stays own-line and freezes
	// the whole `: type` annotation — interface, type literal, optional marker
	interface A {
		a
			// prettier-ignore
			: {x:   1};
	}

	type B = {
		b // c
			// prettier-ignore
			: {y:   2};
	};

	interface C {
		d?
			/* prettier-ignore */
			: {z:   3};
	}

	// before the `?` marker: the freeze starts at the marker, the text the directive
	// precedes
	interface D {
		e
			// prettier-ignore
			?: {w:   4};
	}
</script>
