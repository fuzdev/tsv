<script lang="ts">
	// an own-line directive before a conditional-type value freezes it whole
	type A = (
		// prettier-ignore
		{x:   1}
	) extends Y ? a : b;

	// a composite check type rides inside the freeze, operators and all
	type B = (
		// prettier-ignore
		{x:   1} | {y:   2}
	) extends Y ? a : b;

	// the own-line block spelling behaves like the line spelling
	type C = (
		/* prettier-ignore */
		{x:   1}
	) extends Y ? a : b;
</script>
