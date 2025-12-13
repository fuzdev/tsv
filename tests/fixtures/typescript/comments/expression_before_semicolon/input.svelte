<script lang="ts">
	// Comment moves before "as const" in Prettier 3.7 (#18161)
	1 /* comment */ as const;

	// Other expression statements with trailing comments
	fn() /* comment */;
	a + b /* comment */;
</script>
