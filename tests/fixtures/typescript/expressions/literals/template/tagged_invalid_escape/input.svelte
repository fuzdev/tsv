<script lang="ts">
	// Tagged templates tolerate otherwise-invalid escape sequences (ES2018 template
	// literals revision): the cooked value is null but the raw text is preserved.
	const unicode = tag`\u{zzz}`;
	const unicodeShort = tag`\unicode`;
	const unterminated = tag`\u{41`;
	const hex = tag`\xZZ`;
	const interpolated = tag`a\u{zz}${expr}c`;
	const member = String.raw`\u{zzz}`;

	// Valid escapes in a tagged template still cook normally.
	const valid = tag`\n\t`;
</script>
