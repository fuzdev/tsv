<script lang="ts">
	// a `//` INSIDE the union — between its own start and its first member — takes the
	// union's first-member form, after the leading `|` at the member offset
	type A = (
	// c1
	B)[] | C;

	// the same comment written AHEAD of the union leads the whole RHS instead, flush at
	// the `=` gap's continuation indent
	type D =
	// c2
	E[] | F;
</script>
