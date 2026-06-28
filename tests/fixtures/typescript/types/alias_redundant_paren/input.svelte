<script lang="ts">
	// Short union in redundant parens - stripped, stays inline
	type ShortU = A | B;

	// Long union in redundant parens - stripped, breaks to leading-pipe form
	type LongU =
		Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa | Bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb;

	// Short intersection in redundant parens - stripped, stays inline
	type ShortI = A & B;

	// Long intersection in redundant parens - stripped, first member hugs `=`, rest wrap
	type LongI = Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa &
		Bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb;
</script>
