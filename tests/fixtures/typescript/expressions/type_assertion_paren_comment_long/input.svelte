<script lang="ts">
	// exactly 100 wide: the cast and its commented operand stay inline
	x = <TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT>/*c*/ a;

	// 101 wide: the operand breaks into parens, the comment leading it inside them
	x = <TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT>(
		/*c*/ a
	);

	// the `<type>(` line exactly 100 wide still holds the type
	x = <TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT>(
		/*c*/ a
	);

	// the `<type>(` line 101 wide: the cast breaks after `<`, the operand inline
	x = <
		TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT
	>/*c*/ a;
</script>
