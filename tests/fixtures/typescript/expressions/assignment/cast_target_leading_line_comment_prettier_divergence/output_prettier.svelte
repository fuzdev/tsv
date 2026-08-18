<script lang="ts">
	// A `//` glued to the target shell's `(` keeps that line; the shell expands.
	// c1
	(x as T) = 1;

	// An own-line comment keeps its own line inside the shell; a compound
	// operator takes the same rule.
	// c2
	(y satisfies T) += 2;

	// The angle-bracket assertion target.
	// c3
	(<T>z) = 3;

	// A leading run: each comment keeps its line.
	// c4
	// c5
	(v as T) = 4;

	// An author blank below the pulled comment survives.
	// c8

	(w as T) = 7;

	// The same gap inside destructuring-default targets.
	({
		a: // c6
		(b as T) = 5
	} = obj);
	[
		// c7
		(c as T) = 6
	] = arr;
</script>
