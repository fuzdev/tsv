<script lang="ts">
	// `[`-line comment: tsv keeps it on the `[` line; prettier relocates it to the key's own line
	class A {
		[
			// a
			key: string
		]: number;
	}
	// key→`:` comment: the bracket breaks so the `//` can't swallow `:`; tsv indents the
	// `: type` continuation one level, prettier keeps it flush
	class B {
		[
			key // b
			: string
		]: number;
	}
	// `:`→key-type comment: the key type continues on its own line, indented one level
	class C {
		[
			key: // c
			string
		]: number;
	}
	// own-line comment before `]`: tsv keeps its line; prettier pulls it up to trail the key type
	class D {
		[key: string] // d
		: number;
	}
</script>
