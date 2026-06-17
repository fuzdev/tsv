<script lang="ts">
	// A line comment between the key and `:` forces the bracket to break and keeps
	// the comment after the key; the `: type` continuation drops to the next line,
	// indented one level so it reads as part of the key parameter.
	interface A {
		[key // c
		: string]: number;
	}

	// Two line comments in the same gap: each stays on its own line, then the value
	// type's `:` follows, all on the continuation indent.
	interface B {
		[key // a
		// b
		: string]: number;
	}
</script>
