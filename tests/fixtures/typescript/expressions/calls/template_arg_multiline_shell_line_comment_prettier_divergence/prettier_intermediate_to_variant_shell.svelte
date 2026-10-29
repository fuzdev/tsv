<script lang="ts">
	// A `//` ahead of a sole multiline template argument ends its line, so the template
	// starts a line of its own and the arguments break — the `//` stays on the `(` line
	aaa(// c1
	`
`);
	bbb.ccc(// c2
	`
`);
</script>
