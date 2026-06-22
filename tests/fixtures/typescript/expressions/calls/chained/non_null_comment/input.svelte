<script lang="ts">
	// Non-null assertion with comment before member
	declare const a: { b: { c: () => void } } | null;
	a! /* comment */.b
		.c();
</script>
