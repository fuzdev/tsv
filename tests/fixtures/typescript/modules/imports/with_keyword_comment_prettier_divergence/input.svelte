<script lang="ts">
	// source to with, block comment
	import a from './a' /* c1 */ with {type: 'json'};

	// source to with, line comment
	import b from './b' // c2
	with {type: 'json'};

	// with to brace, block comment
	import c from './c' with /* c3 */ {type: 'json'};

	// with to brace, line comment
	import d from './d' with // c4
	{type: 'json'};
</script>
