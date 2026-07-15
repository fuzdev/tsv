<script lang="ts">
	// A comment authored TRAILING a template-literal type's `${` stays trailing it, with
	// the type dropped below and `}` on its own line — the same broken shape a width-driven
	// break takes, so a `//` can't swallow the type and a multiline block's authored break
	// isn't reflowed. Prettier instead expands the interpolation, moving the comment down
	// onto its own line (output_prettier).
	//
	// That expanded form is variant_expanded: a fixed point of BOTH formatters, so tsv does
	// not pull the comment back up to `${` — the two authorings keep two stable forms on
	// purpose. Authoring it own-line from the start is template_literal_interp_own_line_comment.
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
