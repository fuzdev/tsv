<script lang="ts">
	interface FooU {
		// Short union member in redundant parens - stripped, stays inline
		a: (A | B);

		// Long union member in redundant parens - stripped, breaks after `:` to leading-pipe form
		b: (Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa | Bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb | Ccccccccccccccccccccccccccccccccccccccccccc);
	}

	interface FooI {
		// Short intersection member in redundant parens - stripped, stays inline
		c: (A & B);

		// Long intersection member in redundant parens - stripped, first member hugs `:`, rest wrap
		d: (Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa & Bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb & Ccccccccccccccccccccccccccccccccccccccccccc);
	}
</script>
