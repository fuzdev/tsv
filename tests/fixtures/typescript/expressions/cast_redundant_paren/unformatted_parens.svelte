<script lang="ts">
	// Short union in redundant parens after `as` - stripped, stays inline
	const a = value as (A | B);

	// Long union in redundant parens after `as` - stripped, breaks after `as` to leading-pipe form
	const b = value as (Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa | Bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb | Ccccccccccccccccccccccccccccccccccccccccccc);

	// Short intersection in redundant parens after `as` - stripped, stays inline
	const c = value as (A & B);

	// Long intersection in redundant parens after `as` - stripped, first member hugs `as`, rest wrap
	const d = value as (Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa & Bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb & Ccccccccccccccccccccccccccccccccccccccccccc);

	// satisfies shares the cast path - long union stripped, breaks after `satisfies`
	const e = value satisfies (Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa | Bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb | Ccccccccccccccccccccccccccccccccccccccccccc);
</script>
