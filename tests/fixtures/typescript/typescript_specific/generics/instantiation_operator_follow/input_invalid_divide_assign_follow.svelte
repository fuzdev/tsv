<script lang="ts">
	// A `/=` on the same line refuses the list too: it can start a regular expression, so
	// tsc reads `f < T >` followed by the unterminated literal `/= c;`. acorn-typescript
	// reads an instantiation assigned to and rejects it only as a target. Past a line break
	// tsc does take the list (`f<T>⏎/= c`), so the printer, which joins the two lines,
	// prints that one as `(f<T>) /= c`.
	f<T> /= c;
</script>
