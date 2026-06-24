<script lang="ts">
	// source to with, block comment
	import a from './a' /* c1 */ with { type: 'json' };

	// with to brace, block comment
	import c from './c' /* c3 */ with { type: 'json' };

	// with to brace, block comment whose text contains the `with` keyword
	import e from './e' /* with */ with { type: 'json' };

	// with to brace, line comment
	import d from './d' with { type: 'json' }; // c4
</script>
