<script lang="ts">
	// Short union type predicate: redundant parens stripped, stays inline
	function shortU(p: unknown): p is A | B {
		return true;
	}

	// Long union type predicate: parens stripped, breaks after `is`
	function longU(
		p: unknown,
	): p is
		| Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
		| Bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
		| Ccccccccccccccccccccccccccccccccccccccccccc {
		return true;
	}

	// Short intersection type predicate: stripped, stays inline
	function shortI(p: unknown): p is A & B {
		return true;
	}

	// Long intersection type predicate: stripped, first member hugs, rest wrap
	function longI(
		p: unknown,
	): p is Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa &
		Bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb &
		Ccccccccccccccccccccccccccccccccccccccccccc {
		return true;
	}
</script>
