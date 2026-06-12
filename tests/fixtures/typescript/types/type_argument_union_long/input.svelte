<script lang="ts">
	// Short union in generic - stays inline
	type Short = Promise<A | B>;

	// 100 chars - stays inline (at boundary)
	type T100 = Promise<AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA | B>;

	// 101 chars - breaks after < with union inline, > on own line
	type T101 = Promise<
		AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA | B
	>;

	// Both union members long - breaks each with leading |
	type Both = Promise<
		| AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
		| BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB
	>;
</script>
