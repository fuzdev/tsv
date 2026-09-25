<!--
	A block binding's type annotation behind non-ASCII text: two-byte (`éé`), three-byte
	(`中中`) and astral (`𝑎`, two UTF-16 units) bindings, and U+3000 / U+00A0 between a
	binding and its colon; `ab` is the ASCII control. unformatted_binding_newline and
	unformatted_colon_newline put a newline in the four code units before each multibyte
	binding's colon — except `𝑎𝑎`, four units wide, whose newline sits just outside them.
-->
<script lang="ts">
	let xs: any;
	let p: any;
</script>

{#each xs as éé: T}{éé}{/each}
{#each xs as 中中: T}{中中}{/each}
{#each xs as 𝑎: T}{𝑎}{/each}
{#each xs as 𝑎𝑎: T}{𝑎𝑎}{/each}
{#each xs as ab: T}{ab}{/each}
{#each xs as éé: T, i (i)}{éé}{/each}
{#each xs as e
　　: T}{e}{/each}
{#each xs as [
	e
] 
: T}{e}{/each}
{#each xs as 𝑎é
	: T}{𝑎é}{/each}
{#await p then éé: T}{éé}{/await}
{#await p}x{:then éé: T}{éé}{/await}
{#await p}x{:catch 𝑎: T}{𝑎}{/await}
{#if p}
	{@const 中中: T = p}
	{中中}
{/if}
