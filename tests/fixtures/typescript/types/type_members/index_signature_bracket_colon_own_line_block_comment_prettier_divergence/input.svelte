<script lang="ts">
	// Single comment.
	interface A {
		[k: string] /* c */ : number;
	}

	// A run keeps order, each comment distinct.
	interface B {
		[k: string] /* c1 */ /* c2 */ : number;
	}
</script>
