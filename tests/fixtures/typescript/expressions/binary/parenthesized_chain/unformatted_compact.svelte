<script lang="ts">
	// Short chain - stays flat (23 chars)
	(a/b/c).method();

	// At exactly 100 chars - Prettier keeps flat
	(aaaaaaaaaaaaaaaaaaaaaaaaaaaa/bbbbbbbbbbbbbbbbbbbbbbbbbbb/ccccccccccccccccccccccccc).method();

	// At 101 chars - Prettier breaks with parens on own lines
	(aaaaaaaaaaaaaaaaaaaaaaaaaaaaa/bbbbbbbbbbbbbbbbbbbbbbbbbbb/ccccccccccccccccccccccccc).method();
</script>
