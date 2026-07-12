<script lang="ts">
	// Consecutive line comments between a decorator and the class: the first
	// trails the decorator, each subsequent one keeps its own line (they must not
	// merge — `// c1 // c2` would reparse as a single comment).
	@dec // c1
	// c2
	class A {}
</script>
