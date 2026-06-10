<script lang="ts">
	// Short union named-tuple member: redundant parens stripped, stays inline
	type ShortU = [label: (A | B)];

	// Long union named-tuple member: parens stripped, breaks to leading-pipe
	type LongU = [label: (Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa | Bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb | Ccccccccccccccccccccccccccccccccccccccccccc)];

	// Short intersection named-tuple member: stripped, stays inline
	type ShortI = [label: (A & B)];

	// Long intersection named-tuple member: stripped, first member hugs, rest wrap
	type LongI = [label: (Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa & Bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb & Ccccccccccccccccccccccccccccccccccccccccccc)];
</script>
