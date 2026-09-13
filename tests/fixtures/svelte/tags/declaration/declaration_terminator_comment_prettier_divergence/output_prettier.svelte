<script lang="ts">
	let x: number = 1;
</script>

{let a; /* c1 */}
{let b: T; /* c2 */}
{let c!: T; /* c3 */}
{const d = 1; /* c4 */}
{let e, f; /* c5 */}
{let g = 1,
	h = 2; /* c6 */}
{const { i, j } = expr; /* c7 */}
{let k; /* m1
m2 */}
{let l;} // c8
{const m = 1;} // c9
{let r = 1,
	s = 2;} // c12

{#if cond}
	{let n;} // c10
	{const o = 1; /* c11 */}
{/if}

{let p;}
{const q = 1}
