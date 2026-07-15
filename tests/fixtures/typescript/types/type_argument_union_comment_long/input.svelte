<script lang="ts">
	// A comment between the union members disqualifies the hug, so the type argument
	// is no longer inlined atomically and the `<...>` breaks at the boundary.

	// type position - 100 chars, stays inline
	type C = Foo<{ a: AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA } /* c */ | null>;

	// type position - 101 chars, `<...>` breaks with the union inline
	type D = Foo<
		{ a: AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA } /* c */ | null
	>;

	// expression position - 100 chars, stays inline
	const a = fn<{ a: AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA } /* c */ | null>();

	// expression position - 101 chars, `<...>` breaks with the union inline
	const b = fn<
		{ a: AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA } /* c */ | null
	>();
</script>
