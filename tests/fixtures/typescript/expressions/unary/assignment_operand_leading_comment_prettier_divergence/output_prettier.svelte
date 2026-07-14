<script lang="ts">
	// A leading block comment glued inside a unary operand's required parens — an
	// assignment needs the parens — stays inside the single paren where the author
	// wrote it. prettier relocates it out, before the operand's own parens.
	const a = !(/* c */ (x = y));
</script>
