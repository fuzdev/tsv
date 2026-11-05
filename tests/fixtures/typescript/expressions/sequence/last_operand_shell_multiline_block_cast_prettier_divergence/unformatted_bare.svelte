<script lang="ts">
	// A multi-line block ending a redundant pair around a sequence's last operand, ahead of a
	// cast keyword, keeps a pair around the sequence and the block, as the bare spelling does.
	void (a, b /* x1
	 */) as T;

	void (a, b /* x2
	 */) satisfies T;

	void (a, b /* x3
	 */) as const;
</script>
