<script lang="ts">
	// exactly 100 chars — the annotation stays inline, comment glued before the first member
	let s: /* c */
	eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee | fffffffffffffffffffffffffffffffffffffff;

	// exactly 101 chars — breaks after `:`; an own-line-authored run stays above the flat union
	let m: /* c */
	eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee | ffffffffffffffffffffffffffffffffffffffff;

	// too wide even indented — the run stays above the union's pipes
	let w: /* c */
	| aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
	| bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb;

	// function return type: same rule as the let annotation
	function fn(): /* c */
	| cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc
	| ddddddddddddddddddddddddddddddddddddddddddddddddddddddddd {
		return 'c' as never;
	}
</script>
