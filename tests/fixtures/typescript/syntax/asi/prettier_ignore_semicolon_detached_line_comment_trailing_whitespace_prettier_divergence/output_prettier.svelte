<script lang="ts">
	// A frozen statement whose `//` ends in whitespace still keeps its terminator below the
	// comment, as authored — appending the `;` to the comment's line would swallow it
	// prettier-ignore
	let a = b; // c1
	[a].map(fn);
	// prettier-ignore
	fn(  b  ); // c2
	[b].map(fn);
	function f1() {
		// prettier-ignore
		return  a; // c3
	}
</script>
