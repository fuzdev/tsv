<script lang="ts">
	// A `//` comment authored on the `(` line of a return/throw argument stays trailing
	// the `(`; prettier relocates it to its own line. The comment forces the parens to
	// break either way — the only question is whether it leads the argument line or
	// trails the `(`.
	function assign() {
		return ( // c
			a = b
		);
	}
	function seq() {
		return ( // c
			a, b
		);
	}
	function ident() {
		return ( // c
			a
		);
	}
	function thrower() {
		throw ( // c
			a = b
		);
	}
</script>
