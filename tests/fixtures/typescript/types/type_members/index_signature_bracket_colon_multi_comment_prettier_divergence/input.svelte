<script lang="ts">
	// Two line comments in the `]`→value-`:` gap: the first trails `]` on its line,
	// the second keeps its own line, then the value `:` drops to a continuation line
	// indented one level. Each comment is preserved as a separate node — collapsing
	// them onto one line (`// a // b`) would be content loss.
	interface A {
		[k: string] // a
			// b
			: number;
	}

	type T = {
		[k: string] // c
			// d
			: number;
	};
</script>
