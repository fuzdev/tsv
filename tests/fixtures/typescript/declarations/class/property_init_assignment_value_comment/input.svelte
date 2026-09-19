<script lang="ts">
	class C {
		// an assignment value keeps its clarity parens under every comment layout the
		// property's other values take
		// own-line block comment: stays on its own line, the value hangs below it
		a =
			/* c */
			(b = c);
		// indentable block comment: breaks after `=`, the value stays glued to `*/`
		d =
			/**
			 * c
			 */ (e = f);
		// line comment: trails the `=`, the value hangs below it
		g = // c
			(h = i);
		// glued single-line block comment: stays inline
		j = /* c */ (k = l);
	}
</script>
