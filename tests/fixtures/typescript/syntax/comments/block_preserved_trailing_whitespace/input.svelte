<!--
	A multi-line block comment that is NOT `*`-aligned is emitted VERBATIM by both
	formatters - prettier's non-indentable arm is `['/*', replaceEndOfLine(value), '*/']`,
	which touches nothing, and both doc printers skip their line-end trim behind a LITERAL
	newline. So the TRAILING SPACES on the comment's first two interior lines below are the
	claim, and they are invisible: reflowing this file by hand, or trimming the preserved
	arm 'for tidiness', destroys the fixture rather than failing it. Its indentable twin
	trims every line and is `comments/block_indentable_unicode_space`; the line comment's
	own trailing trim is `comments/trailing_unicode_space`.
-->
<script lang="ts">
	/* a   
	  b   
	  c */
	const x = 1;
</script>
