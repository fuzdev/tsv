<script lang="ts">
	// 100 columns — the retained shell and its operand stay on one line.
	const a = <T>(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa + bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb) /* c */;

	// 101 columns — the operand's chain breaks, and its continuation is indented
	// one level inside the retained parens rather than snapping back outside them.
	const b = <T>(
		(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa + bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb) /* c */
	);
</script>
