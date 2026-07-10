<script lang="ts">
	// Line comment before the mapped key (after `[`): the bracket breaks multiline
	// so the `//` can't swallow the key, each part on its own line.
	type A = {
		[
			// c1
			K in keyof A
		]: V;
	};

	// Two own-line comments with a blank between them: the blank is preserved.
	type B = {
		[
			// c1

			// c2
			K in keyof A
		]: V;
	};

	// `readonly` modifier and an optional `?` value.
	type C = {
		readonly [
			// c1
			K in keyof A
		]?: V;
	};

	// An `as` clause remaps the key.
	type D = {
		[
			// c1
			K in keyof A as U
		]: V;
	};
</script>
