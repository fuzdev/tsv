<script lang="ts">
	// a leading-operator union/intersection has ONE member, so it prints as that member —
	// the parens a real union/intersection would need in these positions are redundant
	type A1 = (& { x: X })[];
	type A2 = (| { x: X })[K];

	// the optional tuple element, whose `?` is what needs the parens in the real case
	type A3 = [(& { x: X })?];
	type A4 = [(| { y: Y })?];

	// a comment in the collapsed member's gap survives the collapse
	type A5 = [(& /* c */ { x: X })?];
</script>
