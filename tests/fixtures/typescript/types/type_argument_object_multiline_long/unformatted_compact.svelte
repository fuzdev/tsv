<script lang="ts">
	// 100 chars - fits entirely on one line (no wrapping)
	const a: Array<{a: A; b: A; c: A; d: A; e: A; f: A; g: A; h: A; i: A; j: A; k: A; l: Abc}> = [];

	// 101 chars - object type stays inline, only `= []` wraps to next line
	const b: Array<{a: A; b: A; c: A; d: A; e: A; f: A; g: A; h: A; i: A; j: A; k: A; l: Abcd}> = [];

	// 108 chars - object type argument expands to multiline (hugs opening brace)
	const c: Array<{a: A; b: A; c: A; d: A; e: A; f: A; g: A; h: A; i: A; j: A; k: A; l: A; m: A; n: A}> = [];
</script>
