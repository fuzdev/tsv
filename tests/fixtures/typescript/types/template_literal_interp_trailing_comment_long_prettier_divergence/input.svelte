<script lang="ts">
	// A trailing `${` comment already broke the interpolation, so a too-wide type just
	// breaks INSIDE it — the width rule adds no second break, and no blank line separates
	// the comment from the type. Prettier emits a blank line after `${` here and indents
	// the members an extra level (output_prettier).
	type V = `a${// c
		| AVeryLongTypeName
		| AnotherVeryLongTypeName
		| AThirdQuiteLongTypeName
		| AFourthLongTypeName
		| AFifthOne
	}b`;
</script>
