<script lang="ts">
	// `BindingIdentifier [no LineTerminator here] !` (definite assignment): a line
	// terminator before `!` makes it not a definite-assignment assertion, so `!` is
	// a stray token (acorn-typescript `hasPrecedingLineBreak`). The valid baseline
	// keeps `!` on the identifier line.
	let x!: number;
</script>
