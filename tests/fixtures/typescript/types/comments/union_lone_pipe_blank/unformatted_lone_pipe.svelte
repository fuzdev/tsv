<script lang="ts">
	// a lone pipe's own line is not a blank — the comment stays directly below the previous member
	type T =
		| a
		|
		// c1
		b;

	// an authored blank after the lone pipe is preserved above the comment
	type U =
		| c
		|

		// c2
		d;
</script>
