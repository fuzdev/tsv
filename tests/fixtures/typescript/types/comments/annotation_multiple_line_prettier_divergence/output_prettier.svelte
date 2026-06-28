<script lang="ts">
	// Union annotation: first comment trails `:`, remaining comments and the type indent one level
	let {
		a,
		b
	}:
		// union one
		// union two
		A | B = init1;

	// Three line comments: each breaks onto its own line (none merge)
	let {
		c,
		d
	}:
		// union three a
		// union three b
		// union three c
		A | B = init2;

	// Block comment stays inline after `:`; the line comment ends its line
	let {
		e,
		f
	}:
		/* lead */ // trail
		A | B = init3;
</script>
