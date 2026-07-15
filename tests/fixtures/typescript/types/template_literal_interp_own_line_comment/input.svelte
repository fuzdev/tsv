<script lang="ts">
	// A comment authored on its OWN LINE after a template-literal type's `${` keeps that
	// line: the interpolation expands and the type drops below the comment. Both formatters
	// agree here, so there is no divergence. The trailing authoring, which tsv keeps
	// trailing `${` and prettier moves down, is
	// template_literal_interp_trailing_comment_prettier_divergence.
	type T = `a${
		// c
		B
	}`;

	type U = `a${
		/* c
		d */
		B
	}`;
</script>
