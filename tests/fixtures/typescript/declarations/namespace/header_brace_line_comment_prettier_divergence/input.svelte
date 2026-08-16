<script lang="ts">
	// Line comment between the namespace header and the body `{` — `{` drops to the next line
	namespace N1 // c1
	{
		const a = 1;
	}

	// Empty body — the collapsed braces still drop below the comment
	namespace N2 // c2
	{}

	// Multiple line comments, each kept on its own line (not merged)
	namespace N3 // c3
	// c4
	{
		const b = 2;
	}

	// `module` takes the same rule
	module M1 // c5
	{
		const c = 3;
	}

	// `declare global` — `global` is both keyword and name, so the gap follows it
	declare global // c6
	{}

	// A dotted name — the gap is after the innermost segment
	namespace N4.Inner // c7
	{
		const d = 4;
	}

	// A string-named external module takes it too — the name is a literal, not an
	// identifier, and the gap is the same one
	declare module 'm' // c9
	{
		const f = 6;
	}

	// A single-line block comment collapses onto the header line and `{` hugs it
	namespace N5 /* c8 */ {
		const e = 5;
	}
</script>
