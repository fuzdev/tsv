<script lang="ts">
	// A `//` after a sole member's authored operator in an angle-bracket assertion is the
	// cast's own `<`→type gap, and takes that gap's answer exactly as the operator-less
	// authoring does: it keeps the `<` line (the opening-delimiter rule)
	const a = <& (// c1
	| A)>x;

	// a function-type member, whose shell it needed only under the operator — generic or not
	const b = <& (// c2
	| (() => R))>x;

	const c = <& (// c3
	| (<T>() => R))>x;

	// a sole-member composite wrapping a real union: the redundant shell strips and the
	// union prints bare, the comment still on the `<` line
	const d = <& (// c4
	| A | B)>x;
</script>
