<script lang="ts">
	// A postfix `++` ends an operand, so the `/` divides and the comment stays inside the parens
	if (aa++ / bb > 1 /* c1 */) {
		g();
	}

	// Same for postfix `--`
	if (aa-- / bb > 1 /* c2 */) {
		g();
	}

	// Another paren-scanned head
	switch (aa++ / bb /* c3 */) {
		default:
			break;
	}

	// Plain division — the operand is an identifier
	if (aa / bb > 1 /* c4 */) {
		g();
	}

	// A binary `+` does NOT end an operand, so the `/` still opens a regex
	if (aa + /re/.test(b) /* c5 */) {
		g();
	}
</script>
