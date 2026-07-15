<script lang="ts">
	// Block comment inside `require(` — hugs the paren, keeps its space on the literal's side
	import A = require(/* c1 */ './a');

	// Two comments in the same gap keep their order
	import B = require(/* c2 */ /* c3 */ './b');
</script>
