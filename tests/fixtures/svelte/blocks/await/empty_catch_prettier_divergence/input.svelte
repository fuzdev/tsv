<!-- An empty {:catch} is KEPT, not dropped: an empty catch still handles a rejection (renders nothing), whereas dropping it would let the rejection propagate — a semantic change. Prettier deletes it; tsv keeps it. (An empty :then, by contrast, IS dropped, matching prettier.) -->

<!-- catch-shorthand, empty body: kept -->
{#await promise catch error}{/await}

<!-- empty :catch after content: kept -->
{#await promise}x{:catch error}{/await}

<!-- inline sibling before an empty-catch await: the </a> closing > stays hugged -->
<div>
	<a href="foo">bar</a>{#await promise catch error}{/await}
</div>
