<script lang="ts">
	type X = T extends (// c
		U) ? A : B;

	// A REQUIRED pair survives the strip: without it the `?` rebinds and the output
	// does not reparse
	type Y = T extends (// c
		V extends W ? A1 : B1) ? A : B;

	// An operand whose pair is redundant here still sheds it
	type Z = T extends (// c
		() => void) ? A : B;
</script>
