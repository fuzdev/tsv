<script lang="ts">
	// Two line comments in the `]`→value-`:` gap: the first trails `]` on its line,
	// the second keeps its own line, then the value `:` drops to a continuation line
	// indented one level. Each comment is preserved as a separate node — collapsing
	// them onto one line (`// c1 // c2`) would be content loss.
	type M = {
		[K in keyof T] // c1
			// c2
			: V;
	};
</script>
