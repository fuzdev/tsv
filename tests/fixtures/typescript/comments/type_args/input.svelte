<script lang="ts">
	// Basic union with trailing comment
	const a = fn<T /* comment */ | U>();

	// Intersection with trailing comment
	const b = fn<T /* comment */ & U>();

	// Multiple comments around union separator
	const c = fn</* a */ T /* b */ | /* c */ U /* d */>();

	// Nested generic with comment
	const d = fn<Array<T /* comment */>>();

	// Comment containing pipe character (edge case)
	const e = fn<T /* | */ | U>();

	// Multiple type args with comments
	const f = fn<Map<K /* k */, V /* v */>>();

	// Intersection with comments around separator
	const g = fn<T /* a */ & /* b */ U>();

	// Multiple consecutive comments
	const h = fn<T /* a */ /* b */ | U>();
</script>
