<script>
	let s = 1;
</script>

<!--
	A <script>, <style>, or <svelte:options> between two root siblings is LIFTED out of the fragment before the
	whitespace rules run, so the nodes around it are adjacent to the compiler: a glued pair
	around one stays glued (`a…{const y = 2}b` renders `ab`), exactly as it would with no
	script between. The glue scan must step over the lifted script's byte gap the same way
	it steps over a hoisted neighbour — a break injected there is a rendered space.
-->
a{const y = 2}b c{#snippet fn()}x{/snippet}d
