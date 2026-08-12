<script lang="ts">
	// A block comment between the operand and its `!`, where a member access
	// continues the chain.
	const a = aaa/* c */!.bbb;

	// The same gap where a call or a computed access continues the chain.
	const b = fn/* c */!(1);
	const c = aaa/* c */![0];

	// The operand is itself a call.
	const d = fn()/* c */!.bbb;

	// Repeated non-null links, each with its own comment.
	const e = aaa/* c1 */!.bbb/* c2 */!.ccc;

	// Optional chain continued by a non-null link.
	const f = p?.q/* c */!.r;
</script>
