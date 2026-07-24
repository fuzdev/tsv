<script lang="ts">
	// a block directive glued directly before a member freezes that member whole (matches
	// prettier); the separators are parent-owned and stay normalized
	type A = a | /* prettier-ignore */ {x:1} | b;

	// composite member: the whole parenthesized member is frozen
	type B = a | /* prettier-ignore */ ({y:2} & c);

	// intersection host, same rule
	type C = a & /* prettier-ignore */ {z:3} & d;

	// other comments in the glued run are transparent — the directive freezes the member
	// from anywhere in the run (matches prettier); only a separator breaks the glue
	type D = a | /* prettier-ignore */ /* other */ {w:4} | e;
</script>
