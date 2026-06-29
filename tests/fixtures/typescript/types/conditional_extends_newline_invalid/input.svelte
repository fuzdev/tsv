<script lang="ts">
	// `CheckType [no LineTerminator here] extends ExtendsType ? … : …`: a line
	// terminator before `extends` ends the type, so the conditional is a syntax
	// error (acorn-typescript `hasPrecedingLineBreak`). The valid baseline keeps
	// `extends` on the check-type line.
	type A<U> = U extends string ? 1 : 2;
</script>
