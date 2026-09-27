<!-- a whitespace-only pending branch renders nothing: it folds into the head, printing nothing -->
{#await promise} {:then value}
<p>text1</p>
<p>text2</p>
{/await}

{#await promise} {:then}
<p>text1</p>
<p>text2</p>
{/await}

{#await promise} {:catch error}
<p>text1</p>
<p>text2</p>
{/await}

{#await promise} {:then value}
<p>text1</p>
<p>text2</p>
{:catch error}
<p>text3</p>
{/await}

{#await promise} {:catch error}
<p>text1</p>
<p>text2</p>
{:then value}
<p>text3</p>
{/await}

{#await promise} {:then value}
text
{/await}

<div>
{#await promise} {:then value}
<p>text1</p>
<p>text2</p>
{/await}
</div>
