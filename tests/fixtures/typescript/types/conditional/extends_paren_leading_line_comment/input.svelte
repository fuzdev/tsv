<script lang="ts">
	type X = T extends U // c
		? A
		: B;

	// A REQUIRED pair survives the strip: without it the `?` rebinds and the output
	// does not reparse
	type Y = T extends (V extends W ? A1 : B1) // c
		? A
		: B;

	// An operand whose pair is redundant here still sheds it
	type Z = T extends () => void // c
		? A
		: B;
</script>
