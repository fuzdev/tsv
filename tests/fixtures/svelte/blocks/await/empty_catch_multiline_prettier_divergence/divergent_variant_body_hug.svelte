<!-- An empty {:catch} is a rejection handler, so tsv keeps it (prettier deletes it). The empty section still COLLAPSES: its marker glues to the close ({:catch error}{/await}), the same way prettier collapses an empty {:else} to {:else}{/if}. -->

<!-- empty :catch after a multi-node pending body -->
{#await promise}<p>a</p>
	<p>b</p>{/await}

<!-- full form (pending + then bodies), empty :catch -->
{#await promise}<p>a</p>
	<p>b</p>{:then value}<p>c</p>
	<p>d</p>{/await}
