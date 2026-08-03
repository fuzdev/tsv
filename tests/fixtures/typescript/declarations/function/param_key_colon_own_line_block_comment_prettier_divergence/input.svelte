<script lang="ts">
	// Single comment.
	function fn1(a /* c */ : string) {}

	// A run keeps order, each comment distinct.
	function fn2(b /* c1 */ /* c2 */ : number) {}
</script>
