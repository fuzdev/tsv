<script lang="ts">
	// source to with, block comment
	import a from './a' /* c1 */ with {type: 'json'};

	// source to with, line comment
	import b from './b' with {type: 'json'}; // c2

	// with to brace, block comment
	import c from './c' /* c3 */ with {type: 'json'};

	// with to brace, line comment
	import d from './d' with {type: 'json'}; // c4
</script>
