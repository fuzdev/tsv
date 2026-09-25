<script>
	// A curried chain as a callee keeps its parens closed around a hugging body however a
	// chain nested in that body lays out by width

	// Boundary: the nested chain's heads fit at exactly 100 and stay on the key's line, and
	// its body hangs
	((a) => (b) => ({ k: (aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa) => (b) => c }))();

	// One char over (101): the nested chain breaks after the key, and the parens still close
	// on the `}` line
	((a) => (b) => ({ k: (aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa) => (b) => c }))();
</script>
