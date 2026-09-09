<script lang="ts">
	declare function ambient(@dec/* one */(1) a): void;

	function overload(@dec/* two */(2) b, @dec/* three */(3) c): void;
	function overload(b: number, c: number): void {}
</script>
