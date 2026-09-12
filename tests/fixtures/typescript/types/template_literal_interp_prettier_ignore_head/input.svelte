<script lang="ts">
	// an own-line directive in a template literal type's `${`→type gap freezes the type
	type A = `x${
		// prettier-ignore
		{a:   1}['k']
	}`;

	// the own-line block spelling behaves like the line spelling
	type C = `z${
		// prettier-ignore
		{a:   3}
	}`;
</script>
