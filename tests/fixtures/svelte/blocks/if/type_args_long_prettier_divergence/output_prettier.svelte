<script lang="ts"></script>

<div>
	<!-- call whose type arguments wrap: the last argument hugs after the >, then )} (divergence) -->
	{#if fn<Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, Bbbbbbbbbbbbbbbbbbbbbbbbbbbbb>( { a: 1 } )}
		z
	{/if}
	<!-- >( ends at column 100: the type arguments fit, the arguments break out (divergence) -->
	{#if fn<Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, Bbbbbbbbbbbbbbbbbbbbbbbbbbbbb>( { a: 1 } )}
		z
	{/if}
	<!-- >( would end at column 101: the type arguments wrap, the last argument hugs (divergence) -->
	{#if fn<Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, Bbbbbbbbbbbbbbbbbbbbbbbbbbbbb>( { a: 1 } )}
		z
	{/if}
</div>
