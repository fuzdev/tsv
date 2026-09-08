<script lang="ts">
	// A `//` after a sole member's authored pipe in a template-literal type's `${` is the
	// interpolation's: it stays trailing the `${`, the type dropped below and `}` on its
	// own line, exactly as the pipe-less authoring prints
	// (template_literal_interp_trailing_comment). The pipe never survives
	type A = `x${ // c1
		B
	}`;

	// a slot followed by more of the literal, and a literal that is only the slot
	type C = `x${ // c2
		D
	}y${E}`;

	type F = `${ // c3
		G
	}`;
</script>
