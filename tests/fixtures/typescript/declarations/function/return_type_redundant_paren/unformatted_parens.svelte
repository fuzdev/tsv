<script lang="ts">
	// Short union return type in redundant parens - stripped, stays inline
	function fnA(): (A | B) {
		return x;
	}

	// Long union return type in redundant parens - stripped, breaks after `:` to leading-pipe form
	function fnB(): (Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa | Bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb | Ccccccccccccccccccccccccccccccccccccccccccc) {
		return x;
	}

	// Short intersection return type in redundant parens - stripped, stays inline
	function fnC(): (A & B) {
		return x;
	}

	// Long intersection return type in redundant parens - stripped, first member hugs `:`, rest wrap
	function fnD(): (Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa & Bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb & Ccccccccccccccccccccccccccccccccccccccccccc) {
		return x;
	}
</script>
