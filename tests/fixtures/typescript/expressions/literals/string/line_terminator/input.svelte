<script lang="ts">
	// A raw <LF> or <CR> may not appear in a string literal — the character reaches one
	// only as an escape. (<LS> / <PS> may, and are covered by
	// typescript/syntax/unicode_line_terminators.)
	const a = '\r';
	const b = '\n';
	const c = '\r\n';
</script>
