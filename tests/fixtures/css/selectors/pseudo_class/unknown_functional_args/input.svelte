<style>
	/* Unknown functional pseudo-class: an An+B argument structures as an Nth
	   node - the name is 'foo', not swallowed into 'foo(2n + 1)' */
	:foo(2n + 1) {
		color: red;
	}

	/* Comma-separated numbers become multiple Nth nodes */
	:foo(0, 1, 2) {
		color: blue;
	}

	/* The odd/even keywords and a bare n are Nth nodes too */
	:foo(odd) {
		color: green;
	}

	:foo(n) {
		color: teal;
	}

	/* A negative An+B (leading minus, required +B tail) is an Nth node */
	:foo(-n + 3) {
		color: purple;
	}

	/* A normal selector argument already structures (regression guard) */
	:foo(.a, .b) {
		color: orange;
	}
</style>
