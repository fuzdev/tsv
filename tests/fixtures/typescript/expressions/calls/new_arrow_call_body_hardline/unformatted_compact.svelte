<script lang="ts">
	// block arrow callback in call body
	const a = new Xy((x) => fn(x, () => { doSomething(); doSomethingElse(); }));

	// with await
	const b = await new Xy((x) => fn(x, () => { doSomething(); doSomethingElse(); }));
</script>
