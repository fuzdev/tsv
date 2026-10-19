<script lang="ts">
	// A two-link stack ending in a call keeps its links glued below a long head too. The first
	// link is a lookup on the head's call: at 100 columns the head fills its line and that
	// lookup moves to the next one, and at 101 the head's own arguments break instead
	xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx.mmmmmmmmmmmmmmmmmmmmmmmmmmm(rrrrrrrrrrrrrrrrrrrrrrrrrrrrr)
		.o(`
a
`).o(`
b
`);
	xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx.mmmmmmmmmmmmmmmmmmmmmmmmmmm(
		rrrrrrrrrrrrrrrrrrrrrrrrrrrrr
	).o(`
a
`).o(`
b
`);
</script>
